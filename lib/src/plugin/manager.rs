//! Plugin manager for loading, registering, and managing plugins

use super::traits::{
    CommandPlugin, HookResult, OutputFormatPlugin, Plugin, PluginContext, PluginInfo,
    SearchContext,
};
use super::hooks::HookExecutor;
use crate::error::{BukursError, Result};
use crate::models::bookmark::Bookmark;
use log::{debug, info, warn};
use std::collections::HashMap;
use std::path::PathBuf;

/// Manages all loaded plugins and provides access to plugin functionality
pub struct PluginManager {
    /// All registered plugins
    plugins: Vec<Box<dyn Plugin>>,
    /// Output format plugins indexed by format name
    output_formats: HashMap<String, usize>,
    /// Command plugins indexed by command name
    commands: HashMap<String, usize>,
    /// Plugin context for hook execution
    context: PluginContext,
    /// Whether plugins are enabled
    enabled: bool,
}

impl PluginManager {
    /// Create a new plugin manager
    pub fn new(db_path: PathBuf, data_dir: PathBuf) -> Self {
        Self {
            plugins: Vec::new(),
            output_formats: HashMap::new(),
            commands: HashMap::new(),
            context: PluginContext::new(db_path, data_dir),
            enabled: true,
        }
    }

    /// Create a disabled plugin manager (for when plugins are disabled)
    pub fn disabled() -> Self {
        Self {
            plugins: Vec::new(),
            output_formats: HashMap::new(),
            commands: HashMap::new(),
            context: PluginContext::new(PathBuf::new(), PathBuf::new()),
            enabled: false,
        }
    }

    /// Check if plugins are enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Enable or disable plugins
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Get the plugin context
    pub fn context(&self) -> &PluginContext {
        &self.context
    }

    /// Get a mutable reference to the plugin context
    pub fn context_mut(&mut self) -> &mut PluginContext {
        &mut self.context
    }

    /// Register a plugin
    pub fn register(&mut self, mut plugin: Box<dyn Plugin>) -> Result<()> {
        let info = plugin.info();
        info!("Registering plugin: {}", info);

        // Check for duplicate plugin names
        if self.plugins.iter().any(|p| p.info().name == info.name) {
            return Err(BukursError::Plugin(format!(
                "Plugin '{}' is already registered",
                info.name
            )));
        }

        // Call the plugin's on_load hook
        match plugin.on_load(&self.context) {
            HookResult::Continue => {}
            HookResult::Skip => {
                warn!("Plugin '{}' requested to skip loading", info.name);
                return Ok(());
            }
            HookResult::Error(e) => {
                return Err(BukursError::Plugin(format!(
                    "Plugin '{}' failed to load: {}",
                    info.name, e
                )));
            }
        }

        self.plugins.push(plugin);
        debug!("Plugin '{}' registered successfully", info.name);
        Ok(())
    }

    /// Register an output format plugin
    pub fn register_output_format(&mut self, plugin: Box<dyn OutputFormatPlugin>) -> Result<()> {
        let format_name = plugin.format_name().to_string();
        let info = plugin.info();

        if self.output_formats.contains_key(&format_name) {
            return Err(BukursError::Plugin(format!(
                "Output format '{}' is already registered",
                format_name
            )));
        }

        info!(
            "Registering output format '{}' from plugin '{}'",
            format_name, info.name
        );

        // Store the index for the output format lookup
        let index = self.plugins.len();
        self.register(plugin)?;
        self.output_formats.insert(format_name, index);

        Ok(())
    }

    /// Register a command plugin
    pub fn register_command(&mut self, plugin: Box<dyn CommandPlugin>) -> Result<()> {
        let command_name = plugin.command_name().to_string();
        let info = plugin.info();

        if self.commands.contains_key(&command_name) {
            return Err(BukursError::Plugin(format!(
                "Command '{}' is already registered",
                command_name
            )));
        }

        info!(
            "Registering command '{}' from plugin '{}'",
            command_name, info.name
        );

        let index = self.plugins.len();
        self.register(plugin)?;
        self.commands.insert(command_name, index);

        Ok(())
    }

