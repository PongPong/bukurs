//! Clipboard Watcher Plugin
//!
//! Monitors clipboard for URLs and offers to add them as bookmarks.
//! This plugin provides a command to start watching the clipboard.
//!
//! Note: This is a passive plugin - it stores clipboard URLs for later review
//! rather than running a background thread.

use bukurs::models::bookmark::Bookmark;
use bukurs::plugin::{HookResult, Plugin, PluginContext, PluginInfo};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// A URL captured from clipboard
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardCapture {
    /// The URL
    pub url: String,
    /// When it was captured
    pub captured_at: u64,
    /// Whether it's been added as a bookmark
    pub added: bool,
}

/// Persisted clipboard data
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ClipboardData {
    /// Queue of captured URLs
    captures: VecDeque<ClipboardCapture>,
    /// Last seen clipboard content (to detect changes)
    last_content: String,
}

pub struct ClipboardWatcherPlugin {
    /// Clipboard data
    data: Mutex<ClipboardData>,
    /// Data file path
    data_file: Option<PathBuf>,
    /// Maximum captures to keep
    max_captures: usize,
    /// Whether the plugin is enabled
    enabled: bool,
    /// Auto-add tag for clipboard bookmarks
    clipboard_tag: String,
}

impl ClipboardWatcherPlugin {
    pub fn new() -> Self {
        Self {
            data: Mutex::new(ClipboardData::default()),
            data_file: None,
            max_captures: 50,
            enabled: true,
            clipboard_tag: "from-clipboard".to_string(),
        }
    }

    /// Get current timestamp
    fn now() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    /// Load data from file
    fn load_data(&self) -> ClipboardData {
        if let Some(ref path) = self.data_file {
            if let Ok(contents) = fs::read_to_string(path) {
                if let Ok(data) = serde_json::from_str(&contents) {
                    return data;
                }
            }
        }
        ClipboardData::default()
    }

    /// Save data to file
    fn save_data(&self) {
        if let Some(ref path) = self.data_file {
            if let Ok(data) = self.data.lock() {
                if let Ok(json) = serde_json::to_string_pretty(&*data) {
                    let _ = fs::write(path, json);
                }
            }
        }
    }

    /// Check if a string looks like a URL
    fn is_url(s: &str) -> bool {
        let s = s.trim();
        s.starts_with("http://") || s.starts_with("https://")
    }

    /// Check clipboard and capture any URLs
    pub fn check_clipboard(&self) -> Option<String> {
        // Try to get clipboard content
        #[cfg(target_os = "windows")]
        {
            use clipboard_win::{formats, get_clipboard};
            if let Ok(content) = get_clipboard::<String, _>(formats::Unicode) {
                return Some(content);
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            // For non-Windows, we'd need a different clipboard library
            // For now, return None
        }

        None
    }

    /// Capture a URL from clipboard
    pub fn capture_url(&self, url: &str) {
        if !Self::is_url(url) {
            return;
        }

        let url = url.trim().to_string();

        if let Ok(mut data) = self.data.lock() {
            // Check if we already have this URL
            if data.captures.iter().any(|c| c.url == url) {
                return;
            }

            // Check if it's different from last content
            if data.last_content == url {
                return;
            }

            data.last_content = url.clone();

            // Add new capture
            let capture = ClipboardCapture {
                url,
                captured_at: Self::now(),
                added: false,
            };

            data.captures.push_front(capture);

            // Trim old captures
            while data.captures.len() > self.max_captures {
                data.captures.pop_back();
            }
        }

        self.save_data();
    }

    /// Get pending (not yet added) captures
    pub fn get_pending(&self) -> Vec<ClipboardCapture> {
        if let Ok(data) = self.data.lock() {
            data.captures
                .iter()
                .filter(|c| !c.added)
                .cloned()
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Mark a URL as added
    pub fn mark_added(&self, url: &str) {
        if let Ok(mut data) = self.data.lock() {
            for capture in data.captures.iter_mut() {
                if capture.url == url {
                    capture.added = true;
                    break;
                }
            }
        }
        self.save_data();
    }

    /// Check if bookmark URL came from clipboard
    fn is_from_clipboard(&self, url: &str) -> bool {
        if let Ok(data) = self.data.lock() {
            data.captures.iter().any(|c| c.url == url && !c.added)
        } else {
            false
        }
    }

    /// Add clipboard tag to bookmark
    fn add_tag(tags: &str, tag: &str) -> String {
        let mut tag_list: Vec<String> = tags
            .trim_matches(',')
            .split(',')
            .filter(|t| !t.is_empty())
            .map(|t| t.to_string())
            .collect();

        if !tag_list.contains(&tag.to_string()) {
            tag_list.push(tag.to_string());
        }

        if tag_list.is_empty() {
            String::new()
        } else {
            format!(",{},", tag_list.join(","))
        }
    }
}

impl Default for ClipboardWatcherPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for ClipboardWatcherPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "clipboard-watcher".to_string(),
            version: "1.0.0".to_string(),
            description: "Captures URLs from clipboard".to_string(),
            author: "bukurs".to_string(),
        }
    }

