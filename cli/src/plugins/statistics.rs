//! Statistics Plugin
//!
//! This plugin tracks bookmark statistics and provides analytics:
//! - Total bookmarks added/updated/deleted during session
//! - Most common tags
//! - Most common domains
//! - Bookmarks per day/week/month statistics
//!
//! Statistics are persisted to a JSON file in the plugin data directory.

use bukurs::models::bookmark::Bookmark;
use bukurs::plugin::{HookResult, Plugin, PluginContext, PluginInfo, SearchContext};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// Statistics data that gets persisted
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StatisticsData {
    /// Total number of bookmarks added (all time)
    pub total_added: u64,
    /// Total number of bookmarks updated (all time)
    pub total_updated: u64,
    /// Total number of bookmarks deleted (all time)
    pub total_deleted: u64,
    /// Total searches performed
    pub total_searches: u64,
    /// Tag frequency counts
    pub tag_counts: HashMap<String, u64>,
    /// Domain frequency counts
    pub domain_counts: HashMap<String, u64>,
    /// Timestamps of add operations (for time-based stats)
    pub add_timestamps: Vec<u64>,
    /// Last updated timestamp
    pub last_updated: u64,
}

impl StatisticsData {
    /// Get the most common tags (sorted by frequency)
    pub fn top_tags(&self, limit: usize) -> Vec<(&String, &u64)> {
        let mut tags: Vec<_> = self.tag_counts.iter().collect();
        tags.sort_by(|a, b| b.1.cmp(a.1));
        tags.into_iter().take(limit).collect()
    }

    /// Get the most common domains (sorted by frequency)
    pub fn top_domains(&self, limit: usize) -> Vec<(&String, &u64)> {
        let mut domains: Vec<_> = self.domain_counts.iter().collect();
        domains.sort_by(|a, b| b.1.cmp(a.1));
        domains.into_iter().take(limit).collect()
    }

    /// Get bookmarks added in the last N days
    pub fn added_in_last_days(&self, days: u64) -> u64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let cutoff = now.saturating_sub(days * 24 * 60 * 60);

        self.add_timestamps.iter().filter(|&&ts| ts >= cutoff).count() as u64
    }

    /// Generate a summary report
    pub fn summary(&self) -> String {
        let mut report = String::new();

        report.push_str("=== Bookmark Statistics ===\n\n");
        report.push_str(&format!("Total Added:   {}\n", self.total_added));
        report.push_str(&format!("Total Updated: {}\n", self.total_updated));
        report.push_str(&format!("Total Deleted: {}\n", self.total_deleted));
        report.push_str(&format!("Total Searches: {}\n\n", self.total_searches));

        report.push_str("--- Recent Activity ---\n");
        report.push_str(&format!("Added Today:      {}\n", self.added_in_last_days(1)));
        report.push_str(&format!("Added This Week:  {}\n", self.added_in_last_days(7)));
        report.push_str(&format!("Added This Month: {}\n\n", self.added_in_last_days(30)));

        report.push_str("--- Top 10 Tags ---\n");
        for (tag, count) in self.top_tags(10) {
            report.push_str(&format!("  {}: {}\n", tag, count));
        }

        report.push_str("\n--- Top 10 Domains ---\n");
        for (domain, count) in self.top_domains(10) {
            report.push_str(&format!("  {}: {}\n", domain, count));
        }

        report
    }
}

/// Statistics plugin for tracking bookmark analytics
pub struct StatisticsPlugin {
    /// Statistics data (thread-safe)
    data: Arc<Mutex<StatisticsData>>,
    /// Path to the data file
    data_file: Option<PathBuf>,
    /// Whether to persist data
    persist: bool,
}

impl StatisticsPlugin {
    pub fn new() -> Self {
        Self {
            data: Arc::new(Mutex::new(StatisticsData::default())),
            data_file: None,
            persist: true,
        }
    }

    /// Create a plugin that doesn't persist data (for testing)
    pub fn in_memory() -> Self {
        Self {
            data: Arc::new(Mutex::new(StatisticsData::default())),
            data_file: None,
            persist: false,
        }
    }

    /// Load statistics from file
    fn load_data(&self) -> StatisticsData {
        if let Some(ref path) = self.data_file {
            if path.exists() {
                if let Ok(contents) = fs::read_to_string(path) {
                    if let Ok(data) = serde_json::from_str(&contents) {
                        return data;
                    }
                }
            }
        }
        StatisticsData::default()
    }

