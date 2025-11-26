use super::{AppContext, BukuCommand};
use bukurs::error::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditCommand {
    pub id: Option<usize>,
}

impl BukuCommand for EditCommand {
    fn execute(&self, ctx: &AppContext) -> Result<()> {
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
                                        return Err(bukurs::error::BukursError::InvalidInput(
                                            format!("Duplicate URL: {}", edited.url)
                                        ));
                                    }
                                }
                                Err(bukurs::error::BukursError::Database(e))
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
                                        return Err(bukurs::error::BukursError::InvalidInput(
                                            format!("Duplicate URL: {}", new_bookmark.url)
                                        ));
                                    }
                                }
                                Err(bukurs::error::BukursError::Database(e))
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
