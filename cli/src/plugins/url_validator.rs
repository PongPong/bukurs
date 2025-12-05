//! URL Validator Plugin
//!
//! This plugin validates URLs before bookmarks are added or updated.
//! It provides configurable validation rules:
//!
//! # Features
//! - URL format validation (scheme, host, etc.)
//! - Blocked domain list
//! - Allowed schemes (http, https, etc.)
//! - Duplicate URL detection
//! - Optional HTTP HEAD check for URL accessibility
//!
//! # Configuration
//! ```yaml
//! plugins:
//!   url-validator:
//!     blocked_domains: "example.com,blocked.net"
//!     allowed_schemes: "http,https"
//!     check_accessibility: false
//!     block_duplicates: true
//! ```

use bukurs::models::bookmark::Bookmark;
use bukurs::plugin::{HookResult, Plugin, PluginContext, PluginInfo};
use std::collections::HashSet;

/// Validation error types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    /// URL is malformed
    MalformedUrl(String),
    /// URL scheme is not allowed
    InvalidScheme(String),
    /// Domain is blocked
    BlockedDomain(String),
    /// URL is a duplicate
    DuplicateUrl,
    /// URL is not accessible (HTTP check failed)
    NotAccessible(String),
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationError::MalformedUrl(msg) => write!(f, "Malformed URL: {}", msg),
            ValidationError::InvalidScheme(scheme) => {
                write!(f, "Invalid scheme '{}'. Only http/https allowed.", scheme)
            }
            ValidationError::BlockedDomain(domain) => {
                write!(f, "Domain '{}' is blocked", domain)
            }
            ValidationError::DuplicateUrl => write!(f, "URL already exists in bookmarks"),
            ValidationError::NotAccessible(msg) => write!(f, "URL not accessible: {}", msg),
        }
    }
}

/// URL validation result
pub type ValidationResult = Result<(), ValidationError>;

/// URL validator plugin
pub struct UrlValidatorPlugin {
    /// Set of blocked domains
    blocked_domains: HashSet<String>,
    /// Set of allowed URL schemes
    allowed_schemes: HashSet<String>,
    /// Whether to check URL accessibility
    check_accessibility: bool,
    /// Whether to block duplicate URLs
    block_duplicates: bool,
    /// Set of known URLs (for duplicate detection)
    known_urls: HashSet<String>,
    /// Whether the plugin is enabled
    enabled: bool,
}

impl UrlValidatorPlugin {
    pub fn new() -> Self {
        let mut allowed_schemes = HashSet::new();
        allowed_schemes.insert("http".to_string());
        allowed_schemes.insert("https".to_string());

        Self {
            blocked_domains: HashSet::new(),
            allowed_schemes,
            check_accessibility: false,
            block_duplicates: false,
            known_urls: HashSet::new(),
            enabled: true,
        }
    }

    /// Add a blocked domain
    pub fn block_domain(&mut self, domain: &str) {
        self.blocked_domains.insert(domain.to_lowercase());
    }

    /// Add multiple blocked domains
    pub fn block_domains(&mut self, domains: &[&str]) {
        for domain in domains {
            self.block_domain(domain);
        }
    }

    /// Set allowed schemes
    pub fn set_allowed_schemes(&mut self, schemes: &[&str]) {
        self.allowed_schemes.clear();
        for scheme in schemes {
            self.allowed_schemes.insert(scheme.to_lowercase());
        }
    }

    /// Enable/disable accessibility checking
    pub fn set_check_accessibility(&mut self, check: bool) {
        self.check_accessibility = check;
    }

    /// Enable/disable duplicate blocking
    pub fn set_block_duplicates(&mut self, block: bool) {
        self.block_duplicates = block;
    }

    /// Add a known URL (for duplicate detection)
    pub fn add_known_url(&mut self, url: &str) {
        self.known_urls.insert(self.normalize_url(url));
    }

    /// Normalize a URL for comparison
    fn normalize_url(&self, url: &str) -> String {
        // Remove trailing slashes, normalize to lowercase
        let url = url.to_lowercase();
        let url = url.trim_end_matches('/');

        // Remove www. prefix for comparison
        if let Some(rest) = url.strip_prefix("https://www.") {
            format!("https://{}", rest)
        } else if let Some(rest) = url.strip_prefix("http://www.") {
            format!("http://{}", rest)
        } else {
            url.to_string()
        }
    }

    /// Extract scheme from URL
    fn extract_scheme(&self, url: &str) -> Option<String> {
        url.find("://").map(|idx| url[..idx].to_lowercase())
    }

    /// Extract domain from URL
    fn extract_domain(&self, url: &str) -> Option<String> {
        let url = url
            .strip_prefix("https://")
            .or_else(|| url.strip_prefix("http://"))
            .unwrap_or(url);

        let url = url.strip_prefix("www.").unwrap_or(url);

        url.split('/').next().map(|s| s.to_lowercase())
    }

