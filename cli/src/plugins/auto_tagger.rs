//! Auto-Tagger Plugin
//!
//! This plugin automatically adds tags to bookmarks based on URL patterns.
//! It runs as a pre-add hook, analyzing the URL and adding relevant tags
//! before the bookmark is saved.
//!
//! # Default Rules
//! - `github.com` -> adds `github`, `code`
//! - `stackoverflow.com` -> adds `stackoverflow`, `programming`
//! - `youtube.com` -> adds `youtube`, `video`
//! - `docs.rs`, `doc.rust-lang.org` -> adds `rust`, `docs`
//! - `*.edu` -> adds `education`
//! - `news.*`, `*.news.*` -> adds `news`
//!
//! # Configuration
//! Rules can be customized via plugin config or by calling `add_rule()`.

use bukurs::models::bookmark::Bookmark;
use bukurs::plugin::{HookResult, Plugin, PluginContext, PluginInfo};
use regex::Regex;

/// A tagging rule that matches URLs against a pattern
#[derive(Debug, Clone)]
pub struct TagRule {
    /// Pattern to match (can be domain or regex)
    pub pattern: String,
    /// Compiled regex for matching
    regex: Regex,
    /// Tags to add when pattern matches
    pub tags: Vec<String>,
}

impl TagRule {
    /// Create a new rule from a domain pattern
    pub fn from_domain(domain: &str, tags: Vec<String>) -> Self {
        // Convert domain pattern to regex
        let pattern = domain
            .replace(".", r"\.")
            .replace("*", r"[^/]*");
        let regex_pattern = format!(r"https?://([^/]*\.)?{}(/|$)", pattern);

        Self {
            pattern: domain.to_string(),
            regex: Regex::new(&regex_pattern).unwrap_or_else(|_| Regex::new("^$").unwrap()),
            tags,
        }
    }

    /// Create a new rule from a regex pattern
    pub fn from_regex(pattern: &str, tags: Vec<String>) -> Option<Self> {
        Regex::new(pattern).ok().map(|regex| Self {
            pattern: pattern.to_string(),
            regex,
            tags,
        })
    }

    /// Check if a URL matches this rule
    pub fn matches(&self, url: &str) -> bool {
        self.regex.is_match(url)
    }
}

/// Auto-tagger plugin that adds tags based on URL patterns
pub struct AutoTaggerPlugin {
    /// Tagging rules
    rules: Vec<TagRule>,
    /// Whether to preserve existing tags
    preserve_existing: bool,
    /// Whether the plugin is enabled
    enabled: bool,
}

impl AutoTaggerPlugin {
    pub fn new() -> Self {
        let mut plugin = Self {
            rules: Vec::new(),
            preserve_existing: true,
            enabled: true,
        };

        // Add default rules
        plugin.add_default_rules();
        plugin
    }

