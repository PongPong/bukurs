//! Hook execution and aggregation utilities

use super::traits::{HookResult, OperationType, Plugin, PluginContext, SearchContext};
use crate::models::bookmark::Bookmark;
use log::{debug, error, warn};

/// Executes a hook across multiple plugins, collecting results
pub struct HookExecutor<'a> {
    plugins: &'a [Box<dyn Plugin>],
    ctx: &'a PluginContext,
}

impl<'a> HookExecutor<'a> {
    pub fn new(plugins: &'a [Box<dyn Plugin>], ctx: &'a PluginContext) -> Self {
        Self { plugins, ctx }
    }

    /// Execute pre-add hooks for all plugins
    /// Returns Error if any plugin returns an error, Skip if any returns skip,
    /// otherwise Continue
    pub fn execute_pre_add(&self, bookmark: &mut Bookmark) -> HookResult {
        debug!("Executing pre-add hooks for {} plugins", self.plugins.len());

        for plugin in self.plugins {
            let plugin_name = plugin.info().name;
            match plugin.on_pre_add(self.ctx, bookmark) {
                HookResult::Continue => {
                    debug!("Plugin '{}' pre-add: continue", plugin_name);
                }
                HookResult::Skip => {
                    warn!("Plugin '{}' requested to skip add operation", plugin_name);
                    return HookResult::Skip;
                }
                HookResult::Error(e) => {
                    error!("Plugin '{}' pre-add error: {}", plugin_name, e);
                    return HookResult::Error(format!("Plugin '{}': {}", plugin_name, e));
                }
            }
        }
        HookResult::Continue
    }

    /// Execute post-add hooks for all plugins
    pub fn execute_post_add(&self, bookmark: &Bookmark) -> HookResult {
        debug!("Executing post-add hooks");

        for plugin in self.plugins {
            let plugin_name = plugin.info().name;
            match plugin.on_post_add(self.ctx, bookmark) {
                HookResult::Continue => {}
                HookResult::Skip => {
                    // Skip doesn't make sense for post-hooks, treat as continue
                    debug!("Plugin '{}' returned Skip for post-add (ignored)", plugin_name);
                }
                HookResult::Error(e) => {
                    // Log error but don't fail the operation since it's already done
                    error!("Plugin '{}' post-add error (non-fatal): {}", plugin_name, e);
                }
            }
        }
        HookResult::Continue
    }

    /// Execute pre-update hooks for all plugins
    pub fn execute_pre_update(&self, old: &Bookmark, new: &mut Bookmark) -> HookResult {
        debug!("Executing pre-update hooks");

        for plugin in self.plugins {
            let plugin_name = plugin.info().name;
            match plugin.on_pre_update(self.ctx, old, new) {
                HookResult::Continue => {}
                HookResult::Skip => {
                    warn!("Plugin '{}' requested to skip update operation", plugin_name);
                    return HookResult::Skip;
                }
                HookResult::Error(e) => {
                    error!("Plugin '{}' pre-update error: {}", plugin_name, e);
                    return HookResult::Error(format!("Plugin '{}': {}", plugin_name, e));
                }
            }
        }
        HookResult::Continue
    }

    /// Execute post-update hooks for all plugins
    pub fn execute_post_update(&self, old: &Bookmark, new: &Bookmark) -> HookResult {
        debug!("Executing post-update hooks");

        for plugin in self.plugins {
            let plugin_name = plugin.info().name;
            if let HookResult::Error(e) = plugin.on_post_update(self.ctx, old, new) {
                error!("Plugin '{}' post-update error (non-fatal): {}", plugin_name, e);
            }
        }
        HookResult::Continue
    }

    /// Execute pre-delete hooks for all plugins
    pub fn execute_pre_delete(&self, bookmark: &Bookmark) -> HookResult {
        debug!("Executing pre-delete hooks");

        for plugin in self.plugins {
            let plugin_name = plugin.info().name;
            match plugin.on_pre_delete(self.ctx, bookmark) {
                HookResult::Continue => {}
                HookResult::Skip => {
                    warn!("Plugin '{}' requested to skip delete operation", plugin_name);
                    return HookResult::Skip;
                }
                HookResult::Error(e) => {
                    error!("Plugin '{}' pre-delete error: {}", plugin_name, e);
                    return HookResult::Error(format!("Plugin '{}': {}", plugin_name, e));
                }
            }
        }
        HookResult::Continue
    }

