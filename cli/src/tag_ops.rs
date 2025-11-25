/// Tag operation types
#[derive(Debug, PartialEq, Clone)]
pub enum TagOp {
    /// Add a tag (prefix: +)
    Add(String),
    /// Remove a tag (prefix: -)
    Remove(String),
    /// Replace a tag (format: ~old:new)
    Replace { old: String, new: String },
}

/// Parse tag operations from command line arguments
///
/// Syntax:
/// - `+tag` - Add tag
/// - `-tag` - Remove tag
/// - `~old:new` - Replace old tag with new tag
/// - `tag` - Add tag (no prefix = add)
pub fn parse_tag_operations(tags: &[String]) -> Vec<TagOp> {
    let mut operations = Vec::new();
    let mut invalid_tags = Vec::new();
    let mut invalid_syntax = Vec::new();

    for tag in tags {
        if tag.is_empty() {
            continue;
        }

        if let Some(tag_name) = tag.strip_prefix('+') {
            if tag_name.contains(' ') {
                invalid_tags.push(format!("+{}", tag_name));
            } else {
                operations.push(TagOp::Add(tag_name.to_string()));
            }
        } else if let Some(tag_name) = tag.strip_prefix('-') {
            if tag_name.contains(' ') {
                invalid_tags.push(format!("-{}", tag_name));
            } else {
                operations.push(TagOp::Remove(tag_name.to_string()));
            }
        } else if let Some(replace_spec) = tag.strip_prefix('~') {
            // Format: ~old:new
            if let Some((old, new)) = replace_spec.split_once(':') {
                if old.contains(' ') || new.contains(' ') {
                    invalid_tags.push(format!("~{}", replace_spec));
                } else {
                    operations.push(TagOp::Replace {
                        old: old.to_string(),
                        new: new.to_string(),
                    });
                }
            } else {
                invalid_syntax.push(tag.clone());
            }
        } else {
            // No prefix = add
            if tag.contains(' ') {
                invalid_tags.push(tag.clone());
            } else {
                operations.push(TagOp::Add(tag.to_string()));
            }
        }
    }

    // Print consolidated warnings
    if !invalid_tags.is_empty() {
        eprintln!(
            "Warning: The following tags contain spaces and were ignored: {}",
            invalid_tags.join(", ")
        );
    }

    if !invalid_syntax.is_empty() {
        eprintln!(
            "Warning: Invalid replace syntax (expected '~old:new'): {}",
            invalid_syntax.join(", ")
        );
    }

    operations
}

/// Apply tag operations to existing tags
pub fn apply_tag_operations(existing_tags: &str, operations: &[TagOp]) -> String {
    let mut tags: Vec<String> = if existing_tags.is_empty() {
        Vec::new()
    } else {
        existing_tags
            .split(',')
            .map(|s| s.trim().to_string())
            .collect()
    };

    for op in operations {
        match op {
            TagOp::Add(tag) => {
                // Only add if not already present
                if !tags.contains(tag) {
                    tags.push(tag.clone());
                }
            }
            TagOp::Remove(tag) => {
                tags.retain(|t| t != tag);
            }
            TagOp::Replace { old, new } => {
                if let Some(pos) = tags.iter().position(|t| t == old) {
                    tags[pos] = new.clone();
                }
            }
        }
    }

    tags.join(",")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(vec!["+foo"], vec![TagOp::Add("foo".to_string())])]
    #[case(vec!["-bar"], vec![TagOp::Remove("bar".to_string())])]
    #[case(vec!["~old:new"], vec![TagOp::Replace { old: "old".to_string(), new: "new".to_string() }])]
    #[case(vec!["simple"], vec![TagOp::Add("simple".to_string())])]
    #[case(
        vec!["+foo", "-bar", "~old:new"],
        vec![
            TagOp::Add("foo".to_string()),
            TagOp::Remove("bar".to_string()),
            TagOp::Replace { old: "old".to_string(), new: "new".to_string() }
        ]
    )]
    fn test_parse_tag_operations(#[case] input: Vec<&str>, #[case] expected: Vec<TagOp>) {
        let input_strings: Vec<String> = input.iter().map(|s| s.to_string()).collect();
        let result = parse_tag_operations(&input_strings);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_parse_invalid_replace() {
        let input = vec!["~nocolon".to_string()];
        let result = parse_tag_operations(&input);
        assert_eq!(result, vec![]); // Should skip invalid
    }

    #[test]
    fn test_parse_tags_with_spaces() {
        let input = vec![
            "tag with space".to_string(),
            "+add space".to_string(),
            "-remove space".to_string(),
            "~old space:new".to_string(),
            "~old:new space".to_string(),
            "valid".to_string(),
        ];
        let result = parse_tag_operations(&input);
        assert_eq!(result, vec![TagOp::Add("valid".to_string())]);
    }

    #[test]
    fn test_parse_empty_tags() {
        let input: Vec<String> = vec![];
        let result = parse_tag_operations(&input);
        assert_eq!(result, vec![]);
    }

    #[rstest]
    #[case("", vec![TagOp::Add("new".to_string())], "new")]
    #[case("existing", vec![TagOp::Add("new".to_string())], "existing,new")]
    #[case("foo,bar", vec![TagOp::Remove("bar".to_string())], "foo")]
    #[case("foo,bar,baz", vec![TagOp::Replace { old: "bar".to_string(), new: "qux".to_string() }], "foo,qux,baz")]
    #[case("foo", vec![TagOp::Add("foo".to_string())], "foo")] // Duplicate add should not create duplicate
    fn test_apply_tag_operations(
        #[case] existing: &str,
        #[case] ops: Vec<TagOp>,
        #[case] expected: &str,
    ) {
        let result = apply_tag_operations(existing, &ops);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_apply_combined_operations() {
        let existing = "rust,tech,old";
        let ops = vec![
            TagOp::Add("new".to_string()),
            TagOp::Remove("tech".to_string()),
            TagOp::Replace {
                old: "old".to_string(),
                new: "fresh".to_string(),
            },
        ];
        let result = apply_tag_operations(existing, &ops);
        assert_eq!(result, "rust,fresh,new");
    }

    #[test]
    fn test_remove_nonexistent_tag() {
        let existing = "foo,bar";
        let ops = vec![TagOp::Remove("baz".to_string())];
        let result = apply_tag_operations(existing, &ops);
        assert_eq!(result, "foo,bar");
    }

    #[test]
    fn test_replace_nonexistent_tag() {
        let existing = "foo,bar";
        let ops = vec![TagOp::Replace {
            old: "baz".to_string(),
            new: "qux".to_string(),
        }];
        let result = apply_tag_operations(existing, &ops);
        assert_eq!(result, "foo,bar"); // No change
    }
}
