//! Core plugin traits and types

use crate::models::bookmark::Bookmark;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;

/// Information about a plugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    /// Unique name/identifier for the plugin
    pub name: String,
    /// Version string (semver recommended)
    pub version: String,
    /// Human-readable description
    pub description: String,
    /// Plugin author
    pub author: String,
}

impl fmt::Display for PluginInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} v{} - {} (by {})",
            self.name, self.version, self.description, self.author
        )
    }
}

/// Result of a hook execution
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookResult {
    /// Continue with the operation
    Continue,
    /// Skip this operation (for pre-hooks only)
    Skip,
    /// Stop processing and return an error
    Error(String),
}

impl HookResult {
    pub fn is_continue(&self) -> bool {
        matches!(self, HookResult::Continue)
    }

    pub fn is_skip(&self) -> bool {
        matches!(self, HookResult::Skip)
    }

    pub fn is_error(&self) -> bool {
        matches!(self, HookResult::Error(_))
    }
}

/// Context passed to plugins during hook execution
#[derive(Debug, Clone)]
pub struct PluginContext {
    /// Path to the database file
    pub db_path: PathBuf,
    /// Plugin-specific configuration from the main config
    pub config: HashMap<String, String>,
    /// Plugin's data directory for persistent storage
    pub data_dir: PathBuf,
}

impl PluginContext {
    pub fn new(db_path: PathBuf, data_dir: PathBuf) -> Self {
        Self {
            db_path,
            config: HashMap::new(),
            data_dir,
        }
    }

    pub fn with_config(mut self, config: HashMap<String, String>) -> Self {
        self.config = config;
        self
    }
}

/// Search context passed to search-related hooks
#[derive(Debug, Clone)]
pub struct SearchContext {
    /// The search query/keywords
    pub query: String,
    /// Whether regex mode is enabled
    pub regex: bool,
    /// Whether deep search is enabled
    pub deep: bool,
    /// Tag filters if any
    pub tags: Vec<String>,
}

impl SearchContext {
    pub fn new(query: String) -> Self {
        Self {
            query,
            regex: false,
            deep: false,
            tags: Vec::new(),
        }
    }
}

/// Operation type for hooks
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationType {
    Add,
    Update,
    Delete,
    Search,
    Open,
    Import,
    Export,
}

impl fmt::Display for OperationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OperationType::Add => write!(f, "add"),
            OperationType::Update => write!(f, "update"),
            OperationType::Delete => write!(f, "delete"),
            OperationType::Search => write!(f, "search"),
            OperationType::Open => write!(f, "open"),
            OperationType::Import => write!(f, "import"),
            OperationType::Export => write!(f, "export"),
        }
    }
}

/// The core plugin trait that all plugins must implement
pub trait Plugin: Send + Sync {
    /// Returns information about the plugin
    fn info(&self) -> PluginInfo;

    /// Called when the plugin is loaded
    fn on_load(&mut self, _ctx: &PluginContext) -> HookResult {
        HookResult::Continue
    }

    /// Called when the plugin is unloaded
    fn on_unload(&mut self, _ctx: &PluginContext) {
        // Default: do nothing
    }

    // ---- Bookmark Add Hooks ----

    /// Called before a bookmark is added to the database
    /// Can modify the bookmark or prevent the add operation
    fn on_pre_add(&self, _ctx: &PluginContext, _bookmark: &mut Bookmark) -> HookResult {
        HookResult::Continue
    }

    /// Called after a bookmark has been successfully added
    fn on_post_add(&self, _ctx: &PluginContext, _bookmark: &Bookmark) -> HookResult {
        HookResult::Continue
    }

    // ---- Bookmark Update Hooks ----

    /// Called before a bookmark is updated
    /// Receives both the old and new bookmark data
    fn on_pre_update(
        &self,
        _ctx: &PluginContext,
        _old: &Bookmark,
        _new: &mut Bookmark,
    ) -> HookResult {
        HookResult::Continue
    }

    /// Called after a bookmark has been updated
    fn on_post_update(
        &self,
        _ctx: &PluginContext,
        _old: &Bookmark,
        _new: &Bookmark,
    ) -> HookResult {
        HookResult::Continue
    }

    // ---- Bookmark Delete Hooks ----

    /// Called before a bookmark is deleted
    fn on_pre_delete(&self, _ctx: &PluginContext, _bookmark: &Bookmark) -> HookResult {
        HookResult::Continue
    }

    /// Called after a bookmark has been deleted
    fn on_post_delete(&self, _ctx: &PluginContext, _bookmark: &Bookmark) -> HookResult {
        HookResult::Continue
    }

