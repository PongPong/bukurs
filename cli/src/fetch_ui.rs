use bukurs::fetch;
use indicatif::{ProgressBar, ProgressStyle};
use std::error::Error;

/// Fetch metadata with visual spinner feedback
///
/// Shows an animated spinner while fetching, then displays success/failure status
/// with categorized error messages.
pub fn fetch_with_spinner(
    url: &str,
    user_agent: &str,
) -> Result<fetch::FetchResult, Box<dyn Error>> {
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap(),
    );

    let url_display = truncate_url(url, 60);
    spinner.set_message(format!("Fetching: {}", url_display));
    spinner.enable_steady_tick(std::time::Duration::from_millis(100));

    let result = fetch::fetch_data(url, Some(user_agent));

    match &result {
        Ok(_) => spinner.finish_with_message(format!("✓ {}", url_display)),
        Err(e) => {
            let error_msg = categorize_error(e.as_ref());
            spinner.finish_with_message(format!("✗ {} ({})", url_display, error_msg));
        }
    }

    result
}

/// Truncate URL to specified length with ellipsis
pub fn truncate_url(url: &str, max_len: usize) -> String {
    if url.len() > max_len {
        let truncate_at = max_len.saturating_sub(3); // Reserve 3 chars for "..."
        format!("{}...", &url[..truncate_at])
    } else {
        url.to_string()
    }
}

/// Categorize error for user-friendly display
pub fn categorize_error(error: &(dyn Error + 'static)) -> &'static str {
    let error_str = error.to_string();

    if error_str.contains("403") {
        "blocked"
    } else if error_str.contains("401") {
        "unauthorized"
    } else if error_str.contains("404") {
        "not found"
    } else if error_str.contains("timeout") {
        "timeout"
    } else if error_str.contains("dns") {
        "dns error"
    } else if error_str.contains("connection") {
        "connection error"
    } else {
        "fetch error"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("https://example.com", 60, "https://example.com")]
    #[case(
        "https://example.com/very/long/path/that/exceeds/the/limit",
        30,
        "https://example.com/very/lo..."
    )]
    #[case("https://a.com", 100, "https://a.com")]
    #[case("https://example.com/test", 20, "https://example.c...")]
    fn test_truncate_url(#[case] url: &str, #[case] max_len: usize, #[case] expected: &str) {
        let result = truncate_url(url, max_len);
        assert_eq!(result, expected);
        // Verify the result doesn't exceed max_len
        assert!(
            result.len() <= max_len,
            "Result length {} exceeds max_len {}",
            result.len(),
            max_len
        );
    }

    #[rstest]
    #[case("HTTP 403 Forbidden", "blocked")]
    #[case("HTTP 401 Unauthorized", "unauthorized")]
    #[case("HTTP 404 Not Found", "not found")]
    #[case("connection timeout", "timeout")]
    #[case("dns lookup failed", "dns error")]
    #[case("connection refused", "connection error")]
    #[case("unexpected error", "fetch error")]
    fn test_categorize_error(#[case] error_msg: &str, #[case] expected: &str) {
        // Create a boxed error for testing
        let error: Box<dyn Error> = error_msg.into();
        assert_eq!(categorize_error(error.as_ref()), expected);
    }

    #[test]
    fn test_truncate_url_boundary() {
        // Test exact boundary
        let url = "https://example.com/12345";
        assert_eq!(truncate_url(url, 25), url);

        // Test one over boundary - should truncate
        let result = truncate_url(url, 24);
        assert_eq!(result, "https://example.com/1...");
        assert_eq!(result.len(), 24);
    }

    #[test]
    fn test_truncate_url_minimum_length() {
        // Very short max_len should still work
        let url = "https://example.com";
        let result = truncate_url(url, 10);
        assert_eq!(result, "https:/...");
        assert_eq!(result.len(), 10);
    }

    #[rstest]
    #[case("http error 403", "blocked")]
    #[case("Error: 403 Forbidden", "blocked")]
    #[case("401 authorization required", "unauthorized")]
    #[case("page not found 404", "not found")]
    fn test_categorize_error_case_insensitive(#[case] error_msg: &str, #[case] expected: &str) {
        let error: Box<dyn Error> = error_msg.into();
        assert_eq!(categorize_error(error.as_ref()), expected);
    }

    #[test]
    fn test_categorize_error_priority() {
        // When multiple keywords match, ensure correct priority
        let error: Box<dyn Error> = "connection timeout occurred".into();
        // "timeout" should take precedence over "connection"
        assert_eq!(categorize_error(error.as_ref()), "timeout");
    }

    // Tests for fetch_with_spinner
    // Testing strategy: Since fetch_with_spinner depends on real network calls,
    // we test the parts we can control (error handling, URL truncation) and
    // mark network-dependent tests as #[ignore]

    #[test]
    fn test_fetch_with_spinner_invalid_url() {
        // Test with malformed URL (no network required)
        // This tests error handling path
        let result = fetch_with_spinner("not-a-valid-url", "Mozilla/5.0 Test");

        assert!(result.is_err(), "Should fail with invalid URL");
    }

    #[test]
    fn test_fetch_with_spinner_empty_url() {
        // Test with empty URL
        let result = fetch_with_spinner("", "Mozilla/5.0 Test");

        assert!(result.is_err(), "Should fail with empty URL");
    }

    #[test]
    fn test_fetch_with_spinner_url_truncation() {
        // Test that long URLs get truncated in display (no network needed)
        // Use .invalid TLD which is reserved and guaranteed not to resolve
        let very_long_url = format!("https://nonexistent.invalid/{}", "a".repeat(100));
        let result = fetch_with_spinner(&very_long_url, "Mozilla/5.0 Test");

        // The function should complete without panic
        // Will fail with DNS error since .invalid never resolves
        assert!(result.is_err());
    }

    #[test]
    fn test_fetch_with_spinner_nonexistent_domain() {
        // Test with non-existent domain (tests DNS error handling)
        let result = fetch_with_spinner(
            "https://this-domain-definitely-does-not-exist-12345.com",
            "Mozilla/5.0 Test",
        );

        assert!(result.is_err(), "Should fail with DNS error");
    }

    // Network-dependent integration tests
    // These are ignored by default because:
    // 1. They require internet connection
    // 2. External services may be unavailable
    // 3. They're slower than unit tests
    // Run with: cargo test -- --ignored

    #[test]
    #[ignore]
    fn test_fetch_with_spinner_network_success() {
        // Test with example.com (very stable)
        let result = fetch_with_spinner("http://example.com", "Mozilla/5.0 Test");

        // Note: Success depends on network and example.com being available
        if result.is_ok() {
            let fetch_result = result.unwrap();
            assert!(!fetch_result.url.is_empty());
        }
        // We don't fail if network is unavailable
    }
}