    /// Save statistics to file
    fn save_data(&self) {
        if !self.persist {
            return;
        }

        if let Some(ref path) = self.data_file {
            if let Ok(data) = self.data.lock() {
                if let Ok(json) = serde_json::to_string_pretty(&*data) {
                    let _ = fs::write(path, json);
                }
            }
        }
    }

    /// Get the current timestamp
    fn now() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    /// Extract domain from URL
    fn extract_domain(url: &str) -> Option<String> {
        // Simple domain extraction
        let url = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://"))?;
        let url = url.strip_prefix("www.").unwrap_or(url);
        url.split('/').next().map(|s| s.to_string())
    }

    /// Parse tags from bukurs format
    fn parse_tags(tags: &str) -> Vec<String> {
        tags.trim_matches(',')
            .split(',')
            .filter(|t| !t.is_empty())
            .map(|t| t.to_string())
            .collect()
    }

    /// Record tags from a bookmark
    fn record_tags(&self, tags: &str) {
        if let Ok(mut data) = self.data.lock() {
            for tag in Self::parse_tags(tags) {
                *data.tag_counts.entry(tag).or_insert(0) += 1;
            }
        }
    }

    /// Record domain from a bookmark
    fn record_domain(&self, url: &str) {
        if let Some(domain) = Self::extract_domain(url) {
            if let Ok(mut data) = self.data.lock() {
                *data.domain_counts.entry(domain).or_insert(0) += 1;
            }
        }
    }

    /// Get current statistics (for display/commands)
    pub fn get_stats(&self) -> StatisticsData {
        self.data.lock().map(|d| d.clone()).unwrap_or_default()
    }

    /// Get summary report
    pub fn get_summary(&self) -> String {
        self.get_stats().summary()
    }
}

impl Default for StatisticsPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for StatisticsPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "statistics".to_string(),
            version: "1.0.0".to_string(),
            description: "Tracks bookmark statistics and analytics".to_string(),
            author: "bukurs".to_string(),
        }
    }

    fn on_load(&mut self, ctx: &PluginContext) -> HookResult {
        // Set up data file path
        let data_file = ctx.data_dir.join("statistics.json");
        self.data_file = Some(data_file);

        // Load existing data
        let loaded = self.load_data();
        if let Ok(mut data) = self.data.lock() {
            *data = loaded;
        }

        // Check persist config
        if let Some(persist) = ctx.config.get("persist") {
            self.persist = persist != "false";
        }

        HookResult::Continue
    }

    fn on_unload(&mut self, _ctx: &PluginContext) {
        self.save_data();
    }

    fn on_post_add(&self, _ctx: &PluginContext, bookmark: &Bookmark) -> HookResult {
        if let Ok(mut data) = self.data.lock() {
            data.total_added += 1;
            data.add_timestamps.push(Self::now());
            data.last_updated = Self::now();
        }

        self.record_tags(&bookmark.tags);
        self.record_domain(&bookmark.url);
        self.save_data();

        HookResult::Continue
    }

    fn on_post_update(
        &self,
        _ctx: &PluginContext,
        old: &Bookmark,
        new: &Bookmark,
    ) -> HookResult {
        if let Ok(mut data) = self.data.lock() {
            data.total_updated += 1;
            data.last_updated = Self::now();
        }

        // Update tag counts if tags changed
        if old.tags != new.tags {
            // Decrement old tags
            let old_tags = Self::parse_tags(&old.tags);
            if let Ok(mut data) = self.data.lock() {
                for tag in old_tags {
                    if let Some(count) = data.tag_counts.get_mut(&tag) {
                        *count = count.saturating_sub(1);
                    }
                }
            }
            // Increment new tags
            self.record_tags(&new.tags);
        }

        // Update domain counts if URL changed
        if old.url != new.url {
            if let Some(old_domain) = Self::extract_domain(&old.url) {
                if let Ok(mut data) = self.data.lock() {
                    if let Some(count) = data.domain_counts.get_mut(&old_domain) {
                        *count = count.saturating_sub(1);
                    }
                }
            }
            self.record_domain(&new.url);
        }

        self.save_data();
        HookResult::Continue
    }

    fn on_post_delete(&self, _ctx: &PluginContext, bookmark: &Bookmark) -> HookResult {
        if let Ok(mut data) = self.data.lock() {
            data.total_deleted += 1;
            data.last_updated = Self::now();

            // Decrement tag counts
            for tag in Self::parse_tags(&bookmark.tags) {
                if let Some(count) = data.tag_counts.get_mut(&tag) {
                    *count = count.saturating_sub(1);
                }
            }

            // Decrement domain count
            if let Some(domain) = Self::extract_domain(&bookmark.url) {
                if let Some(count) = data.domain_counts.get_mut(&domain) {
                    *count = count.saturating_sub(1);
                }
            }
        }

        self.save_data();
        HookResult::Continue
    }

    fn on_post_search(
        &self,
        _ctx: &PluginContext,
        _search_ctx: &SearchContext,
        _results: &mut Vec<Bookmark>,
    ) -> HookResult {
        if let Ok(mut data) = self.data.lock() {
            data.total_searches += 1;
            data.last_updated = Self::now();
        }

        self.save_data();
        HookResult::Continue
    }
}

