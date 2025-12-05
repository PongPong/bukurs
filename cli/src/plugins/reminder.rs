//! Reminder Plugin
//!
//! Set reminders to revisit bookmarks:
//! - Tag with "remind:3d" to be reminded in 3 days
//! - Tag with "remind:1w" to be reminded in 1 week
//! - Tag with "remind:2024-12-25" for a specific date
//! - Shows due reminders on search/print

use bukurs::models::bookmark::Bookmark;
use bukurs::plugin::{HookResult, Plugin, PluginContext, PluginInfo, SearchContext};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

/// Reminder data for a bookmark
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reminder {
    /// Bookmark ID
    pub bookmark_id: usize,
    /// When the reminder is due
    pub due_at: DateTime<Utc>,
    /// Original reminder tag (for display)
    pub reminder_tag: String,
    /// Whether the reminder has been acknowledged
    pub acknowledged: bool,
}

/// Persisted reminder data
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ReminderData {
    reminders: HashMap<usize, Reminder>,
}

pub struct ReminderPlugin {
    /// Reminder data
    data: Mutex<ReminderData>,
    /// Data file path
    data_file: Option<PathBuf>,
    /// Tag prefix for reminders
    reminder_prefix: String,
    /// Tag for due reminders
    due_tag: String,
    /// Whether the plugin is enabled
    enabled: bool,
}

impl ReminderPlugin {
    pub fn new() -> Self {
        Self {
            data: Mutex::new(ReminderData::default()),
            data_file: None,
            reminder_prefix: "remind:".to_string(),
            due_tag: "reminder-due".to_string(),
            enabled: true,
        }
    }

    /// Load data from file
    fn load_data(&self) -> ReminderData {
        if let Some(ref path) = self.data_file {
            if let Ok(contents) = fs::read_to_string(path) {
                if let Ok(data) = serde_json::from_str(&contents) {
                    return data;
                }
            }
        }
        ReminderData::default()
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

    /// Parse reminder tag and return due date
    /// Formats:
    /// - "remind:3d" - 3 days from now
    /// - "remind:1w" - 1 week from now
    /// - "remind:2h" - 2 hours from now
    /// - "remind:2024-12-25" - specific date
    fn parse_reminder(tag: &str, prefix: &str) -> Option<DateTime<Utc>> {
        let value = tag.strip_prefix(prefix)?;

        // Try parsing as duration (3d, 1w, 2h, etc.)
        if let Some(duration) = Self::parse_duration(value) {
            return Some(Utc::now() + duration);
        }

        // Try parsing as date (YYYY-MM-DD)
        if let Ok(date) = NaiveDate::parse_from_str(value, "%Y-%m-%d") {
            let datetime = date.and_hms_opt(9, 0, 0)?; // 9 AM
            return Some(DateTime::from_naive_utc_and_offset(datetime, Utc));
        }

        None
    }

    /// Parse duration string (3d, 1w, 2h, 30m)
    fn parse_duration(s: &str) -> Option<Duration> {
        let s = s.trim().to_lowercase();
        if s.is_empty() {
            return None;
        }

        let (num_str, unit) = s.split_at(s.len() - 1);
        let num: i64 = num_str.parse().ok()?;

        match unit {
            "m" => Some(Duration::minutes(num)),
            "h" => Some(Duration::hours(num)),
            "d" => Some(Duration::days(num)),
            "w" => Some(Duration::weeks(num)),
            _ => None,
        }
    }

    /// Extract reminder tags from bookmark tags
    fn extract_reminder_tags<'a>(&self, tags: &'a str) -> Vec<&'a str> {
        tags.trim_matches(',')
            .split(',')
            .filter(|t| t.starts_with(&self.reminder_prefix))
            .collect()
    }

    /// Check if a reminder is due
    fn is_due(reminder: &Reminder) -> bool {
        !reminder.acknowledged && Utc::now() >= reminder.due_at
    }

    /// Format time until due
    fn format_time_until(due: DateTime<Utc>) -> String {
        let now = Utc::now();
        if due <= now {
            return "now".to_string();
        }

        let diff = due - now;
        let days = diff.num_days();
        let hours = diff.num_hours() % 24;
        let minutes = diff.num_minutes() % 60;

        if days > 0 {
            format!("{}d {}h", days, hours)
        } else if hours > 0 {
            format!("{}h {}m", hours, minutes)
        } else {
            format!("{}m", minutes)
        }
    }

    /// Add due tag to bookmark tags
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

