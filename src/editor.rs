use crate::models::bookmark::Bookmark;
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

fn parse_edited_bookmark(content: &str, original_id: usize) -> Result<Bookmark> {
    let mut url = String::new();
    let mut title = String::new();
    let mut tags = String::new();
    let mut description = String::new();
    let mut in_description = false;

    for line in content.lines() {
        let trimmed = line.trim();

        // Skip comments always
        if trimmed.starts_with('#') {
            continue;
        }

        // Skip empty lines only when NOT in description
        if !in_description && trimmed.is_empty() {
            continue;
        }

        if in_description {
            // Accumulate description lines (check original line for indentation)
            if line.starts_with("  ") || line.is_empty() {
                if !description.is_empty() {
                    description.push('\n');
                }
                description.push_str(line.trim_start());
            } else if !trimmed.is_empty() {
                // Non-indented, non-empty line means end of description
                in_description = false;
            }
        }

        if !in_description {
            if let Some(value) = trimmed.strip_prefix("url:") {
                url = value.trim().to_string();
            } else if let Some(value) = trimmed.strip_prefix("title:") {
                title = value.trim().to_string();
            } else if let Some(value) = trimmed.strip_prefix("tags:") {
                tags = value.trim().to_string();
            } else if trimmed.starts_with("description:") {
                in_description = true;
                if let Some(value) = trimmed.strip_prefix("description:") {
                    let inline_desc = value.trim();
                    if !inline_desc.is_empty() && inline_desc != "|" {
                        description = inline_desc.to_string();
                        in_description = false;
                    }
                }
            }
        }
    }

    // Validate
    if url.is_empty() {
        return Err(EditorError::EmptyUrl);
    }

    Ok(Bookmark::new(
        original_id,
        url,
        title,
        tags,
        description.trim().to_string(),
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