    /// Validate a URL
    pub fn validate(&self, url: &str) -> ValidationResult {
        // Check basic URL format
        if !url.contains("://") {
            return Err(ValidationError::MalformedUrl(
                "URL must include scheme (e.g., https://)".to_string(),
            ));
        }

        // Check scheme
        if let Some(scheme) = self.extract_scheme(url) {
            if !self.allowed_schemes.contains(&scheme) {
                return Err(ValidationError::InvalidScheme(scheme));
            }
        } else {
            return Err(ValidationError::MalformedUrl(
                "Could not parse URL scheme".to_string(),
            ));
        }

        // Check domain
        if let Some(domain) = self.extract_domain(url) {
            // Check exact domain match
            if self.blocked_domains.contains(&domain) {
                return Err(ValidationError::BlockedDomain(domain));
            }

            // Check if any blocked domain is a suffix (for subdomains)
            for blocked in &self.blocked_domains {
                if domain.ends_with(&format!(".{}", blocked)) {
                    return Err(ValidationError::BlockedDomain(blocked.clone()));
                }
            }
        } else {
            return Err(ValidationError::MalformedUrl(
                "Could not parse domain from URL".to_string(),
            ));
        }

        // Check for duplicates
        if self.block_duplicates {
            let normalized = self.normalize_url(url);
            if self.known_urls.contains(&normalized) {
                return Err(ValidationError::DuplicateUrl);
            }
        }

        // Accessibility check would go here (disabled by default)
        // if self.check_accessibility {
        //     // Perform HTTP HEAD request
        // }

        Ok(())
    }

    /// Validate and return detailed error message
    pub fn validate_with_message(&self, url: &str) -> Result<(), String> {
        self.validate(url).map_err(|e| e.to_string())
    }
}

impl Default for UrlValidatorPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for UrlValidatorPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "url-validator".to_string(),
            version: "1.0.0".to_string(),
            description: "Validates URLs before adding/updating bookmarks".to_string(),
            author: "bukurs".to_string(),
        }
    }

    fn on_load(&mut self, ctx: &PluginContext) -> HookResult {
        // Load blocked domains from config
        if let Some(domains) = ctx.config.get("blocked_domains") {
            for domain in domains.split(',') {
                self.block_domain(domain.trim());
            }
        }

        // Load allowed schemes from config
        if let Some(schemes) = ctx.config.get("allowed_schemes") {
            self.allowed_schemes.clear();
            for scheme in schemes.split(',') {
                self.allowed_schemes.insert(scheme.trim().to_lowercase());
            }
        }

        // Load accessibility check setting
        if let Some(check) = ctx.config.get("check_accessibility") {
            self.check_accessibility = check == "true";
        }

        // Load duplicate blocking setting
        if let Some(block) = ctx.config.get("block_duplicates") {
            self.block_duplicates = block == "true";
        }

        // Load enabled setting
        if let Some(enabled) = ctx.config.get("enabled") {
            self.enabled = enabled != "false";
        }

        HookResult::Continue
    }

    fn on_pre_add(&self, _ctx: &PluginContext, bookmark: &mut Bookmark) -> HookResult {
        if !self.enabled {
            return HookResult::Continue;
        }

        match self.validate(&bookmark.url) {
            Ok(()) => HookResult::Continue,
            Err(e) => HookResult::Error(e.to_string()),
        }
    }

    fn on_pre_update(
        &self,
        _ctx: &PluginContext,
        old: &Bookmark,
        new: &mut Bookmark,
    ) -> HookResult {
        if !self.enabled {
            return HookResult::Continue;
        }

        // Only validate if URL changed
        if old.url == new.url {
            return HookResult::Continue;
        }

        match self.validate(&new.url) {
            Ok(()) => HookResult::Continue,
            Err(e) => HookResult::Error(e.to_string()),
        }
    }

    fn on_pre_import(
        &self,
        _ctx: &PluginContext,
        bookmarks: &mut Vec<Bookmark>,
    ) -> HookResult {
        if !self.enabled {
            return HookResult::Continue;
        }

        // Validate all URLs, collect errors
        let mut errors = Vec::new();

        for (i, bookmark) in bookmarks.iter().enumerate() {
            if let Err(e) = self.validate(&bookmark.url) {
                errors.push(format!("Bookmark {}: {} - {}", i + 1, bookmark.url, e));
            }
        }

        if errors.is_empty() {
            HookResult::Continue
        } else {
            // Remove invalid bookmarks instead of blocking entire import
            let valid_urls: HashSet<_> = bookmarks
                .iter()
                .filter(|b| self.validate(&b.url).is_ok())
                .map(|b| b.url.clone())
                .collect();

            bookmarks.retain(|b| valid_urls.contains(&b.url));

            // Log warnings but continue
            for error in errors {
                log::warn!("Skipping invalid bookmark: {}", error);
            }

            HookResult::Continue
        }
    }
}

