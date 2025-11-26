use super::{AppContext, BukuCommand};
use bukurs::error::Result;
use serde::{Deserialize, Serialize};

/// Command to search bookmarks by tags with fuzzy search support
///
/// When no tags are provided:
/// 1. Opens a fuzzy picker to select from all unique tags
/// 2. Searches bookmarks with the selected tag
/// 3. Opens another fuzzy picker to select a specific bookmark
///
/// When tags are provided:
/// 1. Searches bookmarks matching the provided tags
/// 2. Opens a fuzzy picker to select a specific bookmark
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagCommand {
    pub tags: Vec<String>,
    pub limit: Option<usize>,
    pub format: Option<String>,
    pub nc: bool,
    pub open: bool,
}

impl BukuCommand for TagCommand {
    fn execute(&self, ctx: &AppContext) -> Result<()> {
        if self.tags.is_empty() {
            // Get all unique tags and run fuzzy picker
            let tags = ctx.db.get_all_tags()?;
            if tags.is_empty() {
                eprintln!("No tags found in the database.");
                return Ok(());
            }

            eprintln!("Selecting from {} unique tags...", tags.len());

            // Run fuzzy picker on tags
            if let Some(selected_tag) = bukurs::fuzzy::run_fuzzy_tag_search(&tags)? {
                eprintln!("Selected tag: {}", selected_tag);

                // Search bookmarks by the selected tag
                // Pass as slice without cloning - db.search_tags will borrow the String
                let mut records = ctx.db.search_tags(std::slice::from_ref(&selected_tag))?;
                if records.is_empty() {
                    eprintln!("No bookmarks found with tag: {}", selected_tag);
                    return Ok(());
                }

                // Apply limit if specified
                if let Some(limit) = self.limit {
                    let start = records.len().saturating_sub(limit);
                    records = records.into_iter().skip(start).collect();
                }

                // Run fuzzy picker on the bookmarks and handle selection
                crate::commands::helpers::handle_bookmark_selection(
                    &records,
                    None,
                    self.open,
                    self.format.as_deref(),
                    self.nc,
                )?;
            }
        } else {
            eprintln!("Searching tags: {:?}", self.tags);
            let mut records = ctx.db.search_tags(&self.tags)?;
            if records.is_empty() {
                eprintln!("No bookmarks found with the specified tags.");
                return Ok(());
            }

            // Apply limit if specified
            if let Some(limit) = self.limit {
                let start = records.len().saturating_sub(limit);
                records = records.into_iter().skip(start).collect();
            }

            // Run fuzzy picker on the filtered records and handle selection
            crate::commands::helpers::handle_bookmark_selection(
                &records,
                Some(self.tags.join(" ")),
                self.open,
                self.format.as_deref(),
                self.nc,
            )?;
        }
        Ok(())
    }
}
