use reqwest::blocking::Client;
use std::error::Error;
use tl::ParserOptions;

#[derive(Debug, PartialEq)]
pub struct FetchResult {
    pub url: String,
    pub title: String,
    pub desc: String,
    pub keywords: String,
}

const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
    AppleWebKit/605.1.15 (KHTML, like Gecko) \
    Version/18.5 Safari/605.1.15";

pub fn fetch_data(url: &str, user_agent: Option<&str>) -> Result<FetchResult, Box<dyn Error>> {
    let ua = user_agent.unwrap_or(USER_AGENT);
    let client = Client::builder().user_agent(ua).build()?;
    let resp = client.get(url).send()?;

    // Check HTTP status code
    let status = resp.status();
    if !status.is_success() {
        // Provide helpful error messages based on status code
        let error_msg = match status.as_u16() {
            403 => {
                "HTTP 403 Forbidden - This is often caused by user-agent blocking.\n\
                 Try customizing the user-agent in ~/.config/bukurs/config.yml"
            }
            401 => {
                "HTTP 401 Unauthorized - The website requires authentication or is blocking your request.\n\
                 This might be due to user-agent or other access restrictions."
            }
            404 => "HTTP 404 Not Found - The URL does not exist",
            429 => "HTTP 429 Too Many Requests - You are being rate limited",
            500..=599 => "HTTP 5xx Server Error - The website is experiencing issues",
            _ => "HTTP request failed with non-success status",
        };
        return Err(format!("{} (Status: {})", error_msg, status).into());
    }

    let final_url = resp.url().to_string();
    let body = resp.text()?;

    let mut result = parse_html(&body)?;
    result.url = final_url;
    Ok(result)
}

/// Parse HTML content and extract metadata
pub fn parse_html(html: &str) -> Result<FetchResult, Box<dyn Error>> {
    let dom = tl::parse(html, ParserOptions::default())?;
    let parser = dom.parser();

    // Extract title
    let title = dom
        .query_selector("title")
        .and_then(|mut iter| iter.next())
        .and_then(|handle| handle.get(parser))
        .map(|node| node.inner_text(parser).to_string())
        .unwrap_or_default();

    // Extract meta description
    let desc = extract_meta_content(&dom, parser, "description");

    // Extract meta keywords
    let keywords = extract_meta_content(&dom, parser, "keywords");

    Ok(FetchResult {
        url: String::new(), // Will be set by fetch_data
        title,
        desc,
        keywords,
    })
}

/// Helper function to extract content from meta tags
fn extract_meta_content(dom: &tl::VDom, parser: &tl::Parser, name: &str) -> String {
    dom.query_selector(&format!("meta[name='{}']", name))
        .and_then(|mut iter| iter.next())
        .and_then(|handle| handle.get(parser))
        .and_then(|node| {
            if let Some(tag) = node.as_tag() {
                tag.attributes()
                    .get("content")
                    .flatten()
                    .map(|v| v.as_utf8_str().to_string())
            } else {
                None
            }
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(
        r#"<!DOCTYPE html>
        <html><head>
            <title>Test Page</title>
            <meta name="description" content="This is a test description">
            <meta name="keywords" content="rust,testing,html">
        </head><body></body></html>"#,
        "Test Page",
        "This is a test description",
        "rust,testing,html"
    )]
    #[case(
        r#"<!DOCTYPE html>
        <html><head>
            <meta name="description" content="No title here">
        </head><body></body></html>"#,
        "",
        "No title here",
        ""
    )]
    #[case(
        r#"<!DOCTYPE html>
        <html><head>
            <title>Only Title</title>
        </head><body></body></html>"#,
        "Only Title",
        "",
        ""
    )]
    #[case(
        r#"<!DOCTYPE html>
        <html><head>
            <title>Test</title>
            <meta name="description" content="">
            <meta name="keywords" content="">
        </head><body></body></html>"#,
        "Test",
        "",
        ""
    )]
    #[case("", "", "", "")]
    fn test_parse_html_basic_cases(
        #[case] html: &str,
        #[case] expected_title: &str,
        #[case] expected_desc: &str,
        #[case] expected_keywords: &str,
    ) {
        let result = parse_html(html).unwrap();
        assert_eq!(result.title, expected_title);
        assert_eq!(result.desc, expected_desc);
        assert_eq!(result.keywords, expected_keywords);
    }

    #[test]
    fn test_parse_html_with_special_characters() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head>
                <title>Test & Title "with" <special> 'chars'</title>
                <meta name="description" content="Description with & < > " characters">
            </head>
            <body></body>
            </html>
        "#;

        let result = parse_html(html).unwrap();
        assert!(result.title.contains("Test"));
        assert!(result.desc.contains("Description"));
    }

    #[test]
    fn test_parse_html_multiple_meta_tags_same_name() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head>
                <title>Test</title>
                <meta name="description" content="First description">
                <meta name="description" content="Second description">
            </head>
            <body></body>
            </html>
        "#;

        let result = parse_html(html).unwrap();
        // Should get the first one
        assert_eq!(result.desc, "First description");
    }

    #[test]
    fn test_parse_html_whitespace_in_title() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head>
                <title>
                    Test Title
                    With Whitespace
                </title>
            </head>
            <body></body>
            </html>
        "#;

        let result = parse_html(html).unwrap();
        assert!(result.title.contains("Test Title"));
        assert!(result.title.contains("With Whitespace"));
    }

    #[rstest]
    #[case("<html><title>Test<meta name=\"description\" content=\"Test desc\"></html>")]
    #[case("<html><head><title>Unclosed tag")]
    #[case("Not even HTML at all!")]
    fn test_parse_html_malformed(#[case] html: &str) {
        // Should still parse without error
        let result = parse_html(html);
        assert!(result.is_ok(), "Failed to parse: {:?}", result.err());
    }

    #[test]
    fn test_extract_meta_content_case_sensitive() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head>
                <meta name="Description" content="Should not match">
                <meta name="description" content="Should match">
            </head>
            </html>
        "#;

        let result = parse_html(html).unwrap();
        assert_eq!(result.desc, "Should match");
    }

    #[rstest]
    #[case("Test Simple", "Test Simple")]
    #[case("  Spaces  ", "  Spaces  ")]
    #[case("Line1\nLine2", "Line1\nLine2")]
    #[case("", "")]
    fn test_parse_html_title_variations(#[case] title_content: &str, #[case] expected: &str) {
        let html = format!(
            r#"<!DOCTYPE html><html><head><title>{}</title></head></html>"#,
            title_content
        );

        let result = parse_html(&html).unwrap();
        assert_eq!(result.title, expected);
    }
}
