//! RSS Feed Plugin
//!
//! Generates an RSS/Atom feed of your bookmarks:
//! - Serves bookmarks as an RSS feed
//! - Can filter by tags
//! - Useful for sharing or subscribing to your own bookmarks
//! - Outputs to a file that can be served by any web server

use bukurs::models::bookmark::Bookmark;
use bukurs::plugin::{HookResult, Plugin, PluginContext, PluginInfo};
use chrono::{DateTime, Utc};
use std::fs;
use std::path::PathBuf;

pub struct RssFeedPlugin {
    /// Output file path for the RSS feed
    output_path: Option<PathBuf>,
    /// Feed title
    feed_title: String,
    /// Feed description
    feed_description: String,
    /// Feed link (your website)
    feed_link: String,
    /// Maximum items in feed
    max_items: usize,
    /// Tags to include (empty = all)
    include_tags: Vec<String>,
    /// Tags to exclude
    exclude_tags: Vec<String>,
    /// Whether to regenerate on every change
    auto_regenerate: bool,
    /// Whether the plugin is enabled
    enabled: bool,
    /// Database path for regeneration
    db_path: Option<PathBuf>,
}

impl RssFeedPlugin {
    pub fn new() -> Self {
        Self {
            output_path: None,
            feed_title: "My Bookmarks".to_string(),
            feed_description: "Bookmarks RSS Feed".to_string(),
            feed_link: "https://example.com".to_string(),
            max_items: 50,
            include_tags: Vec::new(),
            exclude_tags: vec!["private".to_string()],
            auto_regenerate: true,
            enabled: false, // Disabled by default
            db_path: None,
        }
    }

    /// Check if bookmark should be included in feed
    fn should_include(&self, bookmark: &Bookmark) -> bool {
        let tags: Vec<&str> = bookmark.tags
            .trim_matches(',')
            .split(',')
            .filter(|t| !t.is_empty())
            .collect();

        // Check exclude tags
        for exclude in &self.exclude_tags {
            if tags.contains(&exclude.as_str()) {
                return false;
            }
        }

        // Check include tags (if specified)
        if !self.include_tags.is_empty() {
            for include in &self.include_tags {
                if tags.contains(&include.as_str()) {
                    return true;
                }
            }
            return false;
        }

        true
    }

    /// Escape XML special characters
    fn escape_xml(s: &str) -> String {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&apos;")
    }

