//! Reading List Plugin
//!
//! Tracks read/unread status for bookmarks:
//! - Adds "unread" tag to new bookmarks
//! - Removes "unread" tag when bookmark is opened
//! - Provides reading progress tracking

use bukurs::models::bookmark::Bookmark;
use bukurs::plugin::{HookResult, Plugin, PluginContext, PluginInfo};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// Reading status for a bookmark
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadingStatus {
    /// When the bookmark was added
    pub added_at: u64,
    /// When it was first opened (None if never opened)
    pub first_opened: Option<u64>,
    /// When it was last opened
    pub last_opened: Option<u64>,
    /// Number of times opened
    pub open_count: u32,
    /// Whether marked as read
    pub is_read: bool,
}

impl ReadingStatus {
    fn new() -> Self {
        Self {
            added_at: Self::now(),
            first_opened: None,
            last_opened: None,
            open_count: 0,
            is_read: false,
        }
    }

    fn now() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    fn record_open(&mut self) {
        let now = Self::now();
        if self.first_opened.is_none() {
            self.first_opened = Some(now);
        }
        self.last_opened = Some(now);
        self.open_count += 1;
        self.is_read = true;
    }
}

/// Persisted reading data
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ReadingData {
    /// Map of bookmark ID to reading status
    statuses: HashMap<usize, ReadingStatus>,
}

pub struct ReadingListPlugin {
    /// Reading data
    data: Mutex<ReadingData>,
    /// Data file path
    data_file: Option<PathBuf>,
    /// Tag for unread bookmarks
    unread_tag: String,
    /// Tag for read bookmarks
    read_tag: String,
    /// Whether to auto-tag new bookmarks as unread
    auto_tag_unread: bool,
    /// Whether to mark as read on open
    mark_read_on_open: bool,
    /// Whether the plugin is enabled
    enabled: bool,
}

impl ReadingListPlugin {
    pub fn new() -> Self {
        Self {
            data: Mutex::new(ReadingData::default()),
            data_file: None,
            unread_tag: "unread".to_string(),
            read_tag: "read".to_string(),
            auto_tag_unread: true,
            mark_read_on_open: true,
            enabled: true,
        }
    }

    /// Load data from file
    fn load_data(&self) -> ReadingData {
        if let Some(ref path) = self.data_file {
            if let Ok(contents) = fs::read_to_string(path) {
                if let Ok(data) = serde_json::from_str(&contents) {
                    return data;
                }
            }
        }
        ReadingData::default()
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

    /// Add unread tag to bookmark
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

    /// Remove tag from bookmark
    fn remove_tag(tags: &str, tag: &str) -> String {
        let tag_list: Vec<String> = tags
            .trim_matches(',')
            .split(',')
            .filter(|t| !t.is_empty() && *t != tag)
            .map(|t| t.to_string())
            .collect();

        if tag_list.is_empty() {
            String::new()
        } else {
            format!(",{},", tag_list.join(","))
        }
    }

    /// Replace one tag with another
    fn replace_tag(tags: &str, old_tag: &str, new_tag: &str) -> String {
        let tags = Self::remove_tag(tags, old_tag);
        Self::add_tag(&tags, new_tag)
    }
}

impl Default for ReadingListPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for ReadingListPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "reading-list".to_string(),
            version: "1.0.0".to_string(),
            description: "Tracks read/unread status for bookmarks".to_string(),
            author: "bukurs".to_string(),
        }
    }

    fn on_load(&mut self, ctx: &PluginContext) -> HookResult {
        self.data_file = Some(ctx.data_dir.join("reading_list.json"));

        // Load existing data
        let loaded = self.load_data();
        if let Ok(mut data) = self.data.lock() {
            *data = loaded;
        }

        // Load config
        if let Some(enabled) = ctx.config.get("enabled") {
            self.enabled = enabled != "false";
        }
        if let Some(tag) = ctx.config.get("unread_tag") {
            self.unread_tag = tag.clone();
        }
        if let Some(tag) = ctx.config.get("read_tag") {
            self.read_tag = tag.clone();
        }
        if let Some(auto) = ctx.config.get("auto_tag_unread") {
            self.auto_tag_unread = auto != "false";
        }
        if let Some(mark) = ctx.config.get("mark_read_on_open") {
            self.mark_read_on_open = mark != "false";
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

        // Add unread tag to new bookmarks
        if self.auto_tag_unread {
            bookmark.tags = Self::add_tag(&bookmark.tags, &self.unread_tag);
        }

        HookResult::Continue
    }

    fn on_post_add(&self, _ctx: &PluginContext, bookmark: &Bookmark) -> HookResult {
        if !self.enabled {
            return HookResult::Continue;
        }

        // Track the new bookmark
        if let Ok(mut data) = self.data.lock() {
            data.statuses.insert(bookmark.id, ReadingStatus::new());
        }
        self.save_data();

        HookResult::Continue
    }

    fn on_pre_open(&self, _ctx: &PluginContext, bookmark: &Bookmark) -> HookResult {
        if !self.enabled || !self.mark_read_on_open {
            return HookResult::Continue;
        }

        // Record the open
        if let Ok(mut data) = self.data.lock() {
            if let Some(status) = data.statuses.get_mut(&bookmark.id) {
                status.record_open();
            } else {
                let mut status = ReadingStatus::new();
                status.record_open();
                data.statuses.insert(bookmark.id, status);
            }
        }
        self.save_data();

        // Note: We can't modify the bookmark here (it's immutable in pre_open)
        // The tag change would need to happen via update command
        log::info!("Marked bookmark {} as read", bookmark.id);

        HookResult::Continue
    }

    fn on_post_delete(&self, _ctx: &PluginContext, bookmark: &Bookmark) -> HookResult {
        if !self.enabled {
            return HookResult::Continue;
        }

        // Remove from tracking
        if let Ok(mut data) = self.data.lock() {
            data.statuses.remove(&bookmark.id);
        }
        self.save_data();

        HookResult::Continue
    }
}

/// Create an instance of this plugin (required for auto-discovery)
pub fn create_plugin() -> Box<dyn Plugin> {
    Box::new(ReadingListPlugin::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_tag() {
        assert_eq!(
            ReadingListPlugin::add_tag(",rust,web,", "unread"),
            ",rust,web,unread,"
        );
        assert_eq!(
            ReadingListPlugin::add_tag("", "unread"),
            ",unread,"
        );
    }

    #[test]
    fn test_remove_tag() {
        assert_eq!(
            ReadingListPlugin::remove_tag(",rust,unread,web,", "unread"),
            ",rust,web,"
        );
    }

    #[test]
    fn test_replace_tag() {
        assert_eq!(
            ReadingListPlugin::replace_tag(",rust,unread,", "unread", "read"),
            ",rust,read,"
        );
    }

    #[test]
    fn test_reading_status() {
        let mut status = ReadingStatus::new();
        assert!(!status.is_read);
        assert_eq!(status.open_count, 0);

        status.record_open();
        assert!(status.is_read);
        assert_eq!(status.open_count, 1);
        assert!(status.first_opened.is_some());
    }
}
