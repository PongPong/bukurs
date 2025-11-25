use bukurs::models::bookmark::Bookmark;
use memchr::memchr2;
use std::env;
use std::fs;
use std::io::Write;
use std::process::Command;
use tempfile::NamedTempFile;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EditorError {
    #[error("Failed to create temporary file: {0}")]
    TempFileCreation(#[from] std::io::Error),

    #[error("Failed to launch editor '{0}': {1}")]
    EditorLaunch(String, std::io::Error),

    #[error("Editor exited with non-zero status")]
    EditorExitFailure,

    #[error("URL cannot be empty")]
    EmptyUrl,
}

pub type Result<T> = std::result::Result<T, EditorError>;

pub fn edit_bookmark(bookmark: &Bookmark) -> Result<Bookmark> {
    // Get editor from environment, default to vim
    let editor = env::var("EDITOR").unwrap_or_else(|_| "vim".to_string());

    // Create temporary file with bookmark data in YAML format
    let mut temp_file = NamedTempFile::new()?;

    // Write bookmark as YAML
    let yaml_content = format!(
        "# Edit bookmark (lines starting with # are comments)\n\
         # Save and exit to update, or exit without saving to cancel\n\
         \n\
         id: {}\n\
         url: {}\n\
         title: {}\n\
         tags: {}\n\
         description: |\n  {}\n",
        bookmark.id,
        bookmark.url,
        bookmark.title,
        bookmark.tags,
        bookmark.description.replace("\n", "\n  ")
    );

    temp_file.write_all(yaml_content.as_bytes())?;

    let temp_path = temp_file.path().to_owned();
    let temp_path_str = temp_path.to_string_lossy();

    // Open editor - use shell to support complex EDITOR commands
    // (e.g., "env NVIM_APPNAME=astronvim nvim")
    let status = build_editor_command(&editor, &temp_path_str)
        .status()
        .map_err(|e| EditorError::EditorLaunch(editor.clone(), e))?;

    if !status.success() {
        return Err(EditorError::EditorExitFailure);
    }

    // Read edited content
    let edited_content = fs::read_to_string(&temp_path)?;

    // Parse the edited YAML
    parse_edited_bookmark(&edited_content, bookmark.id)
}

/// Edit a new bookmark template to create a bookmark
pub fn edit_new_bookmark() -> Result<Bookmark> {
    // Get editor from environment, default to vim
    let editor = env::var("EDITOR").unwrap_or_else(|_| "vim".to_string());

    // Create temporary file with template
    let mut temp_file = NamedTempFile::new()?;

    // Write template as YAML
    let template_content = "\
# Create new bookmark (lines starting with # are comments)\n\
# Save and exit to create, or exit without saving to cancel\n\
# URL is required, other fields are optional\n\
\n\
url: \n\
title: \n\
tags: \n\
description: |\n\
  \n";

    temp_file.write_all(template_content.as_bytes())?;

    let temp_path = temp_file.path().to_owned();
    let temp_path_str = temp_path.to_string_lossy();

    // Open editor
    let status = build_editor_command(&editor, &temp_path_str)
        .status()
        .map_err(|e| EditorError::EditorLaunch(editor.clone(), e))?;

    if !status.success() {
        return Err(EditorError::EditorExitFailure);
    }

    // Read edited content
    let edited_content = fs::read_to_string(&temp_path)?;

    // Parse the edited YAML with ID 0 (will be assigned by database)
    parse_edited_bookmark(&edited_content, 0)
}

/// Build the command to launch the editor via shell
fn build_editor_command(editor: &str, file_path: &str) -> Command {
    if cfg!(target_os = "windows") {
        let mut cmd = Command::new("cmd");
        cmd.args(["/C", &format!("{} {}", editor, file_path)]);
        cmd
    } else {
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(format!("{} {}", editor, file_path));
        cmd
    }
}

#[inline]
fn trim_start_simd(s: &str) -> &str {
    let bytes = s.as_bytes();

    if bytes.is_empty() || (bytes[0] != b' ' && bytes[0] != b'\t') {
        return s;
    }

    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        if b != b' ' && b != b'\t' {
            return &s[i..];
        }
        i += 1;
    }

    ""
}

