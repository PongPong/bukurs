use super::{AppContext, BukuCommand};
use bukurs::error::Result;
use bukurs::operations;
use indicatif::{ProgressBar, ProgressStyle};
use serde::{Deserialize, Serialize};
use std::io::{self, Write};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteCommand {
    pub ids: Vec<String>,
    pub force: bool,
}

impl BukuCommand for DeleteCommand {
    fn execute(&self, ctx: &AppContext) -> Result<()> {
        let operation = operations::prepare_delete(&self.ids, ctx.db)?;

        if operation.bookmarks.is_empty() {
            match operation.mode {
                operations::SelectionMode::ByKeywords(_) => {
                    eprintln!("No bookmarks found matching the search criteria.");
                }
                _ => {
                    eprintln!("No bookmarks to delete.");
                }
            }
            return Ok(());
        }

        // Display bookmarks to be deleted
        match &operation.mode {
            operations::SelectionMode::All => {
                eprintln!("⚠️  DELETE ALL BOOKMARKS:");
            }
            operations::SelectionMode::ByKeywords(keywords) => {
                eprintln!("Searching for bookmarks matching: {:?}", keywords);
                eprintln!("Bookmarks matching search criteria:");
            }
            operations::SelectionMode::ByIds(_) => {
                eprintln!("Bookmarks to be deleted:");
            }
        }

        for bookmark in &operation.bookmarks {
            eprintln!("  {}. {} - {}", bookmark.id, bookmark.title, bookmark.url);
        }

        // Ask for confirmation unless --force
        let confirmed = if self.force {
            true
        } else {
            let prompt = match operation.mode {
                operations::SelectionMode::All => {
                    format!(
                        "\n⚠️  DELETE ALL {} bookmark(s)? [y/N]: ",
                        operation.bookmarks.len()
                    )
                }
                _ => {
                    format!(
                        "\nDelete {} bookmark(s)? [y/N]: ",
                        operation.bookmarks.len()
                    )
                }
            };

            print!("{}", prompt);
            io::stdout().flush()?;

            let mut response = String::new();
            io::stdin().read_line(&mut response)?;
            let response = response.trim().to_lowercase();
            response == "y" || response == "yes"
        };

        if confirmed {
            // Show progress bar for batch deletes
            if operation.selected_ids.len() > 1 {
                let pb = ProgressBar::new(operation.selected_ids.len() as u64);
                pb.set_style(
                    ProgressStyle::default_bar()
                        .template("{msg} [{bar:40.cyan/blue}] {pos}/{len}")
                        .unwrap()
                        .progress_chars("=>-"),
                );
                pb.set_message("Deleting bookmarks");

                // The actual deletion happens in the database layer
                let count = operations::execute_delete(&operation, ctx.db)?;

                pb.set_position(count as u64);
                pb.finish_and_clear();

                eprintln!("Deleted {} bookmark(s).", count);
            } else {
                let count = operations::execute_delete(&operation, ctx.db)?;
                eprintln!("Deleted {} bookmark(s).", count);
            }
        } else {
            eprintln!("Deletion cancelled.");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bukurs::config::Config;
    use bukurs::db::BukuDb;
    use rstest::rstest;
    use std::path::PathBuf;

    struct TestEnv {
        db: BukuDb,
        config: Config,
        db_path: PathBuf,
    }

    impl TestEnv {
        fn new() -> Self {
            let db = BukuDb::init_in_memory().expect("Failed to init in-memory DB");
            let config = Config::default();
            let db_path = PathBuf::from(":memory:");
            Self {
                db,
                config,
                db_path,
            }
        }

        fn ctx(&self) -> AppContext<'_> {
            AppContext {
                db: &self.db,
                config: &self.config,
                db_path: &self.db_path,
            }
        }
    }

    #[rstest]
    fn test_delete_command() {
        let env = TestEnv::new();
        // Add a bookmark first
        let id = env
            .db
            .add_rec("http://example.com", "Title", "tags", "Desc", None)
            .expect("Add failed");

        let cmd = DeleteCommand {
            ids: vec![id.to_string()],
            force: true, // Force to skip confirmation in tests
        };

        let result = cmd.execute(&env.ctx());
        assert!(result.is_ok());

        let rec = env.db.get_rec_by_id(id).expect("Get failed");
        assert!(rec.is_none());
    }
}
