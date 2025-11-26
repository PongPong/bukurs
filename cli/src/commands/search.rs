use super::{AppContext, BukuCommand};
use crate::format::OutputFormat;
use serde::{Deserialize, Serialize};
use std::error::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchCommand {
    pub keywords: Vec<String>,
    pub all: bool,
    pub deep: bool,
    pub regex: bool,
    pub limit: Option<usize>,
    pub format: Option<String>,
    pub nc: bool,
}

impl BukuCommand for SearchCommand {
    fn execute(&self, ctx: &AppContext) -> Result<(), Box<dyn Error>> {
        let any = !self.all;
        eprintln!("Searching for: {:?}", self.keywords);
        let mut records = ctx.db.search(&self.keywords, any, self.deep, self.regex)?;

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

        fn ctx(&self) -> AppContext {
            AppContext {
                db: &self.db,
                config: &self.config,
                db_path: &self.db_path,
            }
        }
    }

    #[rstest]
    #[case(vec!["rust".to_string()], true)]
    #[case(vec!["example".to_string()], true)]
    #[case(vec!["nonexistent".to_string()], false)]
    fn test_search_command(#[case] keywords: Vec<String>, #[case] should_find: bool) {
        let env = TestEnv::new();
        env.db
            .add_rec(
                "http://rust-lang.org",
                "Rust Language",
                "rust,lang",
                "Programming",
                None,
            )
            .expect("Add failed");
        env.db
            .add_rec("http://example.com", "Example", "example", "Test", None)
            .expect("Add failed");

        let cmd = SearchCommand {
            keywords,
            all: false,
            deep: false,
            regex: false,
            limit: None,
            format: None,
            nc: true, // No color for tests
        };

        // We can't easily capture stdout/stderr here to verify output,
        // but we can verify it runs without error.
        // Ideally we would refactor handlers to return results or write to a passed writer.
        let result = cmd.execute(&env.ctx());
        assert!(result.is_ok());
    }
}
