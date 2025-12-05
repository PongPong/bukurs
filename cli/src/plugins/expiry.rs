//! Expiry Plugin
//!
//! Manages bookmark expiration:
//! - Auto-tags bookmarks that are older than N days
//! - Can be configured to warn about or auto-delete expired bookmarks
//! - Useful for temporary bookmarks or time-sensitive content

use bukurs::models::bookmark::Bookmark;
use bukurs::plugin::{HookResult, Plugin, PluginContext, PluginInfo, SearchContext};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// Expiry data for a bookmark
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpiryInfo {
    /// When the bookmark was added
    pub added_at: u64,
    /// Custom expiry days (overrides global setting)
    pub expires_in_days: Option<u64>,
    /// Whether this bookmark should never expire
    pub never_expires: bool,
}

impl ExpiryInfo {
    fn new() -> Self {
        Self {
            added_at: Self::now(),
            expires_in_days: None,
            never_expires: false,
        }
    }

    fn now() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    fn is_expired(&self, default_days: u64) -> bool {
        if self.never_expires {
            return false;
        }

        let days = self.expires_in_days.unwrap_or(default_days);
        let expiry_time = self.added_at + (days * 24 * 60 * 60);
        Self::now() > expiry_time
    }

    fn days_until_expiry(&self, default_days: u64) -> i64 {
        if self.never_expires {
            return i64::MAX;
        }

        let days = self.expires_in_days.unwrap_or(default_days);
        let expiry_time = self.added_at + (days * 24 * 60 * 60);
        let now = Self::now();

        if now > expiry_time {
            -(((now - expiry_time) / (24 * 60 * 60)) as i64)
        } else {
            ((expiry_time - now) / (24 * 60 * 60)) as i64
        }
    }
}

/// Persisted expiry data
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ExpiryData {
    bookmarks: HashMap<usize, ExpiryInfo>,
}

pub struct ExpiryPlugin {
    /// Expiry data
    data: Mutex<ExpiryData>,
    /// Data file path
    data_file: Option<PathBuf>,
    /// Default expiry in days
    default_expiry_days: u64,
    /// Tag to add to expired bookmarks
    expired_tag: String,
    /// Tag to add to soon-expiring bookmarks
    expiring_soon_tag: String,
    /// Days before expiry to add "expiring soon" tag
    expiring_soon_days: u64,
    /// Whether to auto-tag expired bookmarks
    auto_tag_expired: bool,
    /// Whether the plugin is enabled
    enabled: bool,
    /// Tag that marks a bookmark as "keep forever"
    keep_forever_tag: String,
}

impl ExpiryPlugin {
    pub fn new() -> Self {
        Self {
            data: Mutex::new(ExpiryData::default()),
            data_file: None,
            default_expiry_days: 30, // Default: bookmarks expire after 30 days
            expired_tag: "expired".to_string(),
            expiring_soon_tag: "expiring-soon".to_string(),
            expiring_soon_days: 7,
            auto_tag_expired: true,
            enabled: false, // Disabled by default - expiry is opt-in
            keep_forever_tag: "keep".to_string(),
        }
    }