#[inline]
fn trim_end_simd(s: &str) -> &str {
    let bytes = s.as_bytes();

    if bytes.is_empty() {
        return s;
    }

    let mut end = bytes.len();
    while end > 0 {
        let b = bytes[end - 1];
        if b != b' ' && b != b'\t' {
            break;
        }
        end -= 1;
    }

    &s[..end]
}

#[inline]
fn trim_both_simd(s: &str) -> &str {
    trim_end_simd(trim_start_simd(s))
}

fn parse_edited_bookmark(content: &str, original_id: usize) -> Result<Bookmark> {
    let mut url: &str = "";
    let mut title: &str = "";
    let mut tags: &str = "";

    // Description can span multiple lines — must own final result
    let mut description_buf = String::new();
    let mut in_description = false;

    for line in content.lines() {
        let trimmed = trim_both_simd(line);

        // Skip comments
        if trimmed.starts_with('#') {
            continue;
        }

        // Skip empty lines outside description
        if !in_description && trimmed.is_empty() {
            continue;
        }

        if in_description {
            // Indented or empty → description continues
            if line.starts_with("  ") || line.is_empty() {
                if !description_buf.is_empty() {
                    description_buf.push('\n');
                }
                description_buf.push_str(trim_start_simd(line));
                continue;
            }

            // Non-indented → end description
            in_description = false;
        }

        // Byte-prefix matching (fast, predictable)
        let b = trimmed.as_bytes();
        if b.starts_with(b"url:") {
            url = trim_both_simd(&trimmed[4..]);
            continue;
        }
        if b.starts_with(b"title:") {
            title = trim_both_simd(&trimmed[6..]);
            continue;
        }
        if b.starts_with(b"tags:") {
            tags = trim_both_simd(&trimmed[5..]);
            continue;
        }
        if b.starts_with(b"description:") {
            let rest = trim_both_simd(&trimmed[12..]);

            // Inline description: description: something
            if !rest.is_empty() && rest != "|" {
                description_buf = rest.to_string();
                in_description = false;
            } else {
                in_description = true;
            }
        }
    }

    if url.is_empty() {
        return Err(EditorError::EmptyUrl);
    }

    Ok(Bookmark::new(
        original_id,
        url.to_string(),
        title.to_string(),
        tags.to_string(),
        description_buf.trim().to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(
        "url: https://example.com\ntitle: Test\ntags: ,test,\ndescription: A test",
        1,
        "https://example.com",
        "Test",
        ",test,",
        "A test"
    )]
    #[case(
        "# Comment\nurl: https://rust-lang.org\ntitle: Rust\ntags: ,programming,\ndescription: |\n  Rust programming\n  language",
        2,
        "https://rust-lang.org",
        "Rust",
        ",programming,",
        "Rust programming\nlanguage"
    )]
    #[case(
        "url: https://test.com\ntitle: Empty Tags\ntags: \ndescription: No tags",
        3,
        "https://test.com",
        "Empty Tags",
        "",
        "No tags"
    )]
    #[case(
        "url: https://minimal.com\ntitle:\ntags:\ndescription:",
        4,
        "https://minimal.com",
        "",
        "",
        ""
    )]
    fn test_parse_edited_bookmark_success(
        #[case] content: &str,
        #[case] id: usize,
        #[case] expected_url: &str,
        #[case] expected_title: &str,
        #[case] expected_tags: &str,
        #[case] expected_desc: &str,
    ) {
        let result = parse_edited_bookmark(content, id);
        assert!(result.is_ok(), "Parsing should succeed: {:?}", result.err());

        let bookmark = result.unwrap();
        assert_eq!(bookmark.id, id);
        assert_eq!(bookmark.url, expected_url);
        assert_eq!(bookmark.title, expected_title);
        assert_eq!(bookmark.tags, expected_tags);
        assert_eq!(bookmark.description, expected_desc);
    }

    #[rstest]
    #[case("title: Test\ntags: ,test,\ndescription: Missing URL")]
    #[case("# Only comments\n# url: not_a_url")]
    #[case("")]
    #[case("url: \ntitle: Empty URL")]
    fn test_parse_edited_bookmark_missing_url(#[case] content: &str) {
        let result = parse_edited_bookmark(content, 1);
        assert!(result.is_err(), "Should fail with missing URL");
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("URL cannot be empty"));
    }

    #[test]
    fn test_parse_multiline_description() {
        let content = "url: https://example.com
title: Test
tags: ,test,
description: |
  Line 1
  Line 2
  Line 3";

        let result = parse_edited_bookmark(content, 1).unwrap();
        assert_eq!(result.description, "Line 1\nLine 2\nLine 3");
    }

    #[test]
    fn test_parse_inline_description() {
        let content = "url: https://example.com
title: Test
tags: ,test,
description: Single line desc";

        let result = parse_edited_bookmark(content, 1).unwrap();
        assert_eq!(result.description, "Single line desc");
    }

    #[test]
    fn test_parse_with_comments() {
        let content = "# This is a comment\n\
                      url: https://example.com\n\
                      # Another comment\n\
                      title: Test\n\
                      tags: ,test,\n\
                      # Comment before description\n\
                      description: Test desc";

        let result = parse_edited_bookmark(content, 1).unwrap();
        assert_eq!(result.url, "https://example.com");
        assert_eq!(result.title, "Test");
    }

    #[test]
    fn test_parse_with_empty_lines() {
        let content = "url: https://example.com\n\
                      \n\
                      title: Test\n\
                      \n\
                      tags: ,test,\n\
                      \n\
                      description: Test desc";

        let result = parse_edited_bookmark(content, 1).unwrap();
        assert_eq!(result.url, "https://example.com");
        assert_eq!(result.title, "Test");
    }

    #[test]
    fn test_parse_preserves_id() {
        let content = "id: 999\nurl: https://example.com\ntitle: Test\ntags: \ndescription: ";

        let result = parse_edited_bookmark(content, 42).unwrap();
        assert_eq!(
            result.id, 42,
            "Should preserve original ID, not parse from content"
        );
    }

    #[rstest]
    #[case(
        "url: https://example.com  \ntitle:  Trimmed  \ntags: ,test,\n",
        "Trimmed"
    )]
    #[case("url:    https://example.com\ntitle:Test\ntags: ,test,\n", "Test")]
    fn test_parse_trims_whitespace(#[case] content: &str, #[case] expected_title: &str) {
        let result = parse_edited_bookmark(content, 1).unwrap();
        assert_eq!(result.title, expected_title);
    }

    #[test]
    fn test_build_editor_command_simple() {
        let cmd = build_editor_command("vim", "/tmp/test.txt");
        let program = cmd.get_program().to_string_lossy();

        if cfg!(target_os = "windows") {
            assert_eq!(program, "cmd");
        } else {
            assert_eq!(program, "sh");
        }
    }

    #[test]
    fn test_build_editor_command_complex() {
        let editor = "env NVIM_APPNAME=astronvim nvim";
        let cmd = build_editor_command(editor, "/tmp/test.txt");
        let program = cmd.get_program().to_string_lossy();

        if cfg!(target_os = "windows") {
            assert_eq!(program, "cmd");
        } else {
            assert_eq!(program, "sh");
        }
    }

    #[rstest]
    #[case("vim")]
    #[case("nvim")]
    #[case("code --wait")]
    #[case("env NVIM_APPNAME=astronvim nvim")]
    #[case("emacs -nw")]
    fn test_build_editor_command_various_editors(#[case] editor: &str) {
        let cmd = build_editor_command(editor, "/tmp/test.txt");

        // Just verify it builds without panicking
        let program = cmd.get_program();
        assert!(!program.is_empty());
    }

    #[test]
    fn test_parse_description_with_pipe() {
        let content = "url: https://example.com
title: Test
tags: ,test,
description: |
  First
  Second";

        let result = parse_edited_bookmark(content, 1).unwrap();
        assert_eq!(result.description, "First\nSecond");
    }

    #[test]
    fn test_parse_special_characters_in_fields() {
        let content = "url: https://example.com/path?query=value&foo=bar\n\
                      title: Test & Title <special>\n\
                      tags: ,tag-1,tag_2,tag.3,\n\
                      description: Special chars: !@#$%";

        let result = parse_edited_bookmark(content, 1).unwrap();
        assert_eq!(result.url, "https://example.com/path?query=value&foo=bar");
        assert_eq!(result.title, "Test & Title <special>");
        assert_eq!(result.tags, ",tag-1,tag_2,tag.3,");
        assert_eq!(result.description, "Special chars: !@#$%");
    }
}
