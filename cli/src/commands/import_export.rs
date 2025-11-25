use super::{AppContext, BukuCommand};
use crate::cli::get_exe_name;
use bukurs::import_export;
use serde::{Deserialize, Serialize};
use std::error::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportCommand {
    pub file: String,
}

impl BukuCommand for ImportCommand {
    fn execute(&self, ctx: &AppContext) -> Result<(), Box<dyn Error>> {
        let count = import_export::import_bookmarks(ctx.db, &self.file)?;
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
    fn execute(&self, ctx: &AppContext) -> Result<(), Box<dyn Error>> {
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
            // Import from all detected browsers
            eprintln!("Importing from all detected browsers...");
            match import_export::auto_import_all(ctx.db) {
                Ok(count) => {
                    eprintln!("✓ Successfully imported {} total bookmark(s)", count);
                }
                Err(e) => {
                    eprintln!("Error during import: {}", e);
                    return Err(e);
                }
            }
        } else if let Some(browser_list) = &self.browsers {
            // Import from specific browsers
            eprintln!("Importing from selected browsers: {:?}", browser_list);
            match import_export::import_from_selected_browsers(ctx.db, browser_list) {
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
    fn execute(&self, ctx: &AppContext) -> Result<(), Box<dyn Error>> {
        import_export::export_bookmarks(ctx.db, &self.file)?;
        eprintln!("Exported bookmarks to {}", self.file);
        Ok(())
    }
}