    /// Add default tagging rules
    fn add_default_rules(&mut self) {
        // GitHub
        self.add_rule(TagRule::from_domain(
            "github.com",
            vec!["github".to_string(), "code".to_string()],
        ));
        self.add_rule(TagRule::from_domain(
            "gitlab.com",
            vec!["gitlab".to_string(), "code".to_string()],
        ));

        // Stack Overflow / Stack Exchange
        self.add_rule(TagRule::from_domain(
            "stackoverflow.com",
            vec!["stackoverflow".to_string(), "programming".to_string()],
        ));
        self.add_rule(TagRule::from_domain(
            "*.stackexchange.com",
            vec!["stackexchange".to_string()],
        ));

        // Video platforms
        self.add_rule(TagRule::from_domain(
            "youtube.com",
            vec!["youtube".to_string(), "video".to_string()],
        ));
        self.add_rule(TagRule::from_domain(
            "youtu.be",
            vec!["youtube".to_string(), "video".to_string()],
        ));
        self.add_rule(TagRule::from_domain(
            "vimeo.com",
            vec!["vimeo".to_string(), "video".to_string()],
        ));

        // Rust documentation
        self.add_rule(TagRule::from_domain(
            "docs.rs",
            vec!["rust".to_string(), "docs".to_string()],
        ));
        self.add_rule(TagRule::from_domain(
            "doc.rust-lang.org",
            vec!["rust".to_string(), "docs".to_string()],
        ));
        self.add_rule(TagRule::from_domain(
            "crates.io",
            vec!["rust".to_string(), "crates".to_string()],
        ));

        // Documentation sites
        self.add_rule(TagRule::from_domain(
            "developer.mozilla.org",
            vec!["mdn".to_string(), "docs".to_string(), "web".to_string()],
        ));
        self.add_rule(TagRule::from_domain(
            "devdocs.io",
            vec!["docs".to_string()],
        ));

        // Social media
        self.add_rule(TagRule::from_domain(
            "twitter.com",
            vec!["twitter".to_string(), "social".to_string()],
        ));
        self.add_rule(TagRule::from_domain(
            "x.com",
            vec!["twitter".to_string(), "social".to_string()],
        ));
        self.add_rule(TagRule::from_domain(
            "reddit.com",
            vec!["reddit".to_string(), "social".to_string()],
        ));
        self.add_rule(TagRule::from_domain(
            "linkedin.com",
            vec!["linkedin".to_string(), "professional".to_string()],
        ));

        // News sites
        self.add_rule(TagRule::from_domain(
            "news.ycombinator.com",
            vec!["hackernews".to_string(), "news".to_string(), "tech".to_string()],
        ));

        // Wikipedia
        self.add_rule(TagRule::from_domain(
            "*.wikipedia.org",
            vec!["wikipedia".to_string(), "reference".to_string()],
        ));

        // Cloud providers
        self.add_rule(TagRule::from_domain(
            "aws.amazon.com",
            vec!["aws".to_string(), "cloud".to_string()],
        ));
        self.add_rule(TagRule::from_domain(
            "cloud.google.com",
            vec!["gcp".to_string(), "cloud".to_string()],
        ));
        self.add_rule(TagRule::from_domain(
            "azure.microsoft.com",
            vec!["azure".to_string(), "cloud".to_string()],
        ));

        // Package managers
        self.add_rule(TagRule::from_domain(
            "npmjs.com",
            vec!["npm".to_string(), "javascript".to_string()],
        ));
        self.add_rule(TagRule::from_domain(
            "pypi.org",
            vec!["pypi".to_string(), "python".to_string()],
        ));
    }

    /// Add a tagging rule
    pub fn add_rule(&mut self, rule: TagRule) {
        self.rules.push(rule);
    }

    /// Remove all rules
    pub fn clear_rules(&mut self) {
        self.rules.clear();
    }

    /// Get matching tags for a URL
    pub fn get_tags_for_url(&self, url: &str) -> Vec<String> {
        let mut tags = Vec::new();

        for rule in &self.rules {
            if rule.matches(url) {
                for tag in &rule.tags {
                    if !tags.contains(tag) {
                        tags.push(tag.clone());
                    }
                }
            }
        }

        tags
    }

    /// Parse existing tags from bukurs format
    fn parse_tags(tags: &str) -> Vec<String> {
        tags.trim_matches(',')
            .split(',')
            .filter(|t| !t.is_empty())
            .map(|t| t.to_string())
            .collect()
    }

    /// Format tags to bukurs format
    fn format_tags(tags: &[String]) -> String {
        if tags.is_empty() {
            String::new()
        } else {
            format!(",{},", tags.join(","))
        }
    }

    /// Merge new tags with existing tags
    fn merge_tags(existing: &str, new_tags: &[String]) -> String {
        let mut all_tags = Self::parse_tags(existing);

        for tag in new_tags {
            if !all_tags.contains(tag) {
                all_tags.push(tag.clone());
            }
        }

        Self::format_tags(&all_tags)
    }
}

impl Default for AutoTaggerPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for AutoTaggerPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "auto-tagger".to_string(),
            version: "1.0.0".to_string(),
            description: "Automatically tags bookmarks based on URL patterns".to_string(),
            author: "bukurs".to_string(),
        }
    }

    fn on_load(&mut self, ctx: &PluginContext) -> HookResult {
        // Check for enabled config
        if let Some(enabled) = ctx.config.get("enabled") {
            self.enabled = enabled != "false";
        }

        // Check for preserve_existing config
        if let Some(preserve) = ctx.config.get("preserve_existing") {
            self.preserve_existing = preserve != "false";
        }

        // Load custom rules from config
        // Format: "rule.github.com=github,code"
        for (key, value) in &ctx.config {
            if key.starts_with("rule.") {
                let domain = key.strip_prefix("rule.").unwrap();
                let tags: Vec<String> = value.split(',').map(|s| s.trim().to_string()).collect();
                self.add_rule(TagRule::from_domain(domain, tags));
            }
        }

        HookResult::Continue
    }

    fn on_pre_add(&self, _ctx: &PluginContext, bookmark: &mut Bookmark) -> HookResult {
        if !self.enabled {
            return HookResult::Continue;
        }

        let new_tags = self.get_tags_for_url(&bookmark.url);

        if !new_tags.is_empty() {
            if self.preserve_existing {
                bookmark.tags = Self::merge_tags(&bookmark.tags, &new_tags);
            } else {
                bookmark.tags = Self::format_tags(&new_tags);
            }
        }

        HookResult::Continue
    }

    fn on_pre_update(
        &self,
        _ctx: &PluginContext,
        old: &Bookmark,
        new: &mut Bookmark,
    ) -> HookResult {
        if !self.enabled {
            return HookResult::Continue;
        }

        // Only re-tag if URL changed
        if old.url != new.url {
            let new_tags = self.get_tags_for_url(&new.url);
            if !new_tags.is_empty() {
                if self.preserve_existing {
                    new.tags = Self::merge_tags(&new.tags, &new_tags);
                } else {
                    new.tags = Self::format_tags(&new_tags);
                }
            }
        }

        HookResult::Continue
    }
}

