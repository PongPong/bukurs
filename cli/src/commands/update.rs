use super::{AppContext, BukuCommand};
use crate::cli::get_exe_name;
use crate::fetch_ui::fetch_with_spinner;
use crate::tag_ops::{apply_tag_operations, parse_tag_operations};
use bukurs::error::Result;
use bukurs::operations;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCommand {
    pub ids: Vec<String>,
    pub url: Option<String>,
    pub tag: Option<Vec<String>>,
    pub title: Option<String>,
    pub comment: Option<String>,
    pub immutable: Option<u8>,
}

impl BukuCommand for UpdateCommand {
    fn execute(&self, ctx: &AppContext) -> Result<()> {
        let has_edit_options = self.url.is_some()
            || self.tag.is_some()
            || self.title.is_some()
            || self.comment.is_some()
            || self.immutable.is_some();

        if self.ids.is_empty() {
            eprintln!("Usage: {} update <ID|RANGE|*> [OPTIONS]", get_exe_name());
            eprintln!("Examples:");
            eprintln!(
                "  {} update 5                  # Refresh metadata for bookmark 5",
                get_exe_name()
            );
            eprintln!(
                "  {} update 1-10               # Refresh metadata for bookmarks 1-10",
                get_exe_name()
            );
            eprintln!(
                "  {} update \"*\"                # Refresh all bookmarks",
                get_exe_name()
            );
            eprintln!(
                "  {} update 5 --tag +urgent    # Add 'urgent' tag",
                get_exe_name()
            );
            eprintln!(
                "  {} update 5 --tag -archived  # Remove 'archived' tag",
                get_exe_name()
            );
            eprintln!(
                "  {} update 5 --tag ~todo:done # Replace 'todo' with 'done'",
                get_exe_name()
            );
            return Err("No bookmark IDs specified".into());
        }

        if has_edit_options {
            // Field update mode
            let operation = operations::prepare_print(&self.ids, ctx.db)?;
            let bookmarks = operation.bookmarks;

            if bookmarks.is_empty() {
                eprintln!("No bookmarks found");
                return Ok(());
            }

            let url_ref = self.url.as_deref();
            let title_str = self.title.as_deref();
            let desc_ref = self.comment.as_deref();
            let tag_operations = self.tag.as_ref().map(|tags| parse_tag_operations(tags));

            if bookmarks.len() > 1 {
                // Batch update mode with parallel processing and progress bar
                eprintln!("Updating {} bookmark(s)...", bookmarks.len());

                let multi = MultiProgress::new();
                let pb = multi.add(ProgressBar::new(bookmarks.len() as u64));
                pb.set_style(
                    ProgressStyle::default_bar()
                        .template("{msg} [{bar:40.cyan/blue}] {pos}/{len}")
                        .unwrap()
                        .progress_chars("=>-"),
                );
                pb.set_message("Processing bookmarks");

                // Now perform the batch update in a single transaction
                let result = if let Some(ref ops) = tag_operations {
                    // Compute updates for each bookmark in parallel
                    let updated_bookmarks: Vec<_> = bookmarks
                        .par_iter()
                        .map(|bookmark| {
                            let mut updated = bookmark.clone();
                            updated.tags = apply_tag_operations(&bookmark.tags, ops);
                            pb.inc(1);
                            updated
                        })
                        .collect();

                    pb.finish_and_clear();

                    ctx.db.update_rec_batch_with_tags(
                        &updated_bookmarks,
                        url_ref,
                        title_str,
                        desc_ref,
                        self.immutable,
                    )
                } else {
                    // No tag operations, just count progress and use original bookmarks
                    bookmarks.par_iter().for_each(|_| pb.inc(1));
                    pb.finish_and_clear();

                    ctx.db.update_rec_batch(
                        &bookmarks,
                        url_ref,
                        title_str,
                        None,
                        desc_ref,
                        self.immutable,
                    )
                };

                match result {
                    Ok((success_count, failed_count)) => {
                        eprintln!();
                        if success_count > 0 {
                            eprintln!("✓ Successfully updated {} bookmark(s)", success_count);
                        }
                        if failed_count > 0 {
                            eprintln!("✗ Failed to update {} bookmark(s)", failed_count);
                        }
                    }
                    Err(e) => {
                        eprintln!("✗ Batch update failed: {}", e);
                        eprintln!("All changes have been rolled back.");
                    }
                }
            } else {
                // Single bookmark update
                let bookmark = &bookmarks[0];

                let final_tags = if let Some(ref ops) = tag_operations {
                    let new_tags = apply_tag_operations(&bookmark.tags, ops);
                    Some(new_tags)
                } else {
                    None
                };

                let tags_ref = final_tags.as_deref();

                match ctx.db.update_rec_partial(
                    bookmark.id,
                    url_ref,
                    title_str,
                    tags_ref,
                    desc_ref,
                    None, // parent_id
                ) {
                    Ok(()) => {
                        eprintln!("✓ Updated bookmark {}", bookmark.id);
                    }
                    Err(e) => {
                        if let rusqlite::Error::SqliteFailure(err, _) = &e {
                            // SQLITE_CONSTRAINT_UNIQUE = 2067
                            if err.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE {
                                eprintln!("✗ Bookmark {}: URL already exists", bookmark.id);
                            } else {
                                eprintln!("✗ Bookmark {}: {}", bookmark.id, e);
                            }
                        } else {
                            eprintln!("✗ Bookmark {}: {}", bookmark.id, e);
                        }
                    }
                }
            }
        } else {
            // Refresh metadata mode
            let operation = operations::prepare_print(&self.ids, ctx.db)?;
            let bookmarks = operation.bookmarks;

            if bookmarks.is_empty() {
                eprintln!("No bookmarks found");
                return Ok(());
            }

            eprintln!("Refreshing metadata for {} bookmark(s)...", bookmarks.len());

            let multi = MultiProgress::new();
            let pb = multi.add(ProgressBar::new(bookmarks.len() as u64));
            pb.set_style(
                ProgressStyle::default_bar()
                    .template("{msg} [{bar:40.cyan/blue}] {pos}/{len}")
                    .unwrap()
                    .progress_chars("=>-"),
            );
            pb.set_message("Overall progress");

            let mut success_count = 0;
            let mut failed_count = 0;
            let mut failed_ids: Vec<usize> = Vec::new();

            for bookmark in &bookmarks {
                match fetch_with_spinner(&bookmark.url, &ctx.config.user_agent) {
                    Ok(fetch_result) => {
                        let new_title = if !fetch_result.title.is_empty() {
                            Some(fetch_result.title.as_str())
                        } else {
                            None
                        };

                        let new_desc = if !fetch_result.desc.is_empty() {
                            Some(fetch_result.desc.as_str())
                        } else {
                            None
                        };

                        match ctx.db.update_rec_partial(
                            bookmark.id,
                            None,
                            new_title,
                            None,
                            new_desc,
                            None,
                        ) {
                            Ok(()) => success_count += 1,
                            Err(_) => {
                                failed_count += 1;
                                failed_ids.push(bookmark.id);
                            }
                        }
                    }
                    Err(_) => {
                        failed_count += 1;
                        failed_ids.push(bookmark.id);
                    }
                }
                pb.inc(1);
            }

            pb.finish_and_clear();

            if success_count > 0 {
                eprintln!("✓ Successfully refreshed {} bookmark(s)", success_count);
            }
            if failed_count > 0 {
                eprintln!("✗ Failed to refresh {} bookmark(s)", failed_count);
                eprintln!(
                    "   Failed IDs: {}",
                    failed_ids
                        .iter()
                        .map(|id| id.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                eprintln!(
                    "   To retry: {} update {}",
                    get_exe_name(),
                    failed_ids
                        .iter()
                        .map(|id| id.to_string())
                        .collect::<Vec<_>>()
                        .join(" ")
                );
            }
        }

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
    fn test_update_command() {
        let env = TestEnv::new();
        // Add a bookmark first
        let id = env
            .db
            .add_rec(
                "http://example.com",
                "Old Title",
                "old,tags",
                "Old Desc",
                None,
            )
            .expect("Add failed");

        let cmd = UpdateCommand {
            ids: vec![id.to_string()],
            url: Some("http://new.com".to_string()),
            tag: Some(vec!["new".to_string(), "tags".to_string()]),
            title: Some("New Title".to_string()),
            comment: Some("New Desc".to_string()),
            immutable: None,
        };

        let result = cmd.execute(&env.ctx());
        assert!(result.is_ok());

        let rec = env
            .db
            .get_rec_by_id(id)
            .expect("Get failed")
            .expect("Bookmark not found");
        assert_eq!(rec.url, "http://new.com");
        assert_eq!(rec.title, "New Title");
        // Tags are added with tag operations, so we expect the new tags to be added
        // The old tags are not removed unless specified with -tag or ~old:new syntax
        assert!(rec.tags.contains("new") && rec.tags.contains("tags"));
        assert_eq!(rec.description, "New Desc");
    }
}