/// Create an instance of this plugin (required for auto-discovery)
pub fn create_plugin() -> Box<dyn Plugin> {
    Box::new(StatisticsPlugin::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_domain() {
        assert_eq!(
            StatisticsPlugin::extract_domain("https://github.com/user/repo"),
            Some("github.com".to_string())
        );
        assert_eq!(
            StatisticsPlugin::extract_domain("https://www.example.com/page"),
            Some("example.com".to_string())
        );
        assert_eq!(
            StatisticsPlugin::extract_domain("http://docs.rs/serde"),
            Some("docs.rs".to_string())
        );
    }

    #[test]
    fn test_statistics_tracking() {
        let plugin = StatisticsPlugin::in_memory();
        let ctx = PluginContext::new(
            PathBuf::from("/test/db"),
            PathBuf::from("/test/data"),
        );

        let bookmark = Bookmark::new(
            1,
            "https://github.com/test".to_string(),
            "Test".to_string(),
            ",rust,code,".to_string(),
            "".to_string(),
        );

        plugin.on_post_add(&ctx, &bookmark);

        let stats = plugin.get_stats();
        assert_eq!(stats.total_added, 1);
        assert_eq!(stats.tag_counts.get("rust"), Some(&1));
        assert_eq!(stats.domain_counts.get("github.com"), Some(&1));
    }

    #[test]
    fn test_top_tags() {
        let mut data = StatisticsData::default();
        data.tag_counts.insert("rust".to_string(), 10);
        data.tag_counts.insert("python".to_string(), 5);
        data.tag_counts.insert("java".to_string(), 3);

        let top = data.top_tags(2);
        assert_eq!(top.len(), 2);
        assert_eq!(*top[0].0, "rust");
        assert_eq!(*top[1].0, "python");
    }

    #[test]
    fn test_delete_decrements_counts() {
        let plugin = StatisticsPlugin::in_memory();
        let ctx = PluginContext::new(
            PathBuf::from("/test/db"),
            PathBuf::from("/test/data"),
        );

        // Add two bookmarks with same tag
        let bookmark1 = Bookmark::new(
            1,
            "https://github.com/test1".to_string(),
            "Test 1".to_string(),
            ",rust,".to_string(),
            "".to_string(),
        );
        let bookmark2 = Bookmark::new(
            2,
            "https://github.com/test2".to_string(),
            "Test 2".to_string(),
            ",rust,".to_string(),
            "".to_string(),
        );

        plugin.on_post_add(&ctx, &bookmark1);
        plugin.on_post_add(&ctx, &bookmark2);

        assert_eq!(plugin.get_stats().tag_counts.get("rust"), Some(&2));

        // Delete one
        plugin.on_post_delete(&ctx, &bookmark1);

        assert_eq!(plugin.get_stats().tag_counts.get("rust"), Some(&1));
        assert_eq!(plugin.get_stats().total_deleted, 1);
    }

    #[test]
    fn test_summary_report() {
        let plugin = StatisticsPlugin::in_memory();
        let ctx = PluginContext::new(
            PathBuf::from("/test/db"),
            PathBuf::from("/test/data"),
        );

        let bookmark = Bookmark::new(
            1,
            "https://github.com/test".to_string(),
            "Test".to_string(),
            ",rust,".to_string(),
            "".to_string(),
        );

        plugin.on_post_add(&ctx, &bookmark);

        let summary = plugin.get_summary();
        assert!(summary.contains("Total Added:   1"));
        assert!(summary.contains("rust: 1"));
        assert!(summary.contains("github.com: 1"));
    }
}