    fn on_load(&mut self, ctx: &PluginContext) -> HookResult {
        self.data_file = Some(ctx.data_dir.join("clipboard_captures.json"));

        // Load existing data
        let loaded = self.load_data();
        if let Ok(mut data) = self.data.lock() {
            *data = loaded;
        }

        // Load config
        if let Some(enabled) = ctx.config.get("enabled") {
            self.enabled = enabled != "false";
        }
        if let Some(max) = ctx.config.get("max_captures") {
            self.max_captures = max.parse().unwrap_or(50);
        }
        if let Some(tag) = ctx.config.get("clipboard_tag") {
            self.clipboard_tag = tag.clone();
        }

        // Check clipboard on load
        if self.enabled {
            if let Some(content) = self.check_clipboard() {
                self.capture_url(&content);
            }
        }

        HookResult::Continue
    }

    fn on_unload(&mut self, _ctx: &PluginContext) {
        self.save_data();
    }

    fn on_pre_add(&self, _ctx: &PluginContext, bookmark: &mut Bookmark) -> HookResult {
        if !self.enabled {
            return HookResult::Continue;
        }

        // Check clipboard for new URLs
        if let Some(content) = self.check_clipboard() {
            self.capture_url(&content);
        }

        // If this URL came from clipboard, tag it
        if self.is_from_clipboard(&bookmark.url) {
            bookmark.tags = Self::add_tag(&bookmark.tags, &self.clipboard_tag);
        }

        HookResult::Continue
    }

    fn on_post_add(&self, _ctx: &PluginContext, bookmark: &Bookmark) -> HookResult {
        if !self.enabled {
            return HookResult::Continue;
        }

        // Mark as added if it was from clipboard
        self.mark_added(&bookmark.url);

        HookResult::Continue
    }
}

/// Create an instance of this plugin (required for auto-discovery)
pub fn create_plugin() -> Box<dyn Plugin> {
    Box::new(ClipboardWatcherPlugin::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_url() {
        assert!(ClipboardWatcherPlugin::is_url("https://example.com"));
        assert!(ClipboardWatcherPlugin::is_url("http://example.com"));
        assert!(ClipboardWatcherPlugin::is_url("  https://example.com  "));
        assert!(!ClipboardWatcherPlugin::is_url("not a url"));
        assert!(!ClipboardWatcherPlugin::is_url("ftp://example.com"));
    }

    #[test]
    fn test_capture_url() {
        let plugin = ClipboardWatcherPlugin::new();
        plugin.capture_url("https://example.com");

        let pending = plugin.get_pending();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].url, "https://example.com");
    }

    #[test]
    fn test_mark_added() {
        let plugin = ClipboardWatcherPlugin::new();
        plugin.capture_url("https://example.com");
        plugin.mark_added("https://example.com");

        let pending = plugin.get_pending();
        assert!(pending.is_empty());
    }

    #[test]
    fn test_add_tag() {
        assert_eq!(
            ClipboardWatcherPlugin::add_tag(",rust,", "from-clipboard"),
            ",rust,from-clipboard,"
        );
    }
}