    /// Unregister a plugin by name
    pub fn unregister(&mut self, name: &str) -> Result<()> {
        let index = self
            .plugins
            .iter()
            .position(|p| p.info().name == name)
            .ok_or_else(|| BukursError::Plugin(format!("Plugin '{}' not found", name)))?;

        let mut plugin = self.plugins.remove(index);
        plugin.on_unload(&self.context);

        // Update indices in output_formats and commands
        self.output_formats.retain(|_, &mut idx| idx != index);
        self.commands.retain(|_, &mut idx| idx != index);

        // Decrement indices greater than the removed index
        for idx in self.output_formats.values_mut() {
            if *idx > index {
                *idx -= 1;
            }
        }
        for idx in self.commands.values_mut() {
            if *idx > index {
                *idx -= 1;
            }
        }

        info!("Plugin '{}' unregistered", name);
        Ok(())
    }

    /// Get list of all registered plugins
    pub fn list_plugins(&self) -> Vec<PluginInfo> {
        self.plugins.iter().map(|p| p.info()).collect()
    }

    /// Get list of available output formats from plugins
    pub fn list_output_formats(&self) -> Vec<String> {
        self.output_formats.keys().cloned().collect()
    }

    /// Get list of available commands from plugins
    pub fn list_commands(&self) -> Vec<(String, String)> {
        self.commands
            .keys()
            .filter_map(|name| {
                self.get_command_plugin(name)
                    .map(|p| (name.clone(), p.command_help().to_string()))
            })
            .collect()
    }

    /// Get an output format plugin by name
    pub fn get_output_format(&self, name: &str) -> Option<&dyn OutputFormatPlugin> {
        self.output_formats.get(name).and_then(|&idx| {
            self.plugins.get(idx).and_then(|_p| {
                // Safe downcast - we know this plugin implements OutputFormatPlugin
                // because we only store indices of such plugins in output_formats
                // Note: Proper implementation would require storing trait objects separately
                // or using downcast with Any trait
                None // Will be implemented via a different approach
            })
        })
    }

    /// Get a command plugin by name
    fn get_command_plugin(&self, name: &str) -> Option<&dyn CommandPlugin> {
        self.commands.get(name).and_then(|&idx| {
            self.plugins.get(idx).and_then(|_p| {
                // Similar to above, needs a different approach
                None
            })
        })
    }

    /// Format bookmarks using a plugin-provided output format
    pub fn format_bookmarks(&self, format: &str, _bookmarks: &[Bookmark]) -> Option<String> {
        // This will be implemented with proper trait object handling
        self.output_formats.get(format).map(|_| {
            // Placeholder - actual implementation requires proper trait object storage
            String::new()
        })
    }

    /// Execute a plugin command
    pub fn execute_command(&self, name: &str, _args: &[String]) -> Result<String> {
        if !self.enabled {
            return Err(BukursError::Plugin("Plugins are disabled".to_string()));
        }

        // Placeholder for actual implementation
        Err(BukursError::Plugin(format!(
            "Command '{}' not found",
            name
        )))
    }

    // ---- Hook execution methods ----

