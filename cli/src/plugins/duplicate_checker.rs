//! Duplicate Checker Plugin
//!
//! Warns about similar URLs when adding bookmarks:
//! - Same domain + path but different query params
//! - HTTP vs HTTPS variants
//! - With/without www prefix
//! - With/without trailing slash

use bukurs::models::bookmark::Bookmark;
use bukurs::plugin::{HookResult, Plugin, PluginContext, PluginInfo};
use std::collections::HashSet;
use std::sync::Mutex;

pub struct DuplicateCheckerPlugin {
    /// Known URLs (normalized)
    known_urls: Mutex<HashSet<String>>,
    /// Whether to block duplicates or just warn
    block_duplicates: bool,
    /// Check for similar URLs (not just exact)
    check_similar: bool,
}

impl DuplicateCheckerPlugin {
    pub fn new() -> Self {
        Self {
            known_urls: Mutex::new(HashSet::new()),
            block_duplicates: false, // Default to warn only
            check_similar: true,
        }
    }

    /// Normalize URL for comparison
    fn normalize_url(url: &str) -> String {
        let mut url = url.to_lowercase();

        // Remove protocol
        url = url
            .strip_prefix("https://")
            .or_else(|| url.strip_prefix("http://"))
            .unwrap_or(&url)
            .to_string();

        // Remove www prefix
        url = url
            .strip_prefix("www.")
            .unwrap_or(&url)
            .to_string();

        // Remove trailing slash
        url = url.trim_end_matches('/').to_string();

        // Remove query string for similarity check
        if let Some(idx) = url.find('?') {
            url.truncate(idx);
        }

        // Remove fragment
        if let Some(idx) = url.find('#') {
            url.truncate(idx);
        }

        url
    }

    /// Check if URL is similar to any known URL
    fn find_similar(&self, url: &str) -> Option<String> {
        let normalized = Self::normalize_url(url);
        let known = self.known_urls.lock().ok()?;

        if known.contains(&normalized) {
            return Some(normalized);
        }

        // Check for partial matches (same domain)
        let domain = normalized.split('/').next()?;
        for known_url in known.iter() {
            if known_url.starts_with(domain) {
                // Same domain, might be similar
                let known_path = known_url.strip_prefix(domain).unwrap_or("");
                let new_path = normalized.strip_prefix(domain).unwrap_or("");

                // If paths are very similar (edit distance could be used here)
                if !known_path.is_empty() && !new_path.is_empty() {
                    // Simple check: same path prefix
                    let known_parts: Vec<_> = known_path.split('/').collect();
                    let new_parts: Vec<_> = new_path.split('/').collect();

                    if known_parts.len() > 1 && new_parts.len() > 1 {
                        if known_parts[..known_parts.len()-1] == new_parts[..new_parts.len()-1] {
                            return Some(known_url.clone());
                        }
                    }
                }
            }
        }

        None
    }

    /// Add URL to known set
    fn add_known(&self, url: &str) {
        let normalized = Self::normalize_url(url);
        if let Ok(mut known) = self.known_urls.lock() {
            known.insert(normalized);
        }
    }
}

impl Default for DuplicateCheckerPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for DuplicateCheckerPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "duplicate-checker".to_string(),
            version: "1.0.0".to_string(),
            description: "Warns about duplicate or similar URLs".to_string(),
            author: "bukurs".to_string(),
        }
    }

    fn on_load(&mut self, ctx: &PluginContext) -> HookResult {
        if let Some(block) = ctx.config.get("block_duplicates") {
            self.block_duplicates = block == "true";
        }
        if let Some(similar) = ctx.config.get("check_similar") {
            self.check_similar = similar != "false";
        }
        HookResult::Continue
    }

    fn on_pre_add(&self, _ctx: &PluginContext, bookmark: &mut Bookmark) -> HookResult {
        if self.check_similar {
            if let Some(similar) = self.find_similar(&bookmark.url) {
                let msg = format!(
                    "Similar URL already exists: {}",
                    similar
                );

                if self.block_duplicates {
                    return HookResult::Error(msg);
                } else {
                    log::warn!("{}", msg);
                }
            }
        }

        HookResult::Continue
    }

    fn on_post_add(&self, _ctx: &PluginContext, bookmark: &Bookmark) -> HookResult {
        self.add_known(&bookmark.url);
        HookResult::Continue
    }
}

/// Create an instance of this plugin (required for auto-discovery)
pub fn create_plugin() -> Box<dyn Plugin> {
    Box::new(DuplicateCheckerPlugin::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_url() {
        assert_eq!(
            DuplicateCheckerPlugin::normalize_url("https://www.example.com/path/"),
            "example.com/path"
        );
        assert_eq!(
            DuplicateCheckerPlugin::normalize_url("http://example.com/path?query=1"),
            "example.com/path"
        );
        assert_eq!(
            DuplicateCheckerPlugin::normalize_url("HTTPS://EXAMPLE.COM"),
            "example.com"
        );
    }

    #[test]
    fn test_find_similar() {
        let plugin = DuplicateCheckerPlugin::new();
        plugin.add_known("https://github.com/user/repo");

        // Exact match (normalized)
        assert!(plugin.find_similar("https://www.github.com/user/repo/").is_some());

        // Different repo - should not match
        assert!(plugin.find_similar("https://github.com/other/project").is_none());
    }
}
