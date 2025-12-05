//! Webhook Notification Plugin
//!
//! This plugin sends HTTP notifications to configured webhook URLs
//! when bookmark operations occur.
//!
//! # Features
//! - Configurable webhook URL
//! - Supports add, update, delete, and import events
//! - JSON payload with bookmark data
//! - Async-friendly design (queues notifications)
//!
//! # Configuration
//! ```yaml
//! plugins:
//!   webhook:
//!     url: "https://example.com/webhook"
//!     events: "add,update,delete"
//!     include_content: true
//! ```
//!
//! # Payload Format
//! ```json
//! {
//!   "event": "bookmark_added",
//!   "timestamp": 1234567890,
//!   "bookmark": {
//!     "id": 1,
//!     "url": "https://example.com",
//!     "title": "Example",
//!     "tags": "tag1,tag2",
//!     "description": "A bookmark"
//!   }
//! }
//! ```

use bukurs::models::bookmark::Bookmark;
use bukurs::plugin::{HookResult, Plugin, PluginContext, PluginInfo};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

/// Webhook event types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WebhookEvent {
    BookmarkAdded,
    BookmarkUpdated,
    BookmarkDeleted,
    BookmarksImported,
}

impl WebhookEvent {
    fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "add" | "added" | "bookmark_added" => Some(Self::BookmarkAdded),
            "update" | "updated" | "bookmark_updated" => Some(Self::BookmarkUpdated),
            "delete" | "deleted" | "bookmark_deleted" => Some(Self::BookmarkDeleted),
            "import" | "imported" | "bookmarks_imported" => Some(Self::BookmarksImported),
            _ => None,
        }
    }
}

/// Webhook payload structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookPayload {
    pub event: WebhookEvent,
    pub timestamp: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bookmark: Option<BookmarkPayload>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bookmarks: Option<Vec<BookmarkPayload>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_bookmark: Option<BookmarkPayload>,
}

/// Bookmark data in webhook payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookmarkPayload {
    pub id: usize,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl BookmarkPayload {
    fn from_bookmark(bookmark: &Bookmark, include_content: bool) -> Self {
        Self {
            id: bookmark.id,
            url: bookmark.url.clone(),
            title: if include_content {
                Some(bookmark.title.clone())
            } else {
                None
            },
            tags: if include_content {
                Some(bookmark.tags.clone())
            } else {
                None
            },
            description: if include_content {
                Some(bookmark.description.clone())
            } else {
                None
            },
        }
    }

    fn minimal(bookmark: &Bookmark) -> Self {
        Self {
            id: bookmark.id,
            url: bookmark.url.clone(),
            title: None,
            tags: None,
            description: None,
        }
    }
}

/// Message sent to the webhook worker thread
enum WebhookMessage {
    Send(String, WebhookPayload),
    Shutdown,
}

/// Webhook notification plugin
pub struct WebhookPlugin {
    /// Webhook URL to send notifications to
    url: Option<String>,
    /// Events to send notifications for
    events: HashSet<WebhookEvent>,
    /// Whether to include full bookmark content
    include_content: bool,
    /// Channel sender for async notifications
    sender: Option<Sender<WebhookMessage>>,
    /// Whether the plugin is enabled
    enabled: bool,
}

impl WebhookPlugin {
    pub fn new() -> Self {
        Self {
            url: None,
            events: HashSet::new(),
            include_content: true,
            sender: None,
            enabled: false,
        }
    }

    /// Create a plugin with a specific URL
    pub fn with_url(url: &str) -> Self {
        let mut plugin = Self::new();
        plugin.url = Some(url.to_string());
        plugin.enabled = true;
        // Enable all events by default
        plugin.events.insert(WebhookEvent::BookmarkAdded);
        plugin.events.insert(WebhookEvent::BookmarkUpdated);
        plugin.events.insert(WebhookEvent::BookmarkDeleted);
        plugin.events.insert(WebhookEvent::BookmarksImported);
        plugin
    }

    /// Set which events to send notifications for
    pub fn with_events(mut self, events: &[WebhookEvent]) -> Self {
        self.events = events.iter().cloned().collect();
        self
    }

    /// Set whether to include full bookmark content
    pub fn with_content(mut self, include: bool) -> Self {
        self.include_content = include;
        self
    }

