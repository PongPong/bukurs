//! Tag Suggester Plugin
//!
//! Suggests tags based on page content analysis:
//! - Extracts keywords from title
//! - Analyzes meta keywords/description
//! - Detects programming languages from URL/content
//! - Identifies content type (article, video, documentation, etc.)

use bukurs::models::bookmark::Bookmark;
use bukurs::plugin::{HookResult, Plugin, PluginContext, PluginInfo};
use std::collections::HashSet;

pub struct TagSuggesterPlugin {
    /// Whether the plugin is enabled
    enabled: bool,
    /// Minimum keyword length to consider
    min_keyword_length: usize,
    /// Common words to ignore
    stop_words: HashSet<String>,
    /// Programming language keywords to detect
    language_keywords: Vec<(&'static str, &'static str)>,
    /// Content type patterns
    content_type_patterns: Vec<(&'static str, &'static str)>,
}

impl TagSuggesterPlugin {
    pub fn new() -> Self {
        let stop_words: HashSet<String> = [
            "the", "a", "an", "and", "or", "but", "in", "on", "at", "to", "for",
            "of", "with", "by", "from", "as", "is", "was", "are", "were", "been",
            "be", "have", "has", "had", "do", "does", "did", "will", "would",
            "could", "should", "may", "might", "must", "shall", "can", "need",
            "this", "that", "these", "those", "i", "you", "he", "she", "it",
            "we", "they", "what", "which", "who", "whom", "how", "when", "where",
            "why", "all", "each", "every", "both", "few", "more", "most", "other",
            "some", "such", "no", "nor", "not", "only", "own", "same", "so",
            "than", "too", "very", "just", "also", "now", "here", "there",
            "about", "into", "over", "after", "before", "between", "under",
            "new", "get", "use", "using",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        let language_keywords = vec![
            ("rust", "rust"),
            ("python", "python"),
            ("javascript", "javascript"),
            ("typescript", "typescript"),
            ("golang", "go"),
            ("java", "java"),
            ("kotlin", "kotlin"),
            ("swift", "swift"),
            ("ruby", "ruby"),
            ("php", "php"),
            ("csharp", "csharp"),
            ("cpp", "cpp"),
            ("c++", "cpp"),
            ("haskell", "haskell"),
            ("scala", "scala"),
            ("elixir", "elixir"),
            ("clojure", "clojure"),
            ("react", "react"),
            ("vue", "vue"),
            ("angular", "angular"),
            ("node", "nodejs"),
            ("django", "django"),
            ("flask", "flask"),
            ("rails", "rails"),
            ("spring", "spring"),
            ("docker", "docker"),
            ("kubernetes", "kubernetes"),
            ("k8s", "kubernetes"),
            ("aws", "aws"),
            ("azure", "azure"),
            ("gcp", "gcp"),
            ("linux", "linux"),
            ("git", "git"),
            ("sql", "sql"),
            ("postgresql", "postgresql"),
            ("mongodb", "mongodb"),
            ("redis", "redis"),
            ("graphql", "graphql"),
            ("api", "api"),
            ("rest", "rest"),
            ("machine-learning", "ml"),
            ("deep-learning", "ml"),
            ("neural", "ml"),
            ("tensorflow", "ml"),
            ("pytorch", "ml"),
        ];

        let content_type_patterns = vec![
            ("tutorial", "tutorial"),
            ("guide", "guide"),
            ("documentation", "docs"),
            ("docs", "docs"),
            ("reference", "reference"),
            ("blog", "blog"),
            ("article", "article"),
            ("video", "video"),
            ("course", "course"),
            ("book", "book"),
            ("cheatsheet", "cheatsheet"),
            ("cheat-sheet", "cheatsheet"),
            ("awesome", "awesome-list"),
            ("list", "list"),
            ("tools", "tools"),
            ("library", "library"),
            ("framework", "framework"),
            ("example", "example"),
            ("sample", "example"),
            ("demo", "demo"),
            ("template", "template"),
            ("starter", "starter"),
            ("boilerplate", "boilerplate"),
        ];

        Self {
            enabled: true,
            min_keyword_length: 3,
            stop_words,
            language_keywords,
            content_type_patterns,
        }
    }

    /// Extract potential tags from title
    fn extract_from_title(&self, title: &str) -> Vec<String> {
        let mut tags = Vec::new();
        let title_lower = title.to_lowercase();

        // Check for programming languages
        for (keyword, tag) in &self.language_keywords {
            if title_lower.contains(keyword) {
                tags.push(tag.to_string());
            }
        }

        // Check for content types
        for (pattern, tag) in &self.content_type_patterns {
            if title_lower.contains(pattern) {
                tags.push(tag.to_string());
            }
        }

        // Extract significant words
        let words: Vec<&str> = title_lower
            .split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
            .filter(|w| {
                w.len() >= self.min_keyword_length
                    && !self.stop_words.contains(*w)
                    && !w.chars().all(|c| c.is_numeric())
            })
            .collect();

        // Add words that appear significant (capitalized in original, etc.)
        for word in words {
            // Don't duplicate language/content tags
            if !tags.contains(&word.to_string()) {
                // Only add if it seems significant
                if word.len() >= 4 {
                    // Check if it's a known tech term
                    for (keyword, tag) in &self.language_keywords {
                        if word == *keyword {
                            if !tags.contains(&tag.to_string()) {
                                tags.push(tag.to_string());
                            }
                            break;
                        }
                    }
                }
            }
        }

        tags
    }

    /// Extract tags from URL path
    fn extract_from_url(&self, url: &str) -> Vec<String> {
        let mut tags = Vec::new();

        // Parse URL path
        let url_lower = url.to_lowercase();

        // Check for programming languages in URL
        for (keyword, tag) in &self.language_keywords {
            if url_lower.contains(&format!("/{}/", keyword))
                || url_lower.contains(&format!("/{}-", keyword))
                || url_lower.contains(&format!("-{}/", keyword))
                || url_lower.contains(&format!("-{}-", keyword))
            {
                tags.push(tag.to_string());
            }
        }

        // Check for content types in URL
        for (pattern, tag) in &self.content_type_patterns {
            if url_lower.contains(&format!("/{}/", pattern))
                || url_lower.contains(&format!("/{}", pattern))
            {
                tags.push(tag.to_string());
            }
        }

        tags
    }

    /// Merge new tags with existing ones
    fn merge_tags(existing: &str, new_tags: &[String]) -> String {
        let mut all_tags: Vec<String> = existing
            .trim_matches(',')
            .split(',')
            .filter(|t| !t.is_empty())
            .map(|t| t.to_string())
            .collect();

        for tag in new_tags {
            if !all_tags.contains(tag) {
                all_tags.push(tag.clone());
            }
        }

        if all_tags.is_empty() {
            String::new()
        } else {
            format!(",{},", all_tags.join(","))
        }
    }
}

impl Default for TagSuggesterPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for TagSuggesterPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "tag-suggester".to_string(),
            version: "1.0.0".to_string(),
            description: "Suggests tags based on title and URL analysis".to_string(),
            author: "bukurs".to_string(),
        }
    }

    fn on_load(&mut self, ctx: &PluginContext) -> HookResult {
        if let Some(enabled) = ctx.config.get("enabled") {
            self.enabled = enabled != "false";
        }
        if let Some(min_len) = ctx.config.get("min_keyword_length") {
            self.min_keyword_length = min_len.parse().unwrap_or(3);
        }
        HookResult::Continue
    }

    fn on_pre_add(&self, _ctx: &PluginContext, bookmark: &mut Bookmark) -> HookResult {
        if !self.enabled {
            return HookResult::Continue;
        }

        let mut suggested_tags = Vec::new();

        // Extract from title
        suggested_tags.extend(self.extract_from_title(&bookmark.title));

        // Extract from URL
        suggested_tags.extend(self.extract_from_url(&bookmark.url));

        // Deduplicate
        let mut seen = HashSet::new();
        suggested_tags.retain(|t| seen.insert(t.clone()));

        if !suggested_tags.is_empty() {
            bookmark.tags = Self::merge_tags(&bookmark.tags, &suggested_tags);
            log::debug!("Suggested tags: {:?}", suggested_tags);
        }

        HookResult::Continue
    }
}

