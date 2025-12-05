//! QR Code Plugin
//!
//! Generates QR codes for bookmark URLs:
//! - Outputs QR code to terminal using Unicode block characters
//! - Can be triggered via `bukurs qr <id>` command integration
//! - Useful for quickly sharing bookmarks to mobile devices

use bukurs::models::bookmark::Bookmark;
use bukurs::plugin::{HookResult, Plugin, PluginContext, PluginInfo};

pub struct QrPlugin {
    /// Whether the plugin is enabled
    enabled: bool,
}

impl QrPlugin {
    pub fn new() -> Self {
        Self { enabled: true }
    }

    /// Generate a QR code as ASCII/Unicode art for terminal display
    /// Returns a string representation of the QR code
    pub fn generate_qr(data: &str) -> Result<String, String> {
        // Use qrcode crate to generate QR code
        let code = qrcode::QrCode::new(data.as_bytes())
            .map_err(|e| format!("Failed to generate QR code: {}", e))?;

        // Render as Unicode using block characters
        // Each module is represented by █ (full) or   (space)
        let mut output = String::new();
        let width = code.width();

        // Add quiet zone (border)
        let quiet_zone = 2;

        // Top quiet zone
        for _ in 0..quiet_zone {
            output.push_str(&"  ".repeat(width + quiet_zone * 2));
            output.push('\n');
        }

        // QR code content
        for y in 0..width {
            // Left quiet zone
            output.push_str(&"  ".repeat(quiet_zone));

            for x in 0..width {
                let color = code[(x, y)];
                if color == qrcode::Color::Dark {
                    output.push_str("██");
                } else {
                    output.push_str("  ");
                }
            }

            // Right quiet zone
            output.push_str(&"  ".repeat(quiet_zone));
            output.push('\n');
        }

        // Bottom quiet zone
        for _ in 0..quiet_zone {
            output.push_str(&"  ".repeat(width + quiet_zone * 2));
            output.push('\n');
        }

        Ok(output)
    }

    /// Generate compact QR code using half-block characters
    /// This uses ▀▄█ characters to fit 2 rows in 1 line
    pub fn generate_qr_compact(data: &str) -> Result<String, String> {
        let code = qrcode::QrCode::new(data.as_bytes())
            .map_err(|e| format!("Failed to generate QR code: {}", e))?;

        let mut output = String::new();
        let width = code.width();

        // Process two rows at a time
        let mut y = 0;
        while y < width {
            for x in 0..width {
                let top = code[(x, y)] == qrcode::Color::Dark;
                let bottom = if y + 1 < width {
                    code[(x, y + 1)] == qrcode::Color::Dark
                } else {
                    false
                };

                let ch = match (top, bottom) {
                    (true, true) => '█',
                    (true, false) => '▀',
                    (false, true) => '▄',
                    (false, false) => ' ',
                };
                output.push(ch);
            }
            output.push('\n');
            y += 2;
        }

        Ok(output)
    }
}

impl Default for QrPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for QrPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "qr".to_string(),
            version: "1.0.0".to_string(),
            description: "Generates QR codes for bookmark URLs".to_string(),
            author: "bukurs".to_string(),
        }
    }

    fn on_load(&mut self, ctx: &PluginContext) -> HookResult {
        if let Some(enabled) = ctx.config.get("enabled") {
            self.enabled = enabled != "false";
        }
        HookResult::Continue
    }

    fn on_pre_open(&self, _ctx: &PluginContext, bookmark: &Bookmark) -> HookResult {
        if !self.enabled {
            return HookResult::Continue;
        }

        // Log that QR generation is available
        log::debug!("QR code available for bookmark {}: {}", bookmark.id, bookmark.url);

        HookResult::Continue
    }
}

/// Create an instance of this plugin (required for auto-discovery)
pub fn create_plugin() -> Box<dyn Plugin> {
    Box::new(QrPlugin::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_qr() {
        let result = QrPlugin::generate_qr("https://example.com");
        assert!(result.is_ok());
        let qr = result.unwrap();
        assert!(qr.contains('█'));
        assert!(qr.contains(' '));
    }

    #[test]
    fn test_generate_qr_compact() {
        let result = QrPlugin::generate_qr_compact("https://example.com");
        assert!(result.is_ok());
        let qr = result.unwrap();
        // Compact version uses half-block characters
        assert!(qr.len() > 0);
    }

    #[test]
    fn test_qr_with_long_url() {
        let long_url = "https://example.com/very/long/path/with/many/segments?param1=value1&param2=value2";
        let result = QrPlugin::generate_qr(long_url);
        assert!(result.is_ok());
    }
}
