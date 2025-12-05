//! Archive Plugin
//!
//! Saves bookmarks to archive.org (Wayback Machine) when added.
//! Stores the archived URL in the bookmark description.

use bukurs::models::bookmark::Bookmark;
use bukurs::plugin::{HookResult, Plugin, PluginContext, PluginInfo};
use std::time::Duration;

pub struct ArchivePlugin {
    /// Whether the plugin is enabled
    enabled: bool,
    /// Whether to block on archiving (false = async/fire-and-forget)
    blocking: bool,
    /// Timeout for archive requests
    timeout_secs: u64,
}

impl ArchivePlugin {
    pub fn new() -> Self {
        Self {
            enabled: false, // Disabled by default - requires explicit opt-in
            blocking: false,
            timeout_secs: 30,
        }
    }

    /// Submit URL to archive.org
    fn archive_url(&self, url: &str) -> Result<String, String> {
        let save_url = format!("https://web.archive.org/save/{}", url);

        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(self.timeout_secs))
            .build()
            .map_err(|e| e.to_string())?;

        let response = client
            .get(&save_url)
            .header("User-Agent", "bukurs-archive-plugin/1.0")
            .send()
            .map_err(|e| e.to_string())?;

        if response.status().is_success() || response.status().is_redirection() {
            // Archive.org returns the archived URL in headers or redirects
            if let Some(location) = response.headers().get("content-location") {
                if let Ok(loc) = location.to_str() {
                    return Ok(format!("https://web.archive.org{}", loc));
                }
            }
            // Fallback: construct the URL
            Ok(format!("https://web.archive.org/web/{}", url))
        } else {
            Err(format!("Archive.org returned status: {}", response.status()))
        }
    }

    /// Append archive URL to description
    fn append_archive_url(description: &str, archive_url: &str) -> String {
        if description.is_empty() {
            format!("[Archived: {}]", archive_url)
        } else {
            format!("{}\n[Archived: {}]", description, archive_url)
        }
    }
}

impl Default for ArchivePlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for ArchivePlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "archive".to_string(),
            version: "1.0.0".to_string(),
            description: "Saves bookmarks to archive.org Wayback Machine".to_string(),
            author: "bukurs".to_string(),
        }
    }

    fn on_load(&mut self, ctx: &PluginContext) -> HookResult {
        if let Some(enabled) = ctx.config.get("enabled") {
            self.enabled = enabled == "true";
        }
        if let Some(blocking) = ctx.config.get("blocking") {
            self.blocking = blocking == "true";
        }
        if let Some(timeout) = ctx.config.get("timeout") {
            self.timeout_secs = timeout.parse().unwrap_or(30);
        }
        HookResult::Continue
    }

    fn on_post_add(&self, _ctx: &PluginContext, bookmark: &Bookmark) -> HookResult {
        if !self.enabled {
            return HookResult::Continue;
        }

        // Only archive http/https URLs
        if !bookmark.url.starts_with("http://") && !bookmark.url.starts_with("https://") {
            return HookResult::Continue;
        }

        let url = bookmark.url.clone();

        if self.blocking {
            match self.archive_url(&url) {
                Ok(archive_url) => {
                    log::info!("Archived to: {}", archive_url);
                }
                Err(e) => {
                    log::warn!("Failed to archive {}: {}", url, e);
                }
            }
        } else {
            // Fire and forget
            let timeout = self.timeout_secs;
            std::thread::spawn(move || {
                let save_url = format!("https://web.archive.org/save/{}", url);
                let client = reqwest::blocking::Client::builder()
                    .timeout(Duration::from_secs(timeout))
                    .build();

                if let Ok(client) = client {
                    let _ = client
                        .get(&save_url)
                        .header("User-Agent", "bukurs-archive-plugin/1.0")
                        .send();
                    log::debug!("Archive request sent for: {}", url);
                }
            });
        }

        HookResult::Continue
    }
}

/// Create an instance of this plugin (required for auto-discovery)
pub fn create_plugin() -> Box<dyn Plugin> {
    Box::new(ArchivePlugin::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_append_archive_url() {
        assert_eq!(
            ArchivePlugin::append_archive_url("", "https://web.archive.org/web/test"),
            "[Archived: https://web.archive.org/web/test]"
        );

        assert_eq!(
            ArchivePlugin::append_archive_url("My description", "https://web.archive.org/web/test"),
            "My description\n[Archived: https://web.archive.org/web/test]"
        );
    }

    #[test]
    fn test_plugin_disabled_by_default() {
        let plugin = ArchivePlugin::new();
        assert!(!plugin.enabled);
    }
}
