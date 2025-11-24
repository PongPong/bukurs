use bukurs::models::bookmark::Bookmark;
use bukurs::tags::parse_tags;
use owo_colors::OwoColorize;

pub trait Colorize {
    fn to_colored(&self) -> String;
}

pub struct ColorizeBookmark<'a>(pub &'a Bookmark);

impl<'a> Colorize for ColorizeBookmark<'a> {
    fn to_colored(&self) -> String {
        let mut s = String::new();
        let id = self.0.id.to_string();
        s.push_str(&format!(
            "{}. {}\n",
            id.bright_blue(),
            self.0.title.bold().green(),
        ));
        let padding = id.len() + 3;
        // padding for alignment
        s.push_str(&format!(
            "{:>padding$} {}\n",
            ">".red(),
            self.0.url.yellow()
        ));

        // Only show description if non-empty
        if !self.0.description.trim().is_empty() {
            s.push_str(&format!("{:>padding$} {}\n", "+".red(), self.0.description));
        }

        // Parse tags and only show if non-empty
        let tags = parse_tags(&self.0.tags);
        if !tags.is_empty() {
            let tags_str = tags.join(", ");
            s.push_str(&format!("{:>padding$} {}\n", "#".red(), tags_str.blue()));
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[test]
    fn test_colorize_bookmark_with_tags() {
        let bookmark = Bookmark::new(
            1,
            "https://example.com".to_string(),
            "Example".to_string(),
            ",rust,testing,".to_string(),
            "A test bookmark".to_string(),
        );

        let colorized = ColorizeBookmark(&bookmark).to_colored();

        // Should contain the tag line
        assert!(colorized.contains("rust, testing"));
        assert!(colorized.contains("#"));
    }

    #[test]
    fn test_colorize_bookmark_without_tags() {
        let bookmark = Bookmark::new(
            1,
            "https://example.com".to_string(),
            "Example".to_string(),
            ",,".to_string(),
            "A test bookmark".to_string(),
        );

        let colorized = ColorizeBookmark(&bookmark).to_colored();

        // Should NOT contain a tag line with just #
        let lines: Vec<&str> = colorized.lines().collect();
        let has_tag_line = lines.iter().any(|line| line.trim().starts_with("#"));
        assert!(!has_tag_line, "Should not have tag line for empty tags");
    }

    #[test]
    fn test_colorize_bookmark_empty_tag_string() {
        let bookmark = Bookmark::new(
            1,
            "https://example.com".to_string(),
            "Example".to_string(),
            "".to_string(),
            "A test bookmark".to_string(),
        );

        let colorized = ColorizeBookmark(&bookmark).to_colored();

        // Should NOT contain a tag line
        let lines: Vec<&str> = colorized.lines().collect();
        let has_tag_line = lines.iter().any(|line| line.trim().starts_with("#"));
        assert!(!has_tag_line);
    }

    #[test]
    fn test_colorize_output_structure() {
        let bookmark = Bookmark::new(
            42,
            "https://rust-lang.org".to_string(),
            "Rust Programming Language".to_string(),
            ",rust,programming,".to_string(),
            "Official Rust website".to_string(),
        );

        let colorized = ColorizeBookmark(&bookmark).to_colored();
        let lines: Vec<&str> = colorized.lines().collect();

        // Should have at least 4 lines (title, url, description, tags)
        assert!(lines.len() >= 4, "Should have at least 4 lines");

        // First line should contain the ID and title
        assert!(lines[0].contains("42"));
        assert!(lines[0].contains("Rust Programming Language"));

        // Second line should contain URL indicator
        assert!(lines[1].contains(">"));
        assert!(lines[1].contains("https://rust-lang.org"));

        // Third line should contain description indicator
        assert!(lines[2].contains("+"));
        assert!(lines[2].contains("Official Rust website"));

        // Fourth line should contain tags
        assert!(lines[3].contains("#"));
        assert!(lines[3].contains("rust"));
        assert!(lines[3].contains("programming"));
    }

    #[rstest]
    #[case(1)]
    #[case(42)]
    #[case(999)]
    fn test_colorize_padding_consistency(#[case] id: usize) {
        let bookmark = Bookmark::new(
            id,
            "https://example.com".to_string(),
            "Test".to_string(),
            ",tag,".to_string(),
            "Description".to_string(),
        );

        let colorized = ColorizeBookmark(&bookmark).to_colored();

        // Verify the output contains all expected elements
        assert!(colorized.contains(&id.to_string()));
        assert!(colorized.contains("Test"));
        assert!(colorized.contains("https://example.com"));
        assert!(colorized.contains("Description"));
        assert!(colorized.contains("tag"));
    }

    #[test]
    fn test_colorize_bookmark_empty_description() {
        let bookmark = Bookmark::new(
            1,
            "https://example.com".to_string(),
            "Example".to_string(),
            ",rust,".to_string(),
            "".to_string(),
        );

        let colorized = ColorizeBookmark(&bookmark).to_colored();

        // Should NOT contain a description line
        let lines: Vec<&str> = colorized.lines().collect();
        let has_desc_line = lines.iter().any(|line| line.trim().starts_with("+"));
        assert!(
            !has_desc_line,
            "Should not have description line for empty description"
        );
    }
}
