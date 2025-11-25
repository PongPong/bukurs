use bukurs::utils;
use std::collections::HashSet;

/// Tag operation types
#[derive(Debug, PartialEq, Clone)]
pub enum TagOp<'a> {
    Add(&'a str),
    Remove(&'a str),
    Replace { old: &'a str, new: &'a str },
}

/// Parse tag operations from command line arguments
///
/// Syntax:
/// - `+tag` - Add tag
/// - `-tag` - Remove tag
/// - `~old:new` - Replace old tag with new tag
/// - `tag` - Add tag (no prefix = add)
pub fn parse_tag_operations<'a>(tags: &'a [String]) -> Vec<TagOp<'a>> {
    let mut operations = Vec::new();
    let mut invalid_tags = Vec::new();
    let mut invalid_syntax = Vec::new();

    for tag in tags {
        if tag.is_empty() {
            continue;
        }

        let bytes = tag.as_bytes();

        let (op, rest) = match bytes {
            [b'+', ..] => ('+', &tag[1..]),
            [b'-', ..] => ('-', &tag[1..]),
            [b'~', ..] => ('~', &tag[1..]),
            _ => ('+', tag.as_str()), // default: add
        };

        // SIMD-accelerated space check
        let has_space = utils::has_spaces(rest);

        if has_space {
            match op {
                '+' => invalid_tags.push(format!("+{}", rest)),
                '-' => invalid_tags.push(format!("-{}", rest)),
                '~' => invalid_tags.push(format!("~{}", rest)),
                _ => invalid_tags.push(rest.to_string()),
            }
            continue;
        }

        match op {
            '+' => operations.push(TagOp::Add(rest)),

            '-' => operations.push(TagOp::Remove(rest)),

            '~' => {
                // SIMD-accelerated ':' search
                if let Some((old, new)) = utils::split_colon_no_space(rest) {
                    operations.push(TagOp::Replace { old, new });
                } else if utils::has_char(b':', rest) {
                    invalid_tags.push(format!("~{}", rest));
                } else {
                    invalid_syntax.push(tag.clone());
                }
            }

            _ => unreachable!(),
        }
    }

    // consolidated warnings
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
pub fn apply_tag_operations<'a>(existing_tags: &'a str, operations: &[TagOp<'a>]) -> String {
    // Parse existing tags into a Vec for order + Set for fast lookup
    let mut vec: Vec<&'a str> = Vec::new();
    let mut set: HashSet<&'a str> = HashSet::new();

    if !existing_tags.is_empty() {
        for tag in existing_tags.split(',').map(utils::trim_both_simd) {
            if !tag.is_empty() && set.insert(tag) {
                vec.push(tag);
            }
        }
    }

    // Apply operations
    for op in operations {
        match *op {
            TagOp::Add(tag) => {
                if set.insert(tag) {
                    vec.push(tag);
                }
            }

            TagOp::Remove(tag) => {
                if set.remove(tag) {
                    vec.retain(|t| *t != tag);
                }
            }

            TagOp::Replace { old, new } => {
                if set.remove(old) {
                    set.insert(new);

                    if let Some(pos) = vec.iter().position(|t| *t == old) {
                        vec[pos] = new;
                    }
                }
            }
        }
    }

    vec.join(",")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(vec!["+foo"], vec![TagOp::Add("foo")])]
    #[case(vec!["-bar"], vec![TagOp::Remove("bar")])]
    #[case(vec!["~old:new"], vec![TagOp::Replace { old: "old", new: "new"}])]
    #[case(vec!["simple"], vec![TagOp::Add("simple")])]
    #[case(
        vec!["+foo", "-bar", "~old:new"],
        vec![
            TagOp::Add("foo"),
            TagOp::Remove("bar"),
            TagOp::Replace { old: "old", new: "new" }
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
        assert_eq!(result, vec![TagOp::Add("valid")]);
    }

    #[test]
    fn test_parse_empty_tags() {
        let input: Vec<String> = vec![];
        let result = parse_tag_operations(&input);
        assert_eq!(result, vec![]);
    }

    #[rstest]
    #[case("", vec![TagOp::Add("new")], "new")]
    #[case("existing", vec![TagOp::Add("new")], "existing,new")]
    #[case("foo,bar", vec![TagOp::Remove("bar")], "foo")]
    #[case("foo,bar,baz", vec![TagOp::Replace { old: "bar", new: "qux"}], "foo,qux,baz")]
    #[case("foo", vec![TagOp::Add("foo")], "foo")] // Duplicate add should not create duplicate
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
            TagOp::Add("new"),
            TagOp::Remove("tech"),
            TagOp::Replace {
                old: "old",
                new: "fresh",
            },
        ];
        let result = apply_tag_operations(existing, &ops);
        assert_eq!(result, "rust,fresh,new");
    }

    #[test]
    fn test_remove_nonexistent_tag() {
        let existing = "foo,bar";
        let ops = vec![TagOp::Remove("baz")];
        let result = apply_tag_operations(existing, &ops);
        assert_eq!(result, "foo,bar");
    }

    #[test]
    fn test_replace_nonexistent_tag() {
        let existing = "foo,bar";
        let ops = vec![TagOp::Replace {
            old: "baz",
            new: "qux",
        }];
        let result = apply_tag_operations(existing, &ops);
        assert_eq!(result, "foo,bar"); // No change
    }
}