    /// Format tags for display
    fn format_tags(tags: &str) -> String {
        tags.trim_matches(',')
            .split(',')
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Generate RSS 2.0 feed
    fn generate_rss(&self, bookmarks: &[Bookmark]) -> String {
        let now: DateTime<Utc> = Utc::now();
        let pub_date = now.format("%a, %d %b %Y %H:%M:%S GMT").to_string();

        let mut rss = String::new();
        rss.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
        rss.push('\n');
        rss.push_str(r#"<rss version="2.0" xmlns:atom="http://www.w3.org/2005/Atom">"#);
        rss.push('\n');
        rss.push_str("  <channel>\n");
        rss.push_str(&format!("    <title>{}</title>\n", Self::escape_xml(&self.feed_title)));
        rss.push_str(&format!("    <link>{}</link>\n", Self::escape_xml(&self.feed_link)));
        rss.push_str(&format!("    <description>{}</description>\n", Self::escape_xml(&self.feed_description)));
        rss.push_str(&format!("    <pubDate>{}</pubDate>\n", pub_date));
        rss.push_str(&format!("    <lastBuildDate>{}</lastBuildDate>\n", pub_date));
        rss.push_str("    <generator>bukurs RSS Plugin</generator>\n");

        // Add items
        let filtered: Vec<_> = bookmarks.iter()
            .filter(|b| self.should_include(b))
            .take(self.max_items)
            .collect();

        for bookmark in filtered {
            rss.push_str("    <item>\n");
            rss.push_str(&format!("      <title>{}</title>\n", Self::escape_xml(&bookmark.title)));
            rss.push_str(&format!("      <link>{}</link>\n", Self::escape_xml(&bookmark.url)));
            rss.push_str(&format!("      <guid isPermaLink=\"true\">{}</guid>\n", Self::escape_xml(&bookmark.url)));

            // Description with tags
            let mut desc = bookmark.description.clone();
            let tags = Self::format_tags(&bookmark.tags);
            if !tags.is_empty() {
                if !desc.is_empty() {
                    desc.push_str("\n\n");
                }
                desc.push_str(&format!("Tags: {}", tags));
            }
            if !desc.is_empty() {
                rss.push_str(&format!("      <description>{}</description>\n", Self::escape_xml(&desc)));
            }

            // Add categories for tags
            for tag in bookmark.tags.trim_matches(',').split(',').filter(|t| !t.is_empty()) {
                rss.push_str(&format!("      <category>{}</category>\n", Self::escape_xml(tag)));
            }

            rss.push_str("    </item>\n");
        }

        rss.push_str("  </channel>\n");
        rss.push_str("</rss>\n");

        rss
    }

    /// Generate Atom feed
    fn generate_atom(&self, bookmarks: &[Bookmark]) -> String {
        let now: DateTime<Utc> = Utc::now();
        let updated = now.format("%Y-%m-%dT%H:%M:%SZ").to_string();

        let mut atom = String::new();
        atom.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
        atom.push('\n');
        atom.push_str(r#"<feed xmlns="http://www.w3.org/2005/Atom">"#);
        atom.push('\n');
        atom.push_str(&format!("  <title>{}</title>\n", Self::escape_xml(&self.feed_title)));
        atom.push_str(&format!("  <link href=\"{}\"/>\n", Self::escape_xml(&self.feed_link)));
        atom.push_str(&format!("  <updated>{}</updated>\n", updated));
        atom.push_str(&format!("  <id>{}</id>\n", Self::escape_xml(&self.feed_link)));
        atom.push_str(&format!("  <subtitle>{}</subtitle>\n", Self::escape_xml(&self.feed_description)));
        atom.push_str("  <generator>bukurs RSS Plugin</generator>\n");

        // Add entries
        let filtered: Vec<_> = bookmarks.iter()
            .filter(|b| self.should_include(b))
            .take(self.max_items)
            .collect();

        for bookmark in filtered {
            atom.push_str("  <entry>\n");
            atom.push_str(&format!("    <title>{}</title>\n", Self::escape_xml(&bookmark.title)));
            atom.push_str(&format!("    <link href=\"{}\"/>\n", Self::escape_xml(&bookmark.url)));
            atom.push_str(&format!("    <id>{}</id>\n", Self::escape_xml(&bookmark.url)));
            atom.push_str(&format!("    <updated>{}</updated>\n", updated));

            if !bookmark.description.is_empty() {
                atom.push_str(&format!("    <summary>{}</summary>\n", Self::escape_xml(&bookmark.description)));
            }

            // Add categories for tags
            for tag in bookmark.tags.trim_matches(',').split(',').filter(|t| !t.is_empty()) {
                atom.push_str(&format!("    <category term=\"{}\"/>\n", Self::escape_xml(tag)));
            }

            atom.push_str("  </entry>\n");
        }

        atom.push_str("</feed>\n");

        atom
    }

    /// Regenerate the feed file
    fn regenerate_feed(&self) {
        let output = match &self.output_path {
            Some(p) => p,
            None => return,
        };

        let db_path = match &self.db_path {
            Some(p) => p,
            None => return,
        };

        // Read all bookmarks
        let db = match bukurs::db::BukuDb::init(db_path) {
            Ok(db) => db,
            Err(e) => {
                log::error!("Failed to open database for RSS generation: {}", e);
                return;
            }
        };

        let bookmarks = match db.get_rec_all() {
            Ok(b) => b,
            Err(e) => {
                log::error!("Failed to read bookmarks for RSS: {}", e);
                return;
            }
        };

        // Generate feed
        let feed = if output.extension().map(|e| e == "atom").unwrap_or(false) {
            self.generate_atom(&bookmarks)
        } else {
            self.generate_rss(&bookmarks)
        };

        // Write to file
        if let Err(e) = fs::write(output, feed) {
            log::error!("Failed to write RSS feed: {}", e);
        } else {
            log::info!("RSS feed updated: {:?}", output);
        }
    }
}

impl Default for RssFeedPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for RssFeedPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "rss-feed".to_string(),
            version: "1.0.0".to_string(),
            description: "Generates RSS/Atom feed of bookmarks".to_string(),
            author: "bukurs".to_string(),
        }
    }

