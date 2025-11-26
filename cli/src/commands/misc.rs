use super::{AppContext, BukuCommand};
use crate::interactive;
use bukurs::browser;
use bukurs::error::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenCommand {
    pub ids: Vec<String>,
}

impl BukuCommand for OpenCommand {
    fn execute(&self, ctx: &AppContext) -> Result<()> {
        if self.ids.is_empty() {
            eprintln!("Opening random bookmark (not implemented yet)");
        } else {
            for arg in &self.ids {
                if let Ok(id) = arg.parse::<usize>() {
                    if let Some(rec) = ctx.db.get_rec_by_id(id)? {
                        eprintln!("Opening: {}", rec.url);
                        browser::open_url(&rec.url)?;
                    } else {
                        eprintln!("Index {} not found", id);
                    }
                } else {
                    eprintln!("Invalid index: {}", arg);
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellCommand;

impl BukuCommand for ShellCommand {
    fn execute(&self, ctx: &AppContext) -> Result<()> {
        interactive::run(ctx.db)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoCommand {
    pub count: usize,
}

impl BukuCommand for UndoCommand {
    fn execute(&self, ctx: &AppContext) -> Result<()> {
        if self.count == 0 {
            eprintln!("Error: Count must be at least 1");
            return Err("Invalid count".into());
        }

        let mut undone_count = 0;
        let mut operations = Vec::new();

        for i in 0..self.count {
            match ctx.db.undo_last()? {
                Some((op_type, affected)) => {
                    undone_count += 1;
                    operations.push((op_type, affected));
                }
                None => {
                    if i == 0 {
                        eprintln!("Nothing to undo.");
                    } else {
                        eprintln!(
                            "No more operations to undo (undid {} operation(s)).",
                            undone_count
                        );
                    }
                    break;
                }
            }
        }

        if undone_count > 0 {
            if undone_count == 1 {
                let (op_type, affected) = &operations[0];
                if *affected > 1 {
                    eprintln!(
                        "✓ Undid batch {}: {} bookmark(s) reverted",
                        op_type, affected
                    );
                } else {
                    eprintln!("✓ Undid last operation: {}", op_type);
                }
            } else {
                eprintln!("✓ Undid {} operations:", undone_count);
                for (i, (op_type, affected)) in operations.iter().enumerate() {
                    if *affected > 1 {
                        eprintln!("  {}. {} (batch: {} bookmarks)", i + 1, op_type, affected);
                    } else {
                        eprintln!("  {}. {}", i + 1, op_type);
                    }
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoCommand {
    pub keywords: Vec<String>,
    pub open: bool,
    pub format: Option<String>,
    pub nc: bool,
}

impl BukuCommand for NoCommand {
    fn execute(&self, ctx: &AppContext) -> Result<()> {
        // Get records: FTS5 search if keywords provided, otherwise all
        let records = if !self.keywords.is_empty() {
            eprintln!("Searching for: {:?}", self.keywords);
            // Use FTS5 search to filter records
            ctx.db.search(&self.keywords, false, false, false)?
        } else {
            // No keywords, get all records
            ctx.db.get_rec_all()?
        };

        if records.is_empty() {
            eprintln!("No bookmarks found");
            return Ok(());
        }

        // Run fuzzy picker on the (possibly filtered) records and handle selection
        let query = if !self.keywords.is_empty() {
            Some(self.keywords.join(" "))
        } else {
            None
        };

        crate::commands::helpers::handle_bookmark_selection(
            &records,
            query,
            self.open,
            self.format.as_deref(),
            self.nc,
        )?;
        Ok(())
    }
}