    fn executor(&self) -> HookExecutor<'_> {
        HookExecutor::new(&self.plugins, &self.context)
    }

    /// Execute pre-add hooks
    pub fn on_pre_add(&self, bookmark: &mut Bookmark) -> HookResult {
        if !self.enabled || self.plugins.is_empty() {
            return HookResult::Continue;
        }
        self.executor().execute_pre_add(bookmark)
    }

    /// Execute post-add hooks
    pub fn on_post_add(&self, bookmark: &Bookmark) -> HookResult {
        if !self.enabled || self.plugins.is_empty() {
            return HookResult::Continue;
        }
        self.executor().execute_post_add(bookmark)
    }

    /// Execute pre-update hooks
    pub fn on_pre_update(&self, old: &Bookmark, new: &mut Bookmark) -> HookResult {
        if !self.enabled || self.plugins.is_empty() {
            return HookResult::Continue;
        }
        self.executor().execute_pre_update(old, new)
    }

    /// Execute post-update hooks
    pub fn on_post_update(&self, old: &Bookmark, new: &Bookmark) -> HookResult {
        if !self.enabled || self.plugins.is_empty() {
            return HookResult::Continue;
        }
        self.executor().execute_post_update(old, new)
    }

    /// Execute pre-delete hooks
    pub fn on_pre_delete(&self, bookmark: &Bookmark) -> HookResult {
        if !self.enabled || self.plugins.is_empty() {
            return HookResult::Continue;
        }
        self.executor().execute_pre_delete(bookmark)
    }

    /// Execute post-delete hooks
    pub fn on_post_delete(&self, bookmark: &Bookmark) -> HookResult {
        if !self.enabled || self.plugins.is_empty() {
            return HookResult::Continue;
        }
        self.executor().execute_post_delete(bookmark)
    }

    /// Execute pre-search hooks
    pub fn on_pre_search(&self, search_ctx: &mut SearchContext) -> HookResult {
        if !self.enabled || self.plugins.is_empty() {
            return HookResult::Continue;
        }
        self.executor().execute_pre_search(search_ctx)
    }

    /// Execute post-search hooks
    pub fn on_post_search(
        &self,
        search_ctx: &SearchContext,
        results: &mut Vec<Bookmark>,
    ) -> HookResult {
        if !self.enabled || self.plugins.is_empty() {
            return HookResult::Continue;
        }
        self.executor().execute_post_search(search_ctx, results)
    }

    /// Execute pre-open hooks
    pub fn on_pre_open(&self, bookmark: &Bookmark) -> HookResult {
        if !self.enabled || self.plugins.is_empty() {
            return HookResult::Continue;
        }
        self.executor().execute_pre_open(bookmark)
    }

    /// Execute pre-import hooks
    pub fn on_pre_import(&self, bookmarks: &mut Vec<Bookmark>) -> HookResult {
        if !self.enabled || self.plugins.is_empty() {
            return HookResult::Continue;
        }
        self.executor().execute_pre_import(bookmarks)
    }

    /// Execute post-import hooks
    pub fn on_post_import(&self, bookmarks: &[Bookmark]) -> HookResult {
        if !self.enabled || self.plugins.is_empty() {
            return HookResult::Continue;
        }
        self.executor().execute_post_import(bookmarks)
    }

    /// Execute pre-export hooks
    pub fn on_pre_export(&self, bookmarks: &mut Vec<Bookmark>) -> HookResult {
        if !self.enabled || self.plugins.is_empty() {
            return HookResult::Continue;
        }
        self.executor().execute_pre_export(bookmarks)
    }

    /// Sort plugins by priority (if they implement PrioritizedPlugin)
    pub fn sort_by_priority(&mut self) {
        // Plugins that don't implement PrioritizedPlugin get Normal priority
        self.plugins.sort_by(|_a, _b| {
            let a_priority = 50; // Default Normal priority
            let b_priority = 50;
            a_priority.cmp(&b_priority)
        });
    }
}

impl Drop for PluginManager {
    fn drop(&mut self) {
        // Call on_unload for all plugins
        for plugin in &mut self.plugins {
            plugin.on_unload(&self.context);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestPlugin {
        name: String,
    }

    impl Plugin for TestPlugin {
        fn info(&self) -> PluginInfo {
            PluginInfo {
                name: self.name.clone(),
                version: "1.0.0".to_string(),
                description: "Test plugin".to_string(),
                author: "Test".to_string(),
            }
        }
    }

    #[test]
    fn test_register_plugin() {
        let mut manager = PluginManager::new(
            PathBuf::from("/test/db.sqlite"),
            PathBuf::from("/test/data"),
        );

        let plugin = TestPlugin {
            name: "test".to_string(),
        };

        assert!(manager.register(Box::new(plugin)).is_ok());
        assert_eq!(manager.list_plugins().len(), 1);
    }

    #[test]
    fn test_duplicate_plugin_rejected() {
        let mut manager = PluginManager::new(
            PathBuf::from("/test/db.sqlite"),
            PathBuf::from("/test/data"),
        );

        let plugin1 = TestPlugin {
            name: "test".to_string(),
        };
        let plugin2 = TestPlugin {
            name: "test".to_string(),
        };

        assert!(manager.register(Box::new(plugin1)).is_ok());
        assert!(manager.register(Box::new(plugin2)).is_err());
    }

    #[test]
    fn test_unregister_plugin() {
        let mut manager = PluginManager::new(
            PathBuf::from("/test/db.sqlite"),
            PathBuf::from("/test/data"),
        );

        let plugin = TestPlugin {
            name: "test".to_string(),
        };

        manager.register(Box::new(plugin)).unwrap();
        assert_eq!(manager.list_plugins().len(), 1);

        manager.unregister("test").unwrap();
        assert_eq!(manager.list_plugins().len(), 0);
    }

    #[test]
    fn test_disabled_manager() {
        let manager = PluginManager::disabled();
        assert!(!manager.is_enabled());

        let mut bookmark = Bookmark::new(
            1,
            "https://example.com".to_string(),
            "Example".to_string(),
            "".to_string(),
            "".to_string(),
        );

        // Hooks should return Continue even when disabled
        assert!(manager.on_pre_add(&mut bookmark).is_continue());
    }
}