    /// Execute post-delete hooks for all plugins
    pub fn execute_post_delete(&self, bookmark: &Bookmark) -> HookResult {
        debug!("Executing post-delete hooks");

        for plugin in self.plugins {
            let plugin_name = plugin.info().name;
            if let HookResult::Error(e) = plugin.on_post_delete(self.ctx, bookmark) {
                error!("Plugin '{}' post-delete error (non-fatal): {}", plugin_name, e);
            }
        }
        HookResult::Continue
    }

    /// Execute pre-search hooks for all plugins
    pub fn execute_pre_search(&self, search_ctx: &mut SearchContext) -> HookResult {
        debug!("Executing pre-search hooks");

        for plugin in self.plugins {
            let plugin_name = plugin.info().name;
            match plugin.on_pre_search(self.ctx, search_ctx) {
                HookResult::Continue => {}
                HookResult::Skip => {
                    warn!("Plugin '{}' requested to skip search operation", plugin_name);
                    return HookResult::Skip;
                }
                HookResult::Error(e) => {
                    error!("Plugin '{}' pre-search error: {}", plugin_name, e);
                    return HookResult::Error(format!("Plugin '{}': {}", plugin_name, e));
                }
            }
        }
        HookResult::Continue
    }

    /// Execute post-search hooks for all plugins
    pub fn execute_post_search(
        &self,
        search_ctx: &SearchContext,
        results: &mut Vec<Bookmark>,
    ) -> HookResult {
        debug!("Executing post-search hooks");

        for plugin in self.plugins {
            let plugin_name = plugin.info().name;
            if let HookResult::Error(e) = plugin.on_post_search(self.ctx, search_ctx, results) {
                error!("Plugin '{}' post-search error (non-fatal): {}", plugin_name, e);
            }
        }
        HookResult::Continue
    }

    /// Execute pre-open hooks for all plugins
    pub fn execute_pre_open(&self, bookmark: &Bookmark) -> HookResult {
        debug!("Executing pre-open hooks");

        for plugin in self.plugins {
            let plugin_name = plugin.info().name;
            match plugin.on_pre_open(self.ctx, bookmark) {
                HookResult::Continue => {}
                HookResult::Skip => {
                    warn!("Plugin '{}' requested to skip open operation", plugin_name);
                    return HookResult::Skip;
                }
                HookResult::Error(e) => {
                    error!("Plugin '{}' pre-open error: {}", plugin_name, e);
                    return HookResult::Error(format!("Plugin '{}': {}", plugin_name, e));
                }
            }
        }
        HookResult::Continue
    }

    /// Execute pre-import hooks for all plugins
    pub fn execute_pre_import(&self, bookmarks: &mut Vec<Bookmark>) -> HookResult {
        debug!("Executing pre-import hooks for {} bookmarks", bookmarks.len());

        for plugin in self.plugins {
            let plugin_name = plugin.info().name;
            match plugin.on_pre_import(self.ctx, bookmarks) {
                HookResult::Continue => {}
                HookResult::Skip => {
                    warn!("Plugin '{}' requested to skip import operation", plugin_name);
                    return HookResult::Skip;
                }
                HookResult::Error(e) => {
                    error!("Plugin '{}' pre-import error: {}", plugin_name, e);
                    return HookResult::Error(format!("Plugin '{}': {}", plugin_name, e));
                }
            }
        }
        HookResult::Continue
    }

    /// Execute post-import hooks for all plugins
    pub fn execute_post_import(&self, bookmarks: &[Bookmark]) -> HookResult {
        debug!("Executing post-import hooks");

        for plugin in self.plugins {
            let plugin_name = plugin.info().name;
            if let HookResult::Error(e) = plugin.on_post_import(self.ctx, bookmarks) {
                error!("Plugin '{}' post-import error (non-fatal): {}", plugin_name, e);
            }
        }
        HookResult::Continue
    }