impl Default for ReminderPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for ReminderPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "reminder".to_string(),
            version: "1.0.0".to_string(),
            description: "Set reminders to revisit bookmarks".to_string(),
            author: "bukurs".to_string(),
        }
    }

    fn on_load(&mut self, ctx: &PluginContext) -> HookResult {
        self.data_file = Some(ctx.data_dir.join("reminders.json"));

        // Load existing data
        let loaded = self.load_data();
        if let Ok(mut data) = self.data.lock() {
            *data = loaded;
        }

        // Load config
        if let Some(enabled) = ctx.config.get("enabled") {
            self.enabled = enabled != "false";
        }
        if let Some(prefix) = ctx.config.get("reminder_prefix") {
            self.reminder_prefix = prefix.clone();
        }
        if let Some(tag) = ctx.config.get("due_tag") {
            self.due_tag = tag.clone();
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

        // Check for reminder tags
        let reminder_tags = self.extract_reminder_tags(&bookmark.tags);

        for tag in reminder_tags {
            if let Some(due_at) = Self::parse_reminder(tag, &self.reminder_prefix) {
                let reminder = Reminder {
                    bookmark_id: bookmark.id,
                    due_at,
                    reminder_tag: tag.to_string(),
                    acknowledged: false,
                };

                if let Ok(mut data) = self.data.lock() {
                    data.reminders.insert(bookmark.id, reminder);
                }

                log::info!(
                    "Reminder set for bookmark {}: due in {}",
                    bookmark.id,
                    Self::format_time_until(due_at)
                );
            }
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

        // Check for new reminder tags
        let reminder_tags = self.extract_reminder_tags(&new.tags);

        if reminder_tags.is_empty() {
            // Remove any existing reminder
            if let Ok(mut data) = self.data.lock() {
                data.reminders.remove(&new.id);
            }
        } else {
            // Update/add reminder
            for tag in reminder_tags {
                if let Some(due_at) = Self::parse_reminder(tag, &self.reminder_prefix) {
                    let reminder = Reminder {
                        bookmark_id: new.id,
                        due_at,
                        reminder_tag: tag.to_string(),
                        acknowledged: false,
                    };

                    if let Ok(mut data) = self.data.lock() {
                        data.reminders.insert(new.id, reminder);
                    }
                }
            }
        }

        self.save_data();
        HookResult::Continue
    }

    fn on_post_delete(&self, _ctx: &PluginContext, bookmark: &Bookmark) -> HookResult {
        if !self.enabled {
            return HookResult::Continue;
        }

        // Remove reminder
        if let Ok(mut data) = self.data.lock() {
            data.reminders.remove(&bookmark.id);
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
        if !self.enabled {
            return HookResult::Continue;
        }

        // Check for due reminders and tag them
        if let Ok(data) = self.data.lock() {
            for bookmark in results.iter_mut() {
                if let Some(reminder) = data.reminders.get(&bookmark.id) {
                    if Self::is_due(reminder) {
                        bookmark.tags = Self::add_tag(&bookmark.tags, &self.due_tag);
                    }
                }
            }
        }

        HookResult::Continue
    }

    fn on_pre_open(&self, _ctx: &PluginContext, bookmark: &Bookmark) -> HookResult {
        if !self.enabled {
            return HookResult::Continue;
        }

        // Acknowledge reminder when bookmark is opened
        if let Ok(mut data) = self.data.lock() {
            if let Some(reminder) = data.reminders.get_mut(&bookmark.id) {
                if Self::is_due(reminder) {
                    reminder.acknowledged = true;
                    log::info!("Reminder acknowledged for bookmark {}", bookmark.id);
                }
            }
        }
        self.save_data();

        HookResult::Continue
    }
}

/// Create an instance of this plugin (required for auto-discovery)
pub fn create_plugin() -> Box<dyn Plugin> {
    Box::new(ReminderPlugin::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_duration() {
        assert_eq!(ReminderPlugin::parse_duration("3d"), Some(Duration::days(3)));
        assert_eq!(ReminderPlugin::parse_duration("1w"), Some(Duration::weeks(1)));
        assert_eq!(ReminderPlugin::parse_duration("2h"), Some(Duration::hours(2)));
        assert_eq!(ReminderPlugin::parse_duration("30m"), Some(Duration::minutes(30)));
        assert_eq!(ReminderPlugin::parse_duration("invalid"), None);
    }

    #[test]
    fn test_parse_reminder_duration() {
        let prefix = "remind:";
        let due = ReminderPlugin::parse_reminder("remind:1d", prefix).unwrap();
        let expected = Utc::now() + Duration::days(1);
        // Allow 1 second difference
        assert!((due - expected).num_seconds().abs() < 1);
    }

    #[test]
    fn test_parse_reminder_date() {
        let prefix = "remind:";
        let due = ReminderPlugin::parse_reminder("remind:2025-12-25", prefix).unwrap();
        assert_eq!(due.date_naive(), NaiveDate::from_ymd_opt(2025, 12, 25).unwrap());
    }

    #[test]
    fn test_extract_reminder_tags() {
        let plugin = ReminderPlugin::new();
        let tags = ",rust,remind:3d,web,remind:1w,";
        let reminder_tags = plugin.extract_reminder_tags(tags);
        assert_eq!(reminder_tags.len(), 2);
        assert!(reminder_tags.contains(&"remind:3d"));
        assert!(reminder_tags.contains(&"remind:1w"));
    }

    #[test]
    fn test_format_time_until() {
        let future = Utc::now() + Duration::days(2) + Duration::hours(5);
        let formatted = ReminderPlugin::format_time_until(future);
        assert!(formatted.contains("2d"));
    }
}
