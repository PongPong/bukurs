//! Dead Link Checker Plugin
//!
//! Checks if bookmark URLs are still accessible.
//! Can run on-demand or automatically tag dead links.

use bukurs::models::bookmark::Bookmark;
use bukurs::plugin::{HookResult, Plugin, PluginContext, PluginInfo};
use std::time::Duration;

pub struct DeadLinkCheckerPlugin {
    /// Whether to check links on open
    check_on_open: bool,
    /// Whether to check links on search results
    check_on_search: bool,
    /// Tag to add for dead links
    dead_link_tag: String,
    /// Timeout for HTTP requests
    timeout_secs: u64,
    /// Whether the plugin is enabled
    enabled: bool,
}

impl DeadLinkCheckerPlugin {
    pub fn new() -> Self {
        Self {
            check_on_open: true,
            check_on_search: false, // Disabled by default - can be slow
            dead_link_tag: "dead-link".to_string(),
            timeout_secs: 10,
            enabled: true,
        }
    }

    /// Check if a URL is accessible
    fn check_url(&self, url: &str) -> LinkStatus {
        // Skip non-HTTP URLs
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return LinkStatus::Skipped;
        }

        let client = match reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(self.timeout_secs))
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()
        {
            Ok(c) => c,
            Err(_) => return LinkStatus::Error("Failed to create HTTP client".to_string()),
        };

        // Use HEAD request first (faster)
        let response = client
            .head(url)
            .header("User-Agent", "bukurs-link-checker/1.0")
            .send();

        match response {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() || status.is_redirection() {
                    LinkStatus::Alive
                } else if status == reqwest::StatusCode::METHOD_NOT_ALLOWED {
                    // Some servers don't allow HEAD, try GET
                    match client.get(url).send() {
                        Ok(resp) => {
                            if resp.status().is_success() {
                                LinkStatus::Alive
                            } else {
                                LinkStatus::Dead(resp.status().as_u16())
                            }
                        }
                        Err(e) => LinkStatus::Error(e.to_string()),
                    }
                } else {
                    LinkStatus::Dead(status.as_u16())
                }
            }
            Err(e) => {
                if e.is_timeout() {
                    LinkStatus::Timeout
                } else if e.is_connect() {
                    LinkStatus::Dead(0)
                } else {
                    LinkStatus::Error(e.to_string())
                }
            }
        }
    }

    /// Add dead-link tag to bookmark
    fn add_dead_tag(tags: &str, dead_tag: &str) -> String {
        let mut tag_list: Vec<String> = tags
            .trim_matches(',')
            .split(',')
            .filter(|t| !t.is_empty())
            .map(|t| t.to_string())
            .collect();

        if !tag_list.contains(&dead_tag.to_string()) {
            tag_list.push(dead_tag.to_string());
        }

        if tag_list.is_empty() {
            String::new()
        } else {
            format!(",{},", tag_list.join(","))
        }
    }

    /// Remove dead-link tag from bookmark
    fn remove_dead_tag(tags: &str, dead_tag: &str) -> String {
        let tag_list: Vec<String> = tags
            .trim_matches(',')
            .split(',')
            .filter(|t| !t.is_empty() && *t != dead_tag)
            .map(|t| t.to_string())
            .collect();

        if tag_list.is_empty() {
            String::new()
        } else {
            format!(",{},", tag_list.join(","))
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum LinkStatus {
    Alive,
    Dead(u16),
    Timeout,
    Error(String),
    Skipped,
}

impl Default for DeadLinkCheckerPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for DeadLinkCheckerPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "dead-link-checker".to_string(),
            version: "1.0.0".to_string(),
            description: "Checks and tags dead links".to_string(),
            author: "bukurs".to_string(),
        }
    }

    fn on_load(&mut self, ctx: &PluginContext) -> HookResult {
        if let Some(enabled) = ctx.config.get("enabled") {
            self.enabled = enabled != "false";
        }
        if let Some(check_open) = ctx.config.get("check_on_open") {
            self.check_on_open = check_open == "true";
        }
        if let Some(check_search) = ctx.config.get("check_on_search") {
            self.check_on_search = check_search == "true";
        }
        if let Some(tag) = ctx.config.get("dead_link_tag") {
            self.dead_link_tag = tag.clone();
        }
        if let Some(timeout) = ctx.config.get("timeout") {
            self.timeout_secs = timeout.parse().unwrap_or(10);
        }
        HookResult::Continue
    }

    fn on_pre_open(&self, _ctx: &PluginContext, bookmark: &Bookmark) -> HookResult {
        if !self.enabled || !self.check_on_open {
            return HookResult::Continue;
        }

        match self.check_url(&bookmark.url) {
            LinkStatus::Dead(code) => {
                log::warn!(
                    "Warning: URL may be dead (HTTP {}): {}",
                    code,
                    bookmark.url
                );
            }
            LinkStatus::Timeout => {
                log::warn!("Warning: URL timed out: {}", bookmark.url);
            }
            LinkStatus::Error(e) => {
                log::warn!("Warning: Could not check URL: {} - {}", bookmark.url, e);
            }
            _ => {}
        }

        // Don't block opening - just warn
        HookResult::Continue
    }

    fn on_post_search(
        &self,
        _ctx: &PluginContext,
        _search_ctx: &bukurs::plugin::SearchContext,
        results: &mut Vec<Bookmark>,
    ) -> HookResult {
        if !self.enabled || !self.check_on_search {
            return HookResult::Continue;
        }

        // Check each result (can be slow!)
        for bookmark in results.iter_mut() {
            match self.check_url(&bookmark.url) {
                LinkStatus::Dead(_) | LinkStatus::Timeout => {
                    bookmark.tags = Self::add_dead_tag(&bookmark.tags, &self.dead_link_tag);
                }
                LinkStatus::Alive => {
                    // Remove dead tag if URL is now alive
                    bookmark.tags = Self::remove_dead_tag(&bookmark.tags, &self.dead_link_tag);
                }
                _ => {}
            }
        }

        HookResult::Continue
    }
}

/// Create an instance of this plugin (required for auto-discovery)
pub fn create_plugin() -> Box<dyn Plugin> {
    Box::new(DeadLinkCheckerPlugin::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_dead_tag() {
        assert_eq!(
            DeadLinkCheckerPlugin::add_dead_tag(",rust,web,", "dead-link"),
            ",rust,web,dead-link,"
        );
        assert_eq!(
            DeadLinkCheckerPlugin::add_dead_tag("", "dead-link"),
            ",dead-link,"
        );
        // Don't add duplicate
        assert_eq!(
            DeadLinkCheckerPlugin::add_dead_tag(",dead-link,", "dead-link"),
            ",dead-link,"
        );
    }

    #[test]
    fn test_remove_dead_tag() {
        assert_eq!(
            DeadLinkCheckerPlugin::remove_dead_tag(",rust,dead-link,web,", "dead-link"),
            ",rust,web,"
        );
        assert_eq!(
            DeadLinkCheckerPlugin::remove_dead_tag(",dead-link,", "dead-link"),
            ""
        );
    }

    #[test]
    fn test_plugin_info() {
        let plugin = DeadLinkCheckerPlugin::new();
        let info = plugin.info();
        assert_eq!(info.name, "dead-link-checker");
    }
}