/// Create an instance of this plugin (required for auto-discovery)
pub fn create_plugin() -> Box<dyn Plugin> {
    Box::new(AutoTaggerPlugin::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tag_rule_domain_match() {
        let rule = TagRule::from_domain("github.com", vec!["github".to_string()]);

        assert!(rule.matches("https://github.com/user/repo"));
        assert!(rule.matches("http://github.com/"));
        assert!(rule.matches("https://www.github.com/user/repo"));
        assert!(!rule.matches("https://notgithub.com/"));
        assert!(!rule.matches("https://example.com/github.com"));
    }

    #[test]
    fn test_tag_rule_wildcard() {
        let rule = TagRule::from_domain("*.wikipedia.org", vec!["wikipedia".to_string()]);

        assert!(rule.matches("https://en.wikipedia.org/wiki/Rust"));
        assert!(rule.matches("https://de.wikipedia.org/wiki/Test"));
        assert!(!rule.matches("https://wikipedia.com/"));
    }

    #[test]
    fn test_get_tags_for_url() {
        let plugin = AutoTaggerPlugin::new();

        let tags = plugin.get_tags_for_url("https://github.com/rust-lang/rust");
        assert!(tags.contains(&"github".to_string()));
        assert!(tags.contains(&"code".to_string()));

        let tags = plugin.get_tags_for_url("https://docs.rs/serde");
        assert!(tags.contains(&"rust".to_string()));
        assert!(tags.contains(&"docs".to_string()));
    }

    #[test]
    fn test_merge_tags() {
        let existing = ",rust,web,";
        let new_tags = vec!["github".to_string(), "rust".to_string()];

        let merged = AutoTaggerPlugin::merge_tags(existing, &new_tags);
        assert!(merged.contains("rust"));
        assert!(merged.contains("web"));
        assert!(merged.contains("github"));
        // Should not have duplicate "rust"
        assert_eq!(merged.matches("rust").count(), 1);
    }

    #[test]
    fn test_parse_and_format_tags() {
        let tags_str = ",rust,web,programming,";
        let parsed = AutoTaggerPlugin::parse_tags(tags_str);
        assert_eq!(parsed, vec!["rust", "web", "programming"]);

        let formatted = AutoTaggerPlugin::format_tags(&parsed);
        assert_eq!(formatted, ",rust,web,programming,");
    }

    #[test]
    fn test_on_pre_add_auto_tags() {
        let plugin = AutoTaggerPlugin::new();
        let ctx = PluginContext::new(
            std::path::PathBuf::from("/test/db"),
            std::path::PathBuf::from("/test/data"),
        );

        let mut bookmark = Bookmark::new(
            0,
            "https://github.com/user/repo".to_string(),
            "Test Repo".to_string(),
            "".to_string(),
            "".to_string(),
        );

        plugin.on_pre_add(&ctx, &mut bookmark);

        assert!(bookmark.tags.contains("github"));
        assert!(bookmark.tags.contains("code"));
    }

    #[test]
    fn test_preserves_existing_tags() {
        let plugin = AutoTaggerPlugin::new();
        let ctx = PluginContext::new(
            std::path::PathBuf::from("/test/db"),
            std::path::PathBuf::from("/test/data"),
        );

        let mut bookmark = Bookmark::new(
            0,
            "https://github.com/user/repo".to_string(),
            "Test Repo".to_string(),
            ",existing,custom,".to_string(),
            "".to_string(),
        );

        plugin.on_pre_add(&ctx, &mut bookmark);

        assert!(bookmark.tags.contains("existing"));
        assert!(bookmark.tags.contains("custom"));
        assert!(bookmark.tags.contains("github"));
    }
}