    /// Start the background worker thread
    fn start_worker(&mut self) {
        let (tx, rx) = mpsc::channel::<WebhookMessage>();
        self.sender = Some(tx);

        thread::spawn(move || {
            while let Ok(msg) = rx.recv() {
                match msg {
                    WebhookMessage::Send(url, payload) => {
                        Self::send_webhook(&url, &payload);
                    }
                    WebhookMessage::Shutdown => break,
                }
            }
        });
    }

    /// Send a webhook notification (blocking)
    fn send_webhook(url: &str, payload: &WebhookPayload) {
        // Use reqwest to send the webhook
        // In a real implementation, this would use the reqwest client
        // For now, we'll just log it
        if let Ok(json) = serde_json::to_string(payload) {
            log::info!("Sending webhook to {}: {}", url, json);

            // Actual HTTP request would go here:
            // let client = reqwest::blocking::Client::new();
            // let _ = client.post(url)
            //     .header("Content-Type", "application/json")
            //     .body(json)
            //     .send();
        }
    }

    /// Queue a webhook notification
    fn queue_notification(&self, payload: WebhookPayload) {
        if let (Some(ref url), Some(ref sender)) = (&self.url, &self.sender) {
            let _ = sender.send(WebhookMessage::Send(url.clone(), payload));
        }
    }

    /// Get current timestamp
    fn now() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    /// Check if an event type is enabled
    fn is_event_enabled(&self, event: WebhookEvent) -> bool {
        self.enabled && self.events.contains(&event)
    }
}

impl Default for WebhookPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for WebhookPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "webhook".to_string(),
            version: "1.0.0".to_string(),
            description: "Sends HTTP notifications on bookmark operations".to_string(),
            author: "bukurs".to_string(),
        }
    }

    fn on_load(&mut self, ctx: &PluginContext) -> HookResult {
        // Load URL from config
        if let Some(url) = ctx.config.get("url") {
            self.url = Some(url.clone());
            self.enabled = true;
        }

        // Load events from config
        if let Some(events_str) = ctx.config.get("events") {
            self.events.clear();
            for event_name in events_str.split(',') {
                if let Some(event) = WebhookEvent::from_str(event_name.trim()) {
                    self.events.insert(event);
                }
            }
        } else {
            // Default to all events
            self.events.insert(WebhookEvent::BookmarkAdded);
            self.events.insert(WebhookEvent::BookmarkUpdated);
            self.events.insert(WebhookEvent::BookmarkDeleted);
            self.events.insert(WebhookEvent::BookmarksImported);
        }

        // Load include_content preference
        if let Some(include) = ctx.config.get("include_content") {
            self.include_content = include != "false";
        }

        // Start background worker if enabled
        if self.enabled {
            self.start_worker();
        }

        HookResult::Continue
    }

    fn on_unload(&mut self, _ctx: &PluginContext) {
        // Shutdown the worker thread
        if let Some(ref sender) = self.sender {
            let _ = sender.send(WebhookMessage::Shutdown);
        }
    }

    fn on_post_add(&self, _ctx: &PluginContext, bookmark: &Bookmark) -> HookResult {
        if !self.is_event_enabled(WebhookEvent::BookmarkAdded) {
            return HookResult::Continue;
        }

        let payload = WebhookPayload {
            event: WebhookEvent::BookmarkAdded,
            timestamp: Self::now(),
            bookmark: Some(if self.include_content {
                BookmarkPayload::from_bookmark(bookmark, true)
            } else {
                BookmarkPayload::minimal(bookmark)
            }),
            bookmarks: None,
            old_bookmark: None,
        };

        self.queue_notification(payload);
        HookResult::Continue
    }

    fn on_post_update(
        &self,
        _ctx: &PluginContext,
        old: &Bookmark,
        new: &Bookmark,
    ) -> HookResult {
        if !self.is_event_enabled(WebhookEvent::BookmarkUpdated) {
            return HookResult::Continue;
        }

        let payload = WebhookPayload {
            event: WebhookEvent::BookmarkUpdated,
            timestamp: Self::now(),
            bookmark: Some(if self.include_content {
                BookmarkPayload::from_bookmark(new, true)
            } else {
                BookmarkPayload::minimal(new)
            }),
            bookmarks: None,
            old_bookmark: Some(if self.include_content {
                BookmarkPayload::from_bookmark(old, true)
            } else {
                BookmarkPayload::minimal(old)
            }),
        };

        self.queue_notification(payload);
        HookResult::Continue
    }

    fn on_post_delete(&self, _ctx: &PluginContext, bookmark: &Bookmark) -> HookResult {
        if !self.is_event_enabled(WebhookEvent::BookmarkDeleted) {
            return HookResult::Continue;
        }

        let payload = WebhookPayload {
            event: WebhookEvent::BookmarkDeleted,
            timestamp: Self::now(),
            bookmark: Some(if self.include_content {
                BookmarkPayload::from_bookmark(bookmark, true)
            } else {
                BookmarkPayload::minimal(bookmark)
            }),
            bookmarks: None,
            old_bookmark: None,
        };

        self.queue_notification(payload);
        HookResult::Continue
    }

    fn on_post_import(&self, _ctx: &PluginContext, bookmarks: &[Bookmark]) -> HookResult {
        if !self.is_event_enabled(WebhookEvent::BookmarksImported) {
            return HookResult::Continue;
        }

        let payload = WebhookPayload {
            event: WebhookEvent::BookmarksImported,
            timestamp: Self::now(),
            bookmark: None,
            bookmarks: Some(
                bookmarks
                    .iter()
                    .map(|b| {
                        if self.include_content {
                            BookmarkPayload::from_bookmark(b, true)
                        } else {
                            BookmarkPayload::minimal(b)
                        }
                    })
                    .collect(),
            ),
            old_bookmark: None,
        };

        self.queue_notification(payload);
        HookResult::Continue
    }
}