    /// Load data from file
    fn load_data(&self) -> ExpiryData {
        if let Some(ref path) = self.data_file {
            if let Ok(contents) = fs::read_to_string(path) {
                if let Ok(data) = serde_json::from_str(&contents) {
                    return data;
                }
            }
        }
        ExpiryData::default()
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

    /// Check if bookmark has keep-forever tag
    fn has_keep_tag(tags: &str, keep_tag: &str) -> bool {
        tags.trim_matches(',')
            .split(',')
            .any(|t| t == keep_tag)
    }

    /// Add tag to bookmark
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
}

impl Default for ExpiryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for ExpiryPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "expiry".to_string(),
            version: "1.0.0".to_string(),
            description: "Auto-expires bookmarks after N days".to_string(),
            author: "bukurs".to_string(),
        }
    }

    fn on_load(&mut self, ctx: &PluginContext) -> HookResult {
        self.data_file = Some(ctx.data_dir.join("expiry.json"));

        // Load existing data
        let loaded = self.load_data();
        if let Ok(mut data) = self.data.lock() {
            *data = loaded;
        }

        // Load config
        if let Some(enabled) = ctx.config.get("enabled") {
            self.enabled = enabled == "true";
        }
        if let Some(days) = ctx.config.get("default_expiry_days") {
            self.default_expiry_days = days.parse().unwrap_or(30);
        }
        if let Some(tag) = ctx.config.get("expired_tag") {
            self.expired_tag = tag.clone();
        }
        if let Some(tag) = ctx.config.get("expiring_soon_tag") {
            self.expiring_soon_tag = tag.clone();
        }
        if let Some(days) = ctx.config.get("expiring_soon_days") {
            self.expiring_soon_days = days.parse().unwrap_or(7);
        }
        if let Some(tag) = ctx.config.get("keep_forever_tag") {
            self.keep_forever_tag = tag.clone();
        }

        HookResult::Continue
    }

    fn on_unload(&mut self, _ctx: &PluginContext) {
        self.save_data();
    }

    fn on_post_add(&self, _ctx: &PluginContext, bookmark: &Bookmark) -> HookResult {
        if !self.enabled {
            return HookResult::Continue;
        }

        // Track the new bookmark
        let mut info = ExpiryInfo::new();

        // Check if it has keep-forever tag
        if Self::has_keep_tag(&bookmark.tags, &self.keep_forever_tag) {
            info.never_expires = true;
        }

        if let Ok(mut data) = self.data.lock() {
            data.bookmarks.insert(bookmark.id, info);
        }
        self.save_data();

        HookResult::Continue
    }

    fn on_post_update(
        &self,
        _ctx: &PluginContext,
        _old: &Bookmark,
        new: &Bookmark,
    ) -> HookResult {
        if !self.enabled {
            return HookResult::Continue;
        }

        // Update never_expires based on keep tag
        if let Ok(mut data) = self.data.lock() {
            if let Some(info) = data.bookmarks.get_mut(&new.id) {
                info.never_expires = Self::has_keep_tag(&new.tags, &self.keep_forever_tag);
            }
        }
        self.save_data();

        HookResult::Continue
    }

    fn on_post_delete(&self, _ctx: &PluginContext, bookmark: &Bookmark) -> HookResult {
        if !self.enabled {
            return HookResult::Continue;
        }

        // Remove from tracking
        if let Ok(mut data) = self.data.lock() {
            data.bookmarks.remove(&bookmark.id);
        }
        self.save_data();

        HookResult::Continue
    }

    fn on_post_search(
        &self,
        _ctx: &PluginContext,
        _search_ctx: &SearchContext,
        results: &mut Vec<Bookmark>,
    ) -> HookResult {
        if !self.enabled || !self.auto_tag_expired {
            return HookResult::Continue;
        }

        // Check and tag expired/expiring bookmarks
        if let Ok(data) = self.data.lock() {
            for bookmark in results.iter_mut() {
                if let Some(info) = data.bookmarks.get(&bookmark.id) {
                    if info.is_expired(self.default_expiry_days) {
                        bookmark.tags = Self::add_tag(&bookmark.tags, &self.expired_tag);
                        bookmark.tags = Self::remove_tag(&bookmark.tags, &self.expiring_soon_tag);
                    } else {
                        let days_left = info.days_until_expiry(self.default_expiry_days);
                        if days_left <= self.expiring_soon_days as i64 && days_left > 0 {
                            bookmark.tags = Self::add_tag(&bookmark.tags, &self.expiring_soon_tag);
                        }
                    }
                }
            }
        }

        HookResult::Continue
    }
}

/// Create an instance of this plugin (required for auto-discovery)
pub fn create_plugin() -> Box<dyn Plugin> {
    Box::new(ExpiryPlugin::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expiry_info() {
        let info = ExpiryInfo::new();
        assert!(!info.is_expired(30));

        // Test never_expires
        let mut info = ExpiryInfo::new();
        info.never_expires = true;
        assert!(!info.is_expired(0)); // Even with 0 days, never expires
    }

    #[test]
    fn test_has_keep_tag() {
        assert!(ExpiryPlugin::has_keep_tag(",keep,rust,", "keep"));
        assert!(!ExpiryPlugin::has_keep_tag(",rust,web,", "keep"));
    }

    #[test]
    fn test_add_remove_tag() {
        let tags = ",rust,web,";
        let with_expired = ExpiryPlugin::add_tag(tags, "expired");
        assert!(with_expired.contains("expired"));

        let without = ExpiryPlugin::remove_tag(&with_expired, "expired");
        assert!(!without.contains("expired"));
    }

    #[test]
    fn test_plugin_disabled_by_default() {
        let plugin = ExpiryPlugin::new();
        assert!(!plugin.enabled);
    }
}