    fn on_load(&mut self, ctx: &PluginContext) -> HookResult {
        self.db_path = Some(ctx.db_path.clone());

        // Load config
        if let Some(enabled) = ctx.config.get("enabled") {
            self.enabled = enabled == "true";
        }
        if let Some(path) = ctx.config.get("output_path") {
            self.output_path = Some(PathBuf::from(path));
        } else {
            // Default to data dir
            self.output_path = Some(ctx.data_dir.join("bookmarks.rss"));
        }
        if let Some(title) = ctx.config.get("feed_title") {
            self.feed_title = title.clone();
        }
        if let Some(desc) = ctx.config.get("feed_description") {
            self.feed_description = desc.clone();
        }
        if let Some(link) = ctx.config.get("feed_link") {
            self.feed_link = link.clone();
        }
        if let Some(max) = ctx.config.get("max_items") {
            self.max_items = max.parse().unwrap_or(50);
        }
        if let Some(tags) = ctx.config.get("include_tags") {
            self.include_tags = tags.split(',').map(|t| t.trim().to_string()).collect();
        }
        if let Some(tags) = ctx.config.get("exclude_tags") {
            self.exclude_tags = tags.split(',').map(|t| t.trim().to_string()).collect();
        }
        if let Some(auto) = ctx.config.get("auto_regenerate") {
            self.auto_regenerate = auto != "false";
        }

        // Generate initial feed if enabled
        if self.enabled {
            self.regenerate_feed();
        }

        HookResult::Continue
    }

    fn on_post_add(&self, _ctx: &PluginContext, _bookmark: &Bookmark) -> HookResult {
        if self.enabled && self.auto_regenerate {
            self.regenerate_feed();
        }
        HookResult::Continue
    }

    fn on_post_update(
        &self,
        _ctx: &PluginContext,
        _old: &Bookmark,
        _new: &Bookmark,
    ) -> HookResult {
        if self.enabled && self.auto_regenerate {
            self.regenerate_feed();
        }
        HookResult::Continue
    }

    fn on_post_delete(&self, _ctx: &PluginContext, _bookmark: &Bookmark) -> HookResult {
        if self.enabled && self.auto_regenerate {
            self.regenerate_feed();
        }
        HookResult::Continue
    }

    fn on_post_import(&self, _ctx: &PluginContext, _bookmarks: &[Bookmark]) -> HookResult {
        if self.enabled && self.auto_regenerate {
            self.regenerate_feed();
        }
        HookResult::Continue
    }
}

/// Create an instance of this plugin (required for auto-discovery)
pub fn create_plugin() -> Box<dyn Plugin> {
    Box::new(RssFeedPlugin::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_xml() {
        assert_eq!(RssFeedPlugin::escape_xml("a & b"), "a &amp; b");
        assert_eq!(RssFeedPlugin::escape_xml("<tag>"), "&lt;tag&gt;");
        assert_eq!(RssFeedPlugin::escape_xml("\"quoted\""), "&quot;quoted&quot;");
    }

    #[test]
    fn test_format_tags() {
        assert_eq!(RssFeedPlugin::format_tags(",rust,web,"), "rust, web");
        assert_eq!(RssFeedPlugin::format_tags(""), "");
    }

    #[test]
    fn test_should_include() {
        let plugin = RssFeedPlugin::new();

        let public = Bookmark::new(1, "https://example.com".to_string(), "Test".to_string(), ",rust,".to_string(), "".to_string());
        assert!(plugin.should_include(&public));

        let private = Bookmark::new(2, "https://secret.com".to_string(), "Secret".to_string(), ",private,".to_string(), "".to_string());
        assert!(!plugin.should_include(&private));
    }

    #[test]
    fn test_generate_rss() {
        let plugin = RssFeedPlugin::new();
        let bookmarks = vec![
            Bookmark::new(1, "https://example.com".to_string(), "Example".to_string(), ",rust,".to_string(), "A test".to_string()),
        ];

        let rss = plugin.generate_rss(&bookmarks);
        assert!(rss.contains("<rss version=\"2.0\""));
        assert!(rss.contains("<title>Example</title>"));
        assert!(rss.contains("<link>https://example.com</link>"));
        assert!(rss.contains("<category>rust</category>"));
    }

    #[test]
    fn test_plugin_disabled_by_default() {
        let plugin = RssFeedPlugin::new();
        assert!(!plugin.enabled);
    }
}