    /// Execute pre-export hooks for all plugins
    pub fn execute_pre_export(&self, bookmarks: &mut Vec<Bookmark>) -> HookResult {
        debug!("Executing pre-export hooks for {} bookmarks", bookmarks.len());

        for plugin in self.plugins {
            let plugin_name = plugin.info().name;
            match plugin.on_pre_export(self.ctx, bookmarks) {
                HookResult::Continue => {}
                HookResult::Skip => {
                    warn!("Plugin '{}' requested to skip export operation", plugin_name);
                    return HookResult::Skip;
                }
                HookResult::Error(e) => {
                    error!("Plugin '{}' pre-export error: {}", plugin_name, e);
                    return HookResult::Error(format!("Plugin '{}': {}", plugin_name, e));
                }
            }
        }
        HookResult::Continue
    }
}

/// Helper function to check if an operation should proceed based on hook result
pub fn should_proceed(result: &HookResult, operation: OperationType) -> bool {
    match result {
        HookResult::Continue => true,
        HookResult::Skip => {
            debug!("Skipping {} operation due to plugin request", operation);
            false
        }
        HookResult::Error(e) => {
            error!("{} operation blocked by plugin: {}", operation, e);
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::PluginInfo;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    struct CountingPlugin {
        pre_add_count: Arc<AtomicUsize>,
        post_add_count: Arc<AtomicUsize>,
    }

    impl Plugin for CountingPlugin {
        fn info(&self) -> PluginInfo {
            PluginInfo {
                name: "counting".to_string(),
                version: "1.0.0".to_string(),
                description: "Counting plugin".to_string(),
                author: "Test".to_string(),
            }
        }

        fn on_pre_add(&self, _ctx: &PluginContext, _bookmark: &mut Bookmark) -> HookResult {
            self.pre_add_count.fetch_add(1, Ordering::SeqCst);
            HookResult::Continue
        }

        fn on_post_add(&self, _ctx: &PluginContext, _bookmark: &Bookmark) -> HookResult {
            self.post_add_count.fetch_add(1, Ordering::SeqCst);
            HookResult::Continue
        }
    }

    struct SkippingPlugin;

    impl Plugin for SkippingPlugin {
        fn info(&self) -> PluginInfo {
            PluginInfo {
                name: "skipping".to_string(),
                version: "1.0.0".to_string(),
                description: "Skipping plugin".to_string(),
                author: "Test".to_string(),
            }
        }

        fn on_pre_add(&self, _ctx: &PluginContext, _bookmark: &mut Bookmark) -> HookResult {
            HookResult::Skip
        }
    }

    #[test]
    fn test_hook_executor_counts_calls() {
        let pre_count = Arc::new(AtomicUsize::new(0));
        let post_count = Arc::new(AtomicUsize::new(0));

        let plugin = CountingPlugin {
            pre_add_count: Arc::clone(&pre_count),
            post_add_count: Arc::clone(&post_count),
        };

        let plugins: Vec<Box<dyn Plugin>> = vec![Box::new(plugin)];
        let ctx = PluginContext::new(
            PathBuf::from("/test/db.sqlite"),
            PathBuf::from("/test/data"),
        );

        let executor = HookExecutor::new(&plugins, &ctx);
        let mut bookmark = Bookmark::new(
            1,
            "https://example.com".to_string(),
            "Example".to_string(),
            "".to_string(),
            "".to_string(),
        );

        executor.execute_pre_add(&mut bookmark);
        executor.execute_post_add(&bookmark);

        assert_eq!(pre_count.load(Ordering::SeqCst), 1);
        assert_eq!(post_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_hook_executor_respects_skip() {
        let plugins: Vec<Box<dyn Plugin>> = vec![Box::new(SkippingPlugin)];
        let ctx = PluginContext::new(
            PathBuf::from("/test/db.sqlite"),
            PathBuf::from("/test/data"),
        );

        let executor = HookExecutor::new(&plugins, &ctx);
        let mut bookmark = Bookmark::new(
            1,
            "https://example.com".to_string(),
            "Example".to_string(),
            "".to_string(),
            "".to_string(),
        );

        let result = executor.execute_pre_add(&mut bookmark);
        assert!(result.is_skip());
    }
}
