//! Notes Plugin
//!
//! Add timestamped notes to bookmarks:
//! - Attach multiple notes to any bookmark
//! - Notes are stored separately from bookmark description
//! - Each note has a timestamp and optional author
//! - Notes persist across bookmark updates

use bukurs::models::bookmark::Bookmark;
use bukurs::plugin::{HookResult, Plugin, PluginContext, PluginInfo};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// A note attached to a bookmark
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    /// Note content
    pub content: String,
    /// When the note was created (Unix timestamp)
    pub created_at: u64,
    /// Optional author/source of the note
    pub author: Option<String>,
}

/// Persisted notes data
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct NotesData {
    /// Map of bookmark ID to list of notes
    notes: HashMap<usize, Vec<Note>>,
}

pub struct NotesPlugin {
    /// Notes data
    data: Mutex<NotesData>,
    /// Data file path
    data_file: Option<PathBuf>,
    /// Whether the plugin is enabled
    enabled: bool,
    /// Default author for new notes
    default_author: Option<String>,
}

impl NotesPlugin {
    pub fn new() -> Self {
        Self {
            data: Mutex::new(NotesData::default()),
            data_file: None,
            enabled: true,
            default_author: None,
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
    fn load_data(&self) -> NotesData {
        if let Some(ref path) = self.data_file {
            if let Ok(contents) = fs::read_to_string(path) {
                if let Ok(data) = serde_json::from_str(&contents) {
                    return data;
                }
            }
        }
        NotesData::default()
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

    /// Add a note to a bookmark
    pub fn add_note(&self, bookmark_id: usize, content: &str, author: Option<String>) {
        let note = Note {
            content: content.to_string(),
            created_at: Self::now(),
            author: author.or_else(|| self.default_author.clone()),
        };

        if let Ok(mut data) = self.data.lock() {
            data.notes
                .entry(bookmark_id)
                .or_insert_with(Vec::new)
                .push(note);
        }

        self.save_data();
    }

    /// Get all notes for a bookmark
    pub fn get_notes(&self, bookmark_id: usize) -> Vec<Note> {
        if let Ok(data) = self.data.lock() {
            data.notes.get(&bookmark_id).cloned().unwrap_or_default()
        } else {
            Vec::new()
        }
    }

    /// Delete a specific note by index
    pub fn delete_note(&self, bookmark_id: usize, note_index: usize) -> bool {
        let deleted = if let Ok(mut data) = self.data.lock() {
            if let Some(notes) = data.notes.get_mut(&bookmark_id) {
                if note_index < notes.len() {
                    notes.remove(note_index);
                    true
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        };

        if deleted {
            self.save_data();
        }

        deleted
    }

    /// Delete all notes for a bookmark
    pub fn delete_all_notes(&self, bookmark_id: usize) {
        if let Ok(mut data) = self.data.lock() {
            data.notes.remove(&bookmark_id);
        }
        self.save_data();
    }

    /// Get note count for a bookmark
    pub fn note_count(&self, bookmark_id: usize) -> usize {
        if let Ok(data) = self.data.lock() {
            data.notes.get(&bookmark_id).map(|n| n.len()).unwrap_or(0)
        } else {
            0
        }
    }

    /// Format timestamp as human-readable string
    pub fn format_timestamp(timestamp: u64) -> String {
        use chrono::{DateTime, Utc};
        let dt = DateTime::from_timestamp(timestamp as i64, 0)
            .unwrap_or_else(|| DateTime::<Utc>::MIN_UTC);
        dt.format("%Y-%m-%d %H:%M").to_string()
    }

    /// Format notes for display
    pub fn format_notes(&self, bookmark_id: usize) -> String {
        let notes = self.get_notes(bookmark_id);
        if notes.is_empty() {
            return String::new();
        }

        let mut output = String::new();
        output.push_str(&format!("Notes ({}):\n", notes.len()));

        for (i, note) in notes.iter().enumerate() {
            let timestamp = Self::format_timestamp(note.created_at);
            let author_str = note
                .author
                .as_ref()
                .map(|a| format!(" by {}", a))
                .unwrap_or_default();

            output.push_str(&format!(
                "  [{}] {}{}\n",
                i + 1,
                timestamp,
                author_str
            ));
            output.push_str(&format!("      {}\n", note.content));
        }

        output
    }
}

impl Default for NotesPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for NotesPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "notes".to_string(),
            version: "1.0.0".to_string(),
            description: "Add timestamped notes to bookmarks".to_string(),
            author: "bukurs".to_string(),
        }
    }

    fn on_load(&mut self, ctx: &PluginContext) -> HookResult {
        self.data_file = Some(ctx.data_dir.join("notes.json"));

        // Load existing data
        let loaded = self.load_data();
        if let Ok(mut data) = self.data.lock() {
            *data = loaded;
        }

        // Load config
        if let Some(enabled) = ctx.config.get("enabled") {
            self.enabled = enabled != "false";
        }
        if let Some(author) = ctx.config.get("default_author") {
            self.default_author = Some(author.clone());
        }

        HookResult::Continue
    }

    fn on_unload(&mut self, _ctx: &PluginContext) {
        self.save_data();
    }

    fn on_post_delete(&self, _ctx: &PluginContext, bookmark: &Bookmark) -> HookResult {
        if !self.enabled {
            return HookResult::Continue;
        }

        // Clean up notes when bookmark is deleted
        self.delete_all_notes(bookmark.id);

        HookResult::Continue
    }
}

/// Create an instance of this plugin (required for auto-discovery)
pub fn create_plugin() -> Box<dyn Plugin> {
    Box::new(NotesPlugin::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_and_get_notes() {
        let plugin = NotesPlugin::new();

        plugin.add_note(1, "First note", None);
        plugin.add_note(1, "Second note", Some("Alice".to_string()));

        let notes = plugin.get_notes(1);
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0].content, "First note");
        assert_eq!(notes[1].content, "Second note");
        assert_eq!(notes[1].author, Some("Alice".to_string()));
    }

    #[test]
    fn test_delete_note() {
        let plugin = NotesPlugin::new();

        plugin.add_note(1, "Note 1", None);
        plugin.add_note(1, "Note 2", None);
        plugin.add_note(1, "Note 3", None);

        assert!(plugin.delete_note(1, 1)); // Delete "Note 2"

        let notes = plugin.get_notes(1);
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0].content, "Note 1");
        assert_eq!(notes[1].content, "Note 3");
    }

    #[test]
    fn test_note_count() {
        let plugin = NotesPlugin::new();

        assert_eq!(plugin.note_count(1), 0);

        plugin.add_note(1, "Note", None);
        assert_eq!(plugin.note_count(1), 1);

        plugin.add_note(1, "Another", None);
        assert_eq!(plugin.note_count(1), 2);
    }

    #[test]
    fn test_delete_all_notes() {
        let plugin = NotesPlugin::new();

        plugin.add_note(1, "Note 1", None);
        plugin.add_note(1, "Note 2", None);

        plugin.delete_all_notes(1);
        assert_eq!(plugin.note_count(1), 0);
    }

    #[test]
    fn test_format_notes() {
        let plugin = NotesPlugin::new();

        plugin.add_note(1, "Test note content", Some("Bob".to_string()));

        let formatted = plugin.format_notes(1);
        assert!(formatted.contains("Notes (1)"));
        assert!(formatted.contains("Test note content"));
        assert!(formatted.contains("by Bob"));
    }
}
