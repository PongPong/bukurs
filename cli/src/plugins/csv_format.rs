//! CSV Output Format Plugin
//!
//! This plugin provides CSV (Comma-Separated Values) output format
//! for bookmark exports and displays.
//!
//! # Features
//! - Proper CSV escaping for fields containing commas or quotes
//! - Configurable delimiter
//! - Optional header row
//! - Compatible with spreadsheet applications
//!
//! # Example Output
//! ```csv
//! id,url,title,tags,description
//! 1,"https://example.com","Example Site","rust,programming","A great site"
//! ```

use bukurs::models::bookmark::Bookmark;
use bukurs::plugin::{HookResult, Plugin, PluginContext, PluginInfo};

/// CSV output format plugin
pub struct CsvFormatPlugin {
    /// Delimiter character (default: comma)
    delimiter: char,
    /// Whether to include header row
    include_header: bool,
}

impl CsvFormatPlugin {
    pub fn new() -> Self {
        Self {
            delimiter: ',',
            include_header: true,
        }
    }

    pub fn with_delimiter(mut self, delimiter: char) -> Self {
        self.delimiter = delimiter;
        self
    }

    pub fn with_header(mut self, include_header: bool) -> Self {
        self.include_header = include_header;
        self
    }

    /// Escape a field for CSV format
    fn escape_field(&self, field: &str) -> String {
        let needs_quotes = field.contains(self.delimiter)
            || field.contains('"')
            || field.contains('\n')
            || field.contains('\r');

        if needs_quotes {
            // Escape double quotes by doubling them
            let escaped = field.replace('"', "\"\"");
            format!("\"{}\"", escaped)
        } else {
            field.to_string()
        }
    }

    /// Convert tags from bukurs format (,tag1,tag2,) to readable format (tag1;tag2)
    fn format_tags(&self, tags: &str) -> String {
        tags.trim_matches(',')
            .split(',')
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>()
            .join(";")
    }

    /// Format a bookmark as a CSV row
    pub fn format_bookmark(&self, bookmark: &Bookmark) -> String {
        let d = self.delimiter;
        format!(
            "{}{}{}{}{}{}{}{}{}",
            bookmark.id,
            d,
            self.escape_field(&bookmark.url),
            d,
            self.escape_field(&bookmark.title),
            d,
            self.escape_field(&self.format_tags(&bookmark.tags)),
            d,
            self.escape_field(&bookmark.description)
        )
    }

    /// Format multiple bookmarks as CSV
    pub fn format_bookmarks(&self, bookmarks: &[Bookmark]) -> String {
        let mut output = String::new();

        if self.include_header {
            let d = self.delimiter;
            output.push_str(&format!(
                "id{}url{}title{}tags{}description\n",
                d, d, d, d
            ));
        }

        for bookmark in bookmarks {
            output.push_str(&self.format_bookmark(bookmark));
            output.push('\n');
        }

        output
    }

    /// Get the CSV header
    pub fn header(&self) -> String {
        let d = self.delimiter;
        format!("id{}url{}title{}tags{}description", d, d, d, d)
    }

    /// Get file extension
    pub fn file_extension(&self) -> &str {
        if self.delimiter == '\t' {
            "tsv"
        } else {
            "csv"
        }
    }
}

impl Default for CsvFormatPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for CsvFormatPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "csv-format".to_string(),
            version: "1.0.0".to_string(),
            description: "CSV output format for bookmarks".to_string(),
            author: "bukurs".to_string(),
        }
    }

    fn on_load(&mut self, ctx: &PluginContext) -> HookResult {
        // Check for custom delimiter in config
        if let Some(delimiter) = ctx.config.get("delimiter") {
            if let Some(c) = delimiter.chars().next() {
                self.delimiter = c;
            }
        }

        // Check for header preference
        if let Some(header) = ctx.config.get("include_header") {
            self.include_header = header == "true";
        }

        HookResult::Continue
    }
}

/// Create an instance of this plugin (required for auto-discovery)
pub fn create_plugin() -> Box<dyn Plugin> {
    Box::new(CsvFormatPlugin::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_field_simple() {
        let plugin = CsvFormatPlugin::new();
        assert_eq!(plugin.escape_field("simple"), "simple");
    }

    #[test]
    fn test_escape_field_with_comma() {
        let plugin = CsvFormatPlugin::new();
        assert_eq!(plugin.escape_field("hello, world"), "\"hello, world\"");
    }

    #[test]
    fn test_escape_field_with_quotes() {
        let plugin = CsvFormatPlugin::new();
        assert_eq!(plugin.escape_field("say \"hello\""), "\"say \"\"hello\"\"\"");
    }

    #[test]
    fn test_format_tags() {
        let plugin = CsvFormatPlugin::new();
        assert_eq!(plugin.format_tags(",rust,programming,"), "rust;programming");
        assert_eq!(plugin.format_tags(",single,"), "single");
        assert_eq!(plugin.format_tags(""), "");
    }

    #[test]
    fn test_format_bookmark() {
        let plugin = CsvFormatPlugin::new();
        let bookmark = Bookmark::new(
            1,
            "https://example.com".to_string(),
            "Example".to_string(),
            ",rust,web,".to_string(),
            "A test bookmark".to_string(),
        );

        let csv = plugin.format_bookmark(&bookmark);
        assert_eq!(csv, "1,https://example.com,Example,rust;web,A test bookmark");
    }

    #[test]
    fn test_format_bookmarks_with_header() {
        let plugin = CsvFormatPlugin::new();
        let bookmarks = vec![
            Bookmark::new(
                1,
                "https://example.com".to_string(),
                "Example".to_string(),
                ",rust,".to_string(),
                "Test".to_string(),
            ),
        ];

        let csv = plugin.format_bookmarks(&bookmarks);
        assert!(csv.starts_with("id,url,title,tags,description\n"));
        assert!(csv.contains("1,https://example.com,Example,rust,Test"));
    }

    #[test]
    fn test_tsv_format() {
        let plugin = CsvFormatPlugin::new().with_delimiter('\t');
        let bookmark = Bookmark::new(
            1,
            "https://example.com".to_string(),
            "Example".to_string(),
            "".to_string(),
            "".to_string(),
        );

        let tsv = plugin.format_bookmark(&bookmark);
        assert!(tsv.contains('\t'));
        assert_eq!(plugin.file_extension(), "tsv");
    }
}
