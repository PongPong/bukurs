use crate::utils;
use strs_tools::string;

/// Parse comma-separated tags, filtering empty ones
/// Note: strs_tools could be used for SIMD, but standard split is efficient for small tag strings
pub fn parse_tags<S: AsRef<str>>(tags_str: S) -> Vec<String> {
    string::split()
        .src(tags_str.as_ref())
        .preserving_delimeters(false)
        .delimeter(",")
        .perform()
        .map(String::from) // move Cow inside Split to String
        .map(|s| String::from(utils::trim_both_simd(&s)))
        .filter(|s| !s.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("", vec![])]
    #[case(",", vec![])]
    #[case(",,", vec![])]
    #[case(",,,", vec![])]
    #[case("rust", vec!["rust"])]
    #[case("rust,testing", vec!["rust", "testing"])]
    #[case(",rust,", vec!["rust"])]
    #[case(",rust,testing,", vec!["rust", "testing"])]
    #[case("rust, testing, programming", vec!["rust", "testing", "programming"])]
    #[case("  rust  ,  testing  ", vec!["rust", "testing"])]
    #[case("rust,,testing", vec!["rust", "testing"])]
    fn test_parse_tags(#[case] input: &str, #[case] expected: Vec<&str>) {
        let result = parse_tags(input);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_parse_tags_preserves_order() {
        let result = parse_tags(",z,a,m,b,");
        assert_eq!(result, vec!["z", "a", "m", "b"]);
    }

    #[test]
    fn test_parse_tags_handles_unicode() {
        let result = parse_tags(",rust,测试,программирование,");
        assert_eq!(result, vec!["rust", "测试", "программирование"]);
    }
}
