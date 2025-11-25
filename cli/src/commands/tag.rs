use super::{AppContext, BukuCommand};
use crate::format::OutputFormat;
use serde::{Deserialize, Serialize};
use std::error::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagCommand {
    pub tags: Vec<String>,
    pub limit: Option<usize>,
    pub format: Option<String>,
    pub nc: bool,
}

impl BukuCommand for TagCommand {
    fn execute(&self, ctx: &AppContext) -> Result<(), Box<dyn Error>> {
        if self.tags.is_empty() {
            eprintln!("Listing all tags (not implemented yet)");
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

            let format: OutputFormat = self
                .format
                .as_deref()
                .map(OutputFormat::from_string)
                .unwrap_or(OutputFormat::Colored);
            format.print_bookmarks(&records, self.nc);
        }
        Ok(())
    }
}