    // ---- Search Hooks ----

    /// Called before a search is executed
    /// Can modify search parameters
    fn on_pre_search(&self, _ctx: &PluginContext, _search_ctx: &mut SearchContext) -> HookResult {
        HookResult::Continue
    }

    /// Called after search results are retrieved
    /// Can filter or modify results
    fn on_post_search(
        &self,
        _ctx: &PluginContext,
        _search_ctx: &SearchContext,
        _results: &mut Vec<Bookmark>,
    ) -> HookResult {
        HookResult::Continue
    }

    // ---- Open Hook ----

    /// Called before a bookmark URL is opened
    fn on_pre_open(&self, _ctx: &PluginContext, _bookmark: &Bookmark) -> HookResult {
        HookResult::Continue
    }

    // ---- Import/Export Hooks ----

    /// Called before bookmarks are imported
    fn on_pre_import(
        &self,
        _ctx: &PluginContext,
        _bookmarks: &mut Vec<Bookmark>,
    ) -> HookResult {
        HookResult::Continue
    }

    /// Called after bookmarks have been imported
    fn on_post_import(&self, _ctx: &PluginContext, _bookmarks: &[Bookmark]) -> HookResult {
        HookResult::Continue
    }

    /// Called before bookmarks are exported
    fn on_pre_export(&self, _ctx: &PluginContext, _bookmarks: &mut Vec<Bookmark>) -> HookResult {
        HookResult::Continue
    }
}

/// Trait for plugins that provide custom output formats
pub trait OutputFormatPlugin: Plugin {
    /// Returns the format name (e.g., "csv", "xml")
    fn format_name(&self) -> &str;

    /// Returns the file extension for this format
    fn file_extension(&self) -> &str;

    /// Formats a single bookmark
    fn format_bookmark(&self, bookmark: &Bookmark) -> String;

    /// Formats multiple bookmarks (can include header/footer)
    fn format_bookmarks(&self, bookmarks: &[Bookmark]) -> String {
        bookmarks
            .iter()
            .map(|b| self.format_bookmark(b))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Returns a header for the format (optional)
    fn header(&self) -> Option<String> {
        None
    }

    /// Returns a footer for the format (optional)
    fn footer(&self) -> Option<String> {
        None
    }
}

/// Trait for plugins that provide custom commands
pub trait CommandPlugin: Plugin {
    /// Returns the command name
    fn command_name(&self) -> &str;

    /// Returns command usage/help text
    fn command_help(&self) -> &str;

    /// Executes the command with given arguments
    fn execute_command(
        &self,
        ctx: &PluginContext,
        args: &[String],
    ) -> Result<String, String>;
}

/// Priority levels for hook execution order
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PluginPriority {
    /// Executed first (e.g., validation plugins)
    High = 0,
    /// Default priority
    Normal = 50,
    /// Executed last (e.g., notification plugins)
    Low = 100,
}

impl Default for PluginPriority {
    fn default() -> Self {
        PluginPriority::Normal
    }
}

/// Extended plugin trait with priority support
pub trait PrioritizedPlugin: Plugin {
    /// Returns the plugin's execution priority
    fn priority(&self) -> PluginPriority {
        PluginPriority::Normal
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestPlugin;

    impl Plugin for TestPlugin {
        fn info(&self) -> PluginInfo {
            PluginInfo {
                name: "test-plugin".to_string(),
                version: "1.0.0".to_string(),
                description: "A test plugin".to_string(),
                author: "Test Author".to_string(),
            }
        }
    }

    #[test]
    fn test_plugin_info_display() {
        let info = PluginInfo {
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            description: "Test plugin".to_string(),
            author: "Test".to_string(),
        };
        assert_eq!(
            format!("{}", info),
            "test v1.0.0 - Test plugin (by Test)"
        );
    }

    #[test]
    fn test_hook_result() {
        assert!(HookResult::Continue.is_continue());
        assert!(HookResult::Skip.is_skip());
        assert!(HookResult::Error("err".to_string()).is_error());
    }

    #[test]
    fn test_default_hooks_return_continue() {
        let plugin = TestPlugin;
        let ctx = PluginContext::new(
            PathBuf::from("/test/db.sqlite"),
            PathBuf::from("/test/data"),
        );
        let mut bookmark = Bookmark::new(
            1,
            "https://example.com".to_string(),
            "Example".to_string(),
            "".to_string(),
            "".to_string(),
        );

        assert!(plugin.on_pre_add(&ctx, &mut bookmark).is_continue());
        assert!(plugin.on_post_add(&ctx, &bookmark).is_continue());
    }
}