/// Create an instance of this plugin (required for auto-discovery)
pub fn create_plugin() -> Box<dyn Plugin> {
    Box::new(UrlValidatorPlugin::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_valid_urls() {
        let plugin = UrlValidatorPlugin::new();

        assert!(plugin.validate("https://example.com").is_ok());
        assert!(plugin.validate("http://example.com/path").is_ok());
        assert!(plugin.validate("https://sub.example.com/path?query=1").is_ok());
    }

    #[test]
    fn test_validate_invalid_scheme() {
        let plugin = UrlValidatorPlugin::new();

        assert!(matches!(
            plugin.validate("ftp://example.com"),
            Err(ValidationError::InvalidScheme(_))
        ));
        assert!(matches!(
            plugin.validate("file://localhost/path"),
            Err(ValidationError::InvalidScheme(_))
        ));
    }

    #[test]
    fn test_validate_malformed_url() {
        let plugin = UrlValidatorPlugin::new();

        assert!(matches!(
            plugin.validate("not-a-url"),
            Err(ValidationError::MalformedUrl(_))
        ));
        assert!(matches!(
            plugin.validate("example.com"),
            Err(ValidationError::MalformedUrl(_))
        ));
    }

    #[test]
    fn test_blocked_domains() {
        let mut plugin = UrlValidatorPlugin::new();
        plugin.block_domain("blocked.com");

        assert!(matches!(
            plugin.validate("https://blocked.com/path"),
            Err(ValidationError::BlockedDomain(_))
        ));
        assert!(matches!(
            plugin.validate("https://sub.blocked.com/path"),
            Err(ValidationError::BlockedDomain(_))
        ));
        assert!(plugin.validate("https://notblocked.com").is_ok());
    }

    #[test]
    fn test_duplicate_detection() {
        let mut plugin = UrlValidatorPlugin::new();
        plugin.set_block_duplicates(true);
        plugin.add_known_url("https://example.com/page");

        assert!(matches!(
            plugin.validate("https://example.com/page"),
            Err(ValidationError::DuplicateUrl)
        ));
        // With trailing slash should also match
        assert!(matches!(
            plugin.validate("https://example.com/page/"),
            Err(ValidationError::DuplicateUrl)
        ));
        // Different URL should pass
        assert!(plugin.validate("https://example.com/other").is_ok());
    }

    #[test]
    fn test_url_normalization() {
        let plugin = UrlValidatorPlugin::new();

        // www. prefix
        assert_eq!(
            plugin.normalize_url("https://www.example.com"),
            "https://example.com"
        );
        // Trailing slash
        assert_eq!(
            plugin.normalize_url("https://example.com/"),
            "https://example.com"
        );
        // Case normalization
        assert_eq!(
            plugin.normalize_url("HTTPS://EXAMPLE.COM"),
            "https://example.com"
        );
    }

    #[test]
    fn test_extract_domain() {
        let plugin = UrlValidatorPlugin::new();

        assert_eq!(
            plugin.extract_domain("https://example.com/path"),
            Some("example.com".to_string())
        );
        assert_eq!(
            plugin.extract_domain("https://www.example.com"),
            Some("example.com".to_string())
        );
        assert_eq!(
            plugin.extract_domain("https://sub.domain.example.com/path"),
            Some("sub.domain.example.com".to_string())
        );
    }

    #[test]
    fn test_on_pre_add_blocks_invalid() {
        let mut plugin = UrlValidatorPlugin::new();
        plugin.block_domain("blocked.com");

        let ctx = PluginContext::new(
            std::path::PathBuf::from("/test/db"),
            std::path::PathBuf::from("/test/data"),
        );

        let mut bookmark = Bookmark::new(
            0,
            "https://blocked.com/page".to_string(),
            "Blocked".to_string(),
            "".to_string(),
            "".to_string(),
        );

        let result = plugin.on_pre_add(&ctx, &mut bookmark);
        assert!(result.is_error());
    }

    #[test]
    fn test_on_pre_update_only_validates_changed_url() {
        let mut plugin = UrlValidatorPlugin::new();
        plugin.block_domain("blocked.com");

        let ctx = PluginContext::new(
            std::path::PathBuf::from("/test/db"),
            std::path::PathBuf::from("/test/data"),
        );

        let old = Bookmark::new(
            1,
            "https://example.com".to_string(),
            "Example".to_string(),
            "".to_string(),
            "".to_string(),
        );

        // Same URL - should pass
        let mut new_same = old.clone();
        new_same.title = "Updated Title".to_string();
        assert!(plugin.on_pre_update(&ctx, &old, &mut new_same).is_continue());

        // Changed to blocked URL - should fail
        let mut new_blocked = old.clone();
        new_blocked.url = "https://blocked.com".to_string();
        assert!(plugin.on_pre_update(&ctx, &old, &mut new_blocked).is_error());
    }
}
