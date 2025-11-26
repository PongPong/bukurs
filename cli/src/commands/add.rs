use super::{AppContext, BukuCommand};
use crate::fetch_ui::fetch_with_spinner;
use bukurs::error::Result;
use bukurs::{fetch, utils};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, OnceLock};

static EMPTY_STRING: OnceLock<Arc<String>> = OnceLock::new();

fn empty_string() -> Arc<String> {
    EMPTY_STRING.get_or_init(|| Arc::new(String::new())).clone()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddCommand {
    pub url: String,
    pub tag: Option<Vec<String>>,
    pub title: Option<String>,
    pub comment: Option<String>,
    pub offline: bool,
}

impl BukuCommand for AddCommand {
    fn execute(&self, ctx: &AppContext) -> Result<()> {
        let tags = self.tag.as_deref().unwrap_or(&[]);

        // Validate tags don't contain spaces
        for t in tags {
            if utils::has_spaces(t) {
                return Err(bukurs::error::BukursError::InvalidInput(format!(
                    "Invalid tag name: '{}' (tags cannot contain spaces)",
                    t
                )));
            }
        }

        // Fetch metadata or use offline mode
        let fetch_result = if self.offline {
            fetch::FetchResult {
                url: self.url.clone(),
                title: empty_string(),
                desc: empty_string(),
                keywords: empty_string(),
            }
        } else {
            match fetch_with_spinner(&self.url, &ctx.config.user_agent) {
                Ok(result) => result,
                Err(e) => {
                    eprintln!("Warning: Failed to fetch metadata: {}", e);
                    eprintln!("Continuing with manual entry...");
                    fetch::FetchResult {
                        url: self.url.clone(),
                        title: empty_string(),
                        desc: empty_string(),
                        keywords: empty_string(),
                    }
                }
            }
        };

        // Determine final title
        let _final_title: &str = if let Some(t) = self.title.as_ref() {
            t.as_str()
        } else if !fetch_result.title.is_empty() {
            fetch_result.title.as_str()
        } else {
            self.url.as_str()
        };

        // Determine final description
        let _desc: &str = self
            .comment
            .as_deref()
            .unwrap_or(fetch_result.desc.as_str());

        // Build tags string
        let tags_str = if tags.is_empty() {
            format!(",{},", fetch_result.keywords)
        } else {
            format!(",{},", tags.join(","))
        };

        // Add to database
        let id_result = ctx.db.add_rec(
            &self.url,
            self.title.as_deref().unwrap_or(""),
            &tags_str,
            self.comment.as_deref().unwrap_or(""),
            None, // parent_id
        );

        match id_result {
            Ok(id) => {
                eprintln!("Added bookmark at index {}", id);
                Ok(())
            }
            Err(e) => {
                if let rusqlite::Error::SqliteFailure(err, _) = &e {
                    // SQLITE_CONSTRAINT_UNIQUE = 2067
                    if err.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE {
                        return Err(bukurs::error::BukursError::InvalidInput(format!(
                            "Duplicate URL: {}",
                            self.url
                        )));
                    }
                }
                Err(bukurs::error::BukursError::Database(e))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bukurs::config::Config;
    use bukurs::db::BukuDb;
    use rstest::rstest;
    use std::path::PathBuf;

    fn setup_ctx(db: &BukuDb) -> AppContext {
        // We need a config and path, but for unit tests of commands that just call handlers,
        // we might need to mock or provide dummy values.
        // Since AppContext holds references, we need the owners to live long enough.
        // This is tricky in a helper function returning AppContext with references to local vars.
        // So we'll do setup in the test or use a fixture that returns the owners.
        panic!("Use setup_test_env instead");
    }

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
    #[case("http://example.com", Some(vec!["tag1".to_string()]), Some("Example".to_string()), None)]
    #[case("http://test.com", None, None, Some("Comment".to_string()))]
    fn test_add_command(
        #[case] url: &str,
        #[case] tag: Option<Vec<String>>,
        #[case] title: Option<String>,
        #[case] comment: Option<String>,
    ) {
        let env = TestEnv::new();
        let cmd = AddCommand {
            url: url.to_string(),
            tag: tag.clone(),
            title: title.clone(),
            comment: comment.clone(),
            offline: true, // Offline to avoid network calls in tests
        };

        let result = cmd.execute(&env.ctx());
        assert!(result.is_ok());

        // Verify it was added
        let records = env
            .db
            .search(&vec![url.to_string()], false, false, false)
            .expect("Search failed");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].url, url);
        if let Some(t) = title {
            assert_eq!(records[0].title, t);
        }
        if let Some(c) = comment {
            assert_eq!(records[0].description, c);
        }
        if let Some(tags) = tag {
            let expected_tags = format!(",{},", tags.join(","));
            assert_eq!(records[0].tags, expected_tags);
        }
    }
}
