use super::{AppContext, BukuCommand};
use bukurs::error::Result;
use crate::format::OutputFormat;
use bukurs::operations;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrintCommand {
    pub ids: Vec<String>,
    pub limit: Option<usize>,
    pub format: Option<String>,
    pub nc: bool,
}

impl BukuCommand for PrintCommand {
    fn execute(&self, ctx: &AppContext) -> Result<()> {
        // Use the prepare_print operation
        let operation = operations::prepare_print(&self.ids, ctx.db)?;

        // Handle empty results
        if operation.bookmarks.is_empty() {
            match operation.mode {
                operations::SelectionMode::ByKeywords(_) => {
                    eprintln!("No bookmarks found matching the search criteria.");
                }
                _ => {
                    eprintln!("No bookmarks to display.");
                }
            }
            return Ok(());
        }

        // Apply limit if specified
        let mut records = operation.bookmarks;
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
        Ok(())
    }
}
