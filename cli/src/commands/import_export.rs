use super::{AppContext, BukuCommand};
use bukurs::error::Result;
use crate::cli::get_exe_name;
use bukurs::import_export;
use console::Term;
use indicatif::{ProgressBar, ProgressStyle};
use serde::{Deserialize, Serialize};

/// Truncate URL to fit terminal width, accounting for spinner, counter, and prefix
fn truncate_url_for_display(url: &str, profile_name: &str) -> String {
    let term = Term::stdout();
    let terminal_width = term.size().1 as usize;

    // Calculate space used by: spinner (2) + space (1) + brackets (2) + counter (max ~5) + space (1) + "Importing from " (16) + profile_name + ": " (2)
    let prefix = format!("Importing from {}: ", profile_name);
    let overhead = 2 + 1 + 2 + 5 + 1 + prefix.len();

    // Reserve some space for safety and avoid wrapping
    let available_width = if terminal_width > overhead + 10 {
        terminal_width - overhead
    } else {
        60 // Fallback to reasonable default
    };

    if url.len() <= available_width {
        url.to_string()
    } else if available_width > 3 {
        format!("{}...", &url[..available_width.saturating_sub(3)])
    } else {
        "...".to_string()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportCommand {
    pub file: String,
}

impl BukuCommand for ImportCommand {
    fn execute(&self, ctx: &AppContext) -> Result<()> {
        let count = if ctx.config.import_threads > 1 {
            eprintln!("Importing with {} threads...", ctx.config.import_threads);
            import_export::import_bookmarks_parallel(ctx.db, &self.file, ctx.config.import_threads)?
        } else {
            import_export::import_bookmarks(ctx.db, &self.file)?
        };
        eprintln!(
            "✓ Successfully imported {} bookmark(s) from {}",
            count, self.file
        );
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportBrowsersCommand {
    pub list: bool,
    pub all: bool,
    pub browsers: Option<Vec<String>>,
}

impl BukuCommand for ImportBrowsersCommand {
    fn execute(&self, ctx: &AppContext) -> Result<()> {
        if self.list {
            // List detected browsers
            let profiles = import_export::list_detected_browsers();
            if profiles.is_empty() {
                eprintln!("No browser profiles detected.");
            } else {
                eprintln!("Detected browser profiles:");
                for profile in profiles {
                    eprintln!("  • {}", profile.display_string());
                }
            }
        } else if self.all {
            // Import from all detected browsers with progress bar
            eprintln!("Importing from all detected browsers...");

            // Create progress bar with spinner
            let pb = ProgressBar::new_spinner();
            pb.set_style(
                ProgressStyle::default_spinner()
                    .template("{spinner:.green} [{pos}] {msg}")
                    .unwrap(),
            );
            pb.enable_steady_tick(std::time::Duration::from_millis(100));

            let result = import_export::auto_import_all_with_progress(
                ctx.db,
                |profile, _current, _total, url| {
                    if let Some(u) = url {
                        // Increment position for display (this is just for showing progress, not actual count)
                        pb.inc(1);
                        // Truncate URL based on terminal width
                        let display_url = truncate_url_for_display(u, &profile.display_string());
                        pb.set_message(format!(
                            "Importing from {}: {}",
                            profile.display_string(),
                            display_url
                        ));
                    } else {
                        // Switching to new profile, reset counter
                        pb.set_position(0);
                        pb.set_message(format!("Importing from {}", profile.display_string()));
                    }
                },
            );

            pb.finish_and_clear();

            match result {
                Ok(count) => {
                    eprintln!("✓ Successfully imported {} total bookmark(s)", count);
                }
                Err(e) => {
                    eprintln!("Error during import: {}", e);
                    return Err(e);
                }
            }
        } else if let Some(browser_list) = &self.browsers {
            // Import from specific browsers with progress bar
            eprintln!("Importing from selected browsers: {:?}", browser_list);

            // Create progress bar with spinner
            let pb = ProgressBar::new_spinner();
            pb.set_style(
                ProgressStyle::default_spinner()
                    .template("{spinner:.green} [{pos}] {msg}")
                    .unwrap(),
            );
            pb.enable_steady_tick(std::time::Duration::from_millis(100));

            let result = import_export::import_from_selected_browsers_with_progress(
                ctx.db,
                browser_list,
                |profile, _current, _total, url| {
                    if let Some(u) = url {
                        // Increment position for display (this is just for showing progress, not actual count)
                        pb.inc(1);
                        // Truncate URL based on terminal width
                        let display_url = truncate_url_for_display(u, &profile.display_string());
                        pb.set_message(format!(
                            "Importing from {}: {}",
                            profile.display_string(),
                            display_url
                        ));
                    } else {
                        // Switching to new profile, reset counter
                        pb.set_position(0);
                        pb.set_message(format!("Importing from {}", profile.display_string()));
                    }
                },
            );

            pb.finish_and_clear();

            match result {
                Ok(count) => {
                    eprintln!("✓ Successfully imported {} total bookmark(s)", count);
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    return Err(e);
                }
            }
        } else {
            eprintln!("Error: Please specify --list, --all, or --browsers");
            eprintln!("Examples:");
            eprintln!("  {} import-browsers --list", get_exe_name());
            eprintln!("  {} import-browsers --all", get_exe_name());
            eprintln!(
                "  {} import-browsers --browsers chrome,firefox",
                get_exe_name()
            );
            return Err("No import option specified".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportCommand {
    pub file: String,
}

impl BukuCommand for ExportCommand {
    fn execute(&self, ctx: &AppContext) -> Result<()> {
        import_export::export_bookmarks(ctx.db, &self.file)?;
        eprintln!("Exported bookmarks to {}", self.file);
        Ok(())
    }
}
