//! Title Fetcher Plugin
//!
//! Automatically fetches the page title if not provided when adding a bookmark.
//! Uses the existing bukurs fetch functionality.

use bukurs::models::bookmark::Bookmark;
use bukurs::plugin::{HookResult, Plugin, PluginContext, PluginInfo};

pub struct TitleFetcherPlugin {
    /// Whether the plugin is enabled
    enabled: bool,
    /// Timeout for fetching in seconds
    timeout_secs: u64,
}

impl TitleFetcherPlugin {
    pub fn new() -> Self {
        Self {
            enabled: true,
            timeout_secs: 10,
        }
    }

    /// Fetch title from URL
    fn fetch_title(&self, url: &str) -> Option<String> {
        // Use bukurs fetch module
        match bukurs::fetch::fetch_data(url, None) {
            Ok(page_data) => {
                if !page_data.title.is_empty() {
                    Some(page_data.title.to_string())
                } else {
                    None
                }
            }
            Err(e) => {
                log::debug!("Failed to fetch title for {}: {}", url, e);
                None
            }
        }
    }
}

impl Default for TitleFetcherPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for TitleFetcherPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "title-fetcher".to_string(),
            version: "1.0.0".to_string(),
            description: "Auto-fetches page title if not provided".to_string(),
            author: "bukurs".to_string(),
        }
    }

    fn on_load(&mut self, ctx: &PluginContext) -> HookResult {
        if let Some(enabled) = ctx.config.get("enabled") {
            self.enabled = enabled != "false";
        }
        if let Some(timeout) = ctx.config.get("timeout") {
            self.timeout_secs = timeout.parse().unwrap_or(10);
        }
        HookResult::Continue
    }

    fn on_pre_add(&self, _ctx: &PluginContext, bookmark: &mut Bookmark) -> HookResult {
        if !self.enabled {
            return HookResult::Continue;
        }

        // Only fetch if title is empty or just the URL
        if bookmark.title.is_empty() || bookmark.title == bookmark.url {
            if let Some(title) = self.fetch_title(&bookmark.url) {
                log::info!("Auto-fetched title: {}", title);
                bookmark.title = title;
            }
        }

        HookResult::Continue
    }
}

/// Create an instance of this plugin (required for auto-discovery)
pub fn create_plugin() -> Box<dyn Plugin> {
    Box::new(TitleFetcherPlugin::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_info() {
        let plugin = TitleFetcherPlugin::new();
        let info = plugin.info();
        assert_eq!(info.name, "title-fetcher");
    }
}