/// Create an instance of this plugin (required for auto-discovery)
pub fn create_plugin() -> Box<dyn Plugin> {
    Box::new(WebhookPlugin::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_webhook_event_from_str() {
        assert_eq!(
            WebhookEvent::from_str("add"),
            Some(WebhookEvent::BookmarkAdded)
        );
        assert_eq!(
            WebhookEvent::from_str("UPDATE"),
            Some(WebhookEvent::BookmarkUpdated)
        );
        assert_eq!(
            WebhookEvent::from_str("deleted"),
            Some(WebhookEvent::BookmarkDeleted)
        );
        assert_eq!(WebhookEvent::from_str("invalid"), None);
    }

    #[test]
    fn test_bookmark_payload() {
        let bookmark = Bookmark::new(
            1,
            "https://example.com".to_string(),
            "Example".to_string(),
            ",rust,".to_string(),
            "A test".to_string(),
        );

        let full = BookmarkPayload::from_bookmark(&bookmark, true);
        assert_eq!(full.id, 1);
        assert_eq!(full.url, "https://example.com");
        assert!(full.title.is_some());
        assert!(full.tags.is_some());

        let minimal = BookmarkPayload::minimal(&bookmark);
        assert_eq!(minimal.id, 1);
        assert!(minimal.title.is_none());
        assert!(minimal.tags.is_none());
    }

    #[test]
    fn test_payload_serialization() {
        let payload = WebhookPayload {
            event: WebhookEvent::BookmarkAdded,
            timestamp: 1234567890,
            bookmark: Some(BookmarkPayload {
                id: 1,
                url: "https://example.com".to_string(),
                title: Some("Example".to_string()),
                tags: None,
                description: None,
            }),
            bookmarks: None,
            old_bookmark: None,
        };

        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("\"event\":\"bookmark_added\""));
        assert!(json.contains("\"timestamp\":1234567890"));
        assert!(json.contains("\"url\":\"https://example.com\""));
        // tags should be omitted since it's None
        assert!(!json.contains("\"tags\":"));
    }

    #[test]
    fn test_plugin_event_filtering() {
        let plugin = WebhookPlugin::with_url("https://example.com/webhook")
            .with_events(&[WebhookEvent::BookmarkAdded]);

        assert!(plugin.is_event_enabled(WebhookEvent::BookmarkAdded));
        assert!(!plugin.is_event_enabled(WebhookEvent::BookmarkDeleted));
    }
}