/// Create an instance of this plugin (required for auto-discovery)
pub fn create_plugin() -> Box<dyn Plugin> {
    Box::new(TagSuggesterPlugin::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_from_title() {
        let plugin = TagSuggesterPlugin::new();

        let tags = plugin.extract_from_title("Rust Tutorial: Getting Started");
        assert!(tags.contains(&"rust".to_string()));
        assert!(tags.contains(&"tutorial".to_string()));

        let tags = plugin.extract_from_title("Python Deep-Learning Guide");
        assert!(tags.contains(&"python".to_string()));
        assert!(tags.contains(&"ml".to_string()));
        assert!(tags.contains(&"guide".to_string()));
    }

    #[test]
    fn test_extract_from_url() {
        let plugin = TagSuggesterPlugin::new();

        let tags = plugin.extract_from_url("https://example.com/rust/tutorial/basics");
        assert!(tags.contains(&"rust".to_string()));
        assert!(tags.contains(&"tutorial".to_string()));
    }

    #[test]
    fn test_merge_tags() {
        let merged = TagSuggesterPlugin::merge_tags(",existing,tags,", &["new".to_string(), "tags".to_string()]);
        assert!(merged.contains("existing"));
        assert!(merged.contains("new"));
        // "tags" should not be duplicated
        assert_eq!(merged.matches("tags").count(), 1);
    }
}
