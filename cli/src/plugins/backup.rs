//! Backup Plugin
//!
//! Automatically exports bookmarks to JSON after a configurable number of changes.
//! Keeps multiple backup versions with timestamps.

use bukurs::models::bookmark::Bookmark;
use bukurs::plugin::{HookResult, Plugin, PluginContext, PluginInfo};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Backup state persisted to disk
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct BackupState {
    changes_since_backup: usize,
    last_backup_time: u64,
    total_backups: usize,
}

pub struct BackupPlugin {
    /// Number of changes before auto-backup
    changes_threshold: usize,
    /// Current change count
    change_count: AtomicUsize,
    /// Backup directory
    backup_dir: Option<PathBuf>,
    /// Maximum number of backups to keep
    max_backups: usize,
    /// Whether the plugin is enabled
    enabled: bool,
    /// State file path
    state_file: Option<PathBuf>,
}

impl BackupPlugin {
    pub fn new() -> Self {
        Self {
            changes_threshold: 10,
            change_count: AtomicUsize::new(0),
            backup_dir: None,
            max_backups: 5,
            enabled: true,
            state_file: None,
        }
    }

    /// Get current timestamp
    fn now() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    /// Load state from disk
    fn load_state(&self) -> BackupState {
        if let Some(ref path) = self.state_file {
            if let Ok(contents) = fs::read_to_string(path) {
                if let Ok(state) = serde_json::from_str(&contents) {
                    return state;
                }
            }
        }
        BackupState::default()
    }

    /// Save state to disk
    fn save_state(&self, state: &BackupState) {
        if let Some(ref path) = self.state_file {
            if let Ok(json) = serde_json::to_string_pretty(state) {
                let _ = fs::write(path, json);
            }
        }
    }

    /// Increment change counter and maybe trigger backup
    fn record_change(&self, ctx: &PluginContext) {
        let count = self.change_count.fetch_add(1, Ordering::SeqCst) + 1;

        if count >= self.changes_threshold {
            self.perform_backup(ctx);
            self.change_count.store(0, Ordering::SeqCst);
        }
    }

    /// Perform the backup
    fn perform_backup(&self, ctx: &PluginContext) {
        let backup_dir = match &self.backup_dir {
            Some(dir) => dir.clone(),
            None => return,
        };

        // Create backup directory if needed
        if let Err(e) = fs::create_dir_all(&backup_dir) {
            log::error!("Failed to create backup directory: {}", e);
            return;
        }

        // Read all bookmarks from database
        let db = match bukurs::db::BukuDb::init(&ctx.db_path) {
            Ok(db) => db,
            Err(e) => {
                log::error!("Failed to open database for backup: {}", e);
                return;
            }
        };

        let bookmarks = match db.get_rec_all() {
            Ok(b) => b,
            Err(e) => {
                log::error!("Failed to read bookmarks for backup: {}", e);
                return;
            }
        };

        // Create backup filename with timestamp
        let timestamp = Self::now();
        let backup_file = backup_dir.join(format!("bookmarks_{}.json", timestamp));

        // Serialize and write
        match serde_json::to_string_pretty(&bookmarks) {
            Ok(json) => {
                if let Err(e) = fs::write(&backup_file, json) {
                    log::error!("Failed to write backup: {}", e);
                    return;
                }
                log::info!("Backup created: {:?}", backup_file);
            }
            Err(e) => {
                log::error!("Failed to serialize bookmarks: {}", e);
                return;
            }
        }

        // Update state
        let mut state = self.load_state();
        state.changes_since_backup = 0;
        state.last_backup_time = timestamp;
        state.total_backups += 1;
        self.save_state(&state);

        // Cleanup old backups
        self.cleanup_old_backups(&backup_dir);
    }

    /// Remove old backups beyond max_backups
    fn cleanup_old_backups(&self, backup_dir: &PathBuf) {
        let mut backups: Vec<_> = fs::read_dir(backup_dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("bookmarks_")
            })
            .collect();

        if backups.len() <= self.max_backups {
            return;
        }

        // Sort by name (which contains timestamp) - oldest first
        backups.sort_by_key(|e| e.file_name());

        // Remove oldest backups
        let to_remove = backups.len() - self.max_backups;
        for entry in backups.into_iter().take(to_remove) {
            if let Err(e) = fs::remove_file(entry.path()) {
                log::warn!("Failed to remove old backup {:?}: {}", entry.path(), e);
            } else {
                log::debug!("Removed old backup: {:?}", entry.path());
            }
        }
    }
}

impl Default for BackupPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for BackupPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "backup".to_string(),
            version: "1.0.0".to_string(),
            description: "Auto-exports bookmarks after N changes".to_string(),
            author: "bukurs".to_string(),
        }
    }

    fn on_load(&mut self, ctx: &PluginContext) -> HookResult {
        // Setup backup directory
        self.backup_dir = Some(ctx.data_dir.join("backups"));
        self.state_file = Some(ctx.data_dir.join("backup_state.json"));

        // Load config
        if let Some(enabled) = ctx.config.get("enabled") {
            self.enabled = enabled != "false";
        }
        if let Some(threshold) = ctx.config.get("changes_threshold") {
            self.changes_threshold = threshold.parse().unwrap_or(10);
        }
        if let Some(max) = ctx.config.get("max_backups") {
            self.max_backups = max.parse().unwrap_or(5);
        }

        // Load existing state
        let state = self.load_state();
        self.change_count.store(state.changes_since_backup, Ordering::SeqCst);

        HookResult::Continue
    }

    fn on_unload(&mut self, _ctx: &PluginContext) {
        // Save current change count
        let mut state = self.load_state();
        state.changes_since_backup = self.change_count.load(Ordering::SeqCst);
        self.save_state(&state);
    }

    fn on_post_add(&self, ctx: &PluginContext, _bookmark: &Bookmark) -> HookResult {
        if self.enabled {
            self.record_change(ctx);
        }
        HookResult::Continue
    }

    fn on_post_update(
        &self,
        ctx: &PluginContext,
        _old: &Bookmark,
        _new: &Bookmark,
    ) -> HookResult {
        if self.enabled {
            self.record_change(ctx);
        }
        HookResult::Continue
    }

    fn on_post_delete(&self, ctx: &PluginContext, _bookmark: &Bookmark) -> HookResult {
        if self.enabled {
            self.record_change(ctx);
        }
        HookResult::Continue
    }

    fn on_post_import(&self, ctx: &PluginContext, bookmarks: &[Bookmark]) -> HookResult {
        if self.enabled && !bookmarks.is_empty() {
            // Import counts as multiple changes
            let count = self.change_count.fetch_add(bookmarks.len(), Ordering::SeqCst);
            if count + bookmarks.len() >= self.changes_threshold {
                self.perform_backup(ctx);
                self.change_count.store(0, Ordering::SeqCst);
            }
        }
        HookResult::Continue
    }
}

/// Create an instance of this plugin (required for auto-discovery)
pub fn create_plugin() -> Box<dyn Plugin> {
    Box::new(BackupPlugin::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_info() {
        let plugin = BackupPlugin::new();
        let info = plugin.info();
        assert_eq!(info.name, "backup");
    }

    #[test]
    fn test_default_threshold() {
        let plugin = BackupPlugin::new();
        assert_eq!(plugin.changes_threshold, 10);
    }
}
