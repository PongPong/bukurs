use super::{AppContext, BukuCommand};
use bukurs::error::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchCommand {
    pub keywords: Vec<String>,
    pub all: bool,
    pub deep: bool,
    pub regex: bool,
    pub limit: Option<usize>,
    pub format: Option<String>,
    pub nc: bool,
    pub open: bool,
}

impl BukuCommand for SearchCommand {
    fn execute(&self, ctx: &AppContext) -> Result<()> {
        let any = !self.all;
        eprintln!("Searching for: {:?}", self.keywords);
        let mut records = ctx.db.search(&self.keywords, any, self.deep, self.regex)?;

        if records.is_empty() {
            eprintln!("No bookmarks found matching the search criteria.");
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
            Some(self.keywords.join(" ")),
            self.open,
            self.format.as_deref(),
            self.nc,
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bukurs::config::Config;
    use bukurs::db::BukuDb;
    use bukurs::plugin::PluginManager;
    use rstest::rstest;
    use std::path::PathBuf;

    struct TestEnv {
        db: BukuDb,
        config: Config,
        db_path: PathBuf,
        plugins: PluginManager,
    }

    impl TestEnv {
        fn new() -> Self {
            let db = BukuDb::init_in_memory().expect("Failed to init in-memory DB");
            let config = Config::default();
            let db_path = PathBuf::from(":memory:");
            let plugins = PluginManager::disabled();
            Self {
                db,
                config,
                db_path,
                plugins,
            }
        }

        fn ctx(&self) -> AppContext<'_> {
            AppContext {
                db: &self.db,
                config: &self.config,
                db_path: &self.db_path,
                plugins: &self.plugins,
            }
        }
    }

    #[rstest]
    #[case(vec!["rust".to_string()], true)]
    #[case(vec!["example".to_string()], true)]
    #[case(vec!["nonexistent".to_string()], false)]
    #[ignore = "Requires interactive terminal for fuzzy picker"]
    fn test_search_command(#[case] keywords: Vec<String>, #[case] _should_find: bool) {
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
            open: false,
        };

        // We can't easily capture stdout/stderr here to verify output,
        // but we can verify it runs without error.
        // Ideally we would refactor handlers to return results or write to a passed writer.
        let result = cmd.execute(&env.ctx());
        if let Err(e) = &result {
            eprintln!("Error: {:?}", e);
        }
        assert!(result.is_ok());
    }
}
