//! Pinned Bookmarks Plugin
//!
//! Pin important bookmarks to show them first in search results:
//! - Tag a bookmark with "pinned" to pin it
//! - Pinned bookmarks appear at the top of search results
//! - Supports priority levels: "pinned:1", "pinned:2", etc.

use bukurs::models::bookmark::Bookmark;
use bukurs::plugin::{HookResult, Plugin, PluginContext, PluginInfo, SearchContext};

pub struct PinnedPlugin {
    /// Whether the plugin is enabled
    enabled: bool,
    /// Tag prefix for pinned bookmarks
    pinned_prefix: String,
    /// Simple pinned tag (no priority)
    pinned_tag: String,
}

impl PinnedPlugin {
    pub fn new() -> Self {
        Self {
            enabled: true,
            pinned_prefix: "pinned:".to_string(),
            pinned_tag: "pinned".to_string(),
        }
    }

    /// Extract pin priority from tags (lower = higher priority)
    /// Returns None if not pinned, Some(priority) if pinned
    fn get_pin_priority(&self, tags: &str) -> Option<u32> {
        let tags_lower = tags.to_lowercase();
        let tag_list: Vec<&str> = tags_lower.trim_matches(',').split(',').collect();

        for tag in tag_list {
            // Check for priority tag like "pinned:1"
            if let Some(priority_str) = tag.strip_prefix(&self.pinned_prefix) {
                if let Ok(priority) = priority_str.parse::<u32>() {
                    return Some(priority);
                }
            }
            // Check for simple "pinned" tag (default priority 0)
            if tag == self.pinned_tag {
                return Some(0);
            }
        }

        None
    }

    /// Check if bookmark is pinned
    fn is_pinned(&self, tags: &str) -> bool {
        self.get_pin_priority(tags).is_some()
    }
}

impl Default for PinnedPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for PinnedPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "pinned".to_string(),
            version: "1.0.0".to_string(),
            description: "Pin important bookmarks to show first".to_string(),
            author: "bukurs".to_string(),
        }
    }

    fn on_load(&mut self, ctx: &PluginContext) -> HookResult {
        if let Some(enabled) = ctx.config.get("enabled") {
            self.enabled = enabled != "false";
        }
        if let Some(prefix) = ctx.config.get("pinned_prefix") {
            self.pinned_prefix = prefix.clone();
        }
        if let Some(tag) = ctx.config.get("pinned_tag") {
            self.pinned_tag = tag.clone();
        }
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

        // Sort results: pinned first (by priority), then non-pinned
        results.sort_by(|a, b| {
            let a_priority = self.get_pin_priority(&a.tags);
            let b_priority = self.get_pin_priority(&b.tags);

            match (a_priority, b_priority) {
                // Both pinned: sort by priority (lower first)
                (Some(a_p), Some(b_p)) => a_p.cmp(&b_p),
                // Only a is pinned: a comes first
                (Some(_), None) => std::cmp::Ordering::Less,
                // Only b is pinned: b comes first
                (None, Some(_)) => std::cmp::Ordering::Greater,
                // Neither pinned: keep original order
                (None, None) => std::cmp::Ordering::Equal,
            }
        });

        HookResult::Continue
    }
}

/// Create an instance of this plugin (required for auto-discovery)
pub fn create_plugin() -> Box<dyn Plugin> {
    Box::new(PinnedPlugin::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_pinned() {
        let plugin = PinnedPlugin::new();
        assert!(plugin.is_pinned(",pinned,rust,"));
        assert!(plugin.is_pinned(",rust,pinned,web,"));
        assert!(!plugin.is_pinned(",rust,web,"));
    }

    #[test]
    fn test_get_pin_priority() {
        let plugin = PinnedPlugin::new();

        // Simple pinned tag has priority 0
        assert_eq!(plugin.get_pin_priority(",pinned,"), Some(0));

        // Priority tags
        assert_eq!(plugin.get_pin_priority(",pinned:1,"), Some(1));
        assert_eq!(plugin.get_pin_priority(",pinned:5,rust,"), Some(5));

        // Not pinned
        assert_eq!(plugin.get_pin_priority(",rust,web,"), None);
    }

    #[test]
    fn test_sorting() {
        let plugin = PinnedPlugin::new();

        let mut bookmarks = vec![
            Bookmark {
                id: 1,
                url: "https://normal.com".to_string(),
                title: "Normal".to_string(),
                tags: ",rust,".to_string(),
                description: String::new(),
            },
            Bookmark {
                id: 2,
                url: "https://pinned-low.com".to_string(),
                title: "Pinned Low Priority".to_string(),
                tags: ",pinned:2,".to_string(),
                description: String::new(),
            },
            Bookmark {
                id: 3,
                url: "https://pinned-high.com".to_string(),
                title: "Pinned High Priority".to_string(),
                tags: ",pinned:1,".to_string(),
                description: String::new(),
            },
            Bookmark {
                id: 4,
                url: "https://pinned-default.com".to_string(),
                title: "Pinned Default".to_string(),
                tags: ",pinned,".to_string(),
                description: String::new(),
            },
        ];

        // Simulate sort from on_post_search
        bookmarks.sort_by(|a, b| {
            let a_priority = plugin.get_pin_priority(&a.tags);
            let b_priority = plugin.get_pin_priority(&b.tags);

            match (a_priority, b_priority) {
                (Some(a_p), Some(b_p)) => a_p.cmp(&b_p),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            }
        });

        // Expected order: pinned (priority 0), pinned:1, pinned:2, normal
        assert_eq!(bookmarks[0].id, 4); // pinned (priority 0)
        assert_eq!(bookmarks[1].id, 3); // pinned:1
        assert_eq!(bookmarks[2].id, 2); // pinned:2
        assert_eq!(bookmarks[3].id, 1); // normal (not pinned)
    }
}
