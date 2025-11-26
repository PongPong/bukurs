use super::{AppContext, BukuCommand};
use crate::commands::add::AppError;
use serde::{Deserialize, Serialize};
use std::error::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditCommand {
    pub id: Option<usize>,
}

impl BukuCommand for EditCommand {
    fn execute(&self, ctx: &AppContext) -> Result<(), Box<dyn Error>> {
        match self.id {
            Some(bookmark_id) => {
                // Edit existing bookmark
                let bookmark = ctx
                    .db
                    .get_rec_by_id(bookmark_id)?
                    .ok_or_else(|| format!("Bookmark {} not found", bookmark_id))?;

                eprintln!("Opening bookmark #{} in editor...", bookmark_id);

                match crate::editor::edit_bookmark(&bookmark) {
                    Ok(edited) => {
                        match ctx.db.update_rec_partial(
                            bookmark_id,
                            Some(&edited.url),
                            Some(&edited.title),
                            Some(&edited.tags),
                            Some(&edited.description),
                            None,
                        ) {
                            Ok(()) => {
                                eprintln!("Bookmark {} updated successfully", bookmark_id);
                                Ok(())
                            }
                            Err(e) => {
                                if let rusqlite::Error::SqliteFailure(err, _) = &e {
                                    // SQLITE_CONSTRAINT_UNIQUE = 2067
                                    if err.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE
                                    {
                                        return Err(Box::new(AppError::DuplicateUrl(
                                            edited.url.clone(),
                                        )));
                                    }
                                }
                                Err(Box::new(AppError::DbError))
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Edit cancelled or failed: {}", e);
                        Ok(())
                    }
                }
            }
            None => {
                // Create new bookmark
                eprintln!("Opening editor to create new bookmark...");

                match crate::editor::edit_new_bookmark() {
                    Ok(new_bookmark) => {
                        match ctx.db.add_rec(
                            &new_bookmark.url,
                            &new_bookmark.title,
                            &new_bookmark.tags,
                            &new_bookmark.description,
                            None, // parent_id
                        ) {
                            Ok(id) => {
                                eprintln!("✓ Created new bookmark at index {}", id);
                                Ok(())
                            }
                            Err(e) => {
                                if let rusqlite::Error::SqliteFailure(err, _) = &e {
                                    // SQLITE_CONSTRAINT_UNIQUE = 2067
                                    if err.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE
                                    {
                                        return Err(Box::new(AppError::DuplicateUrl(
                                            new_bookmark.url.clone(),
                                        )));
                                    }
                                }
                                Err(Box::new(AppError::DbError))
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Creation cancelled or failed: {}", e);
                        Ok(())
                    }
                }
            }
        }
    }
}
