use crate::db::BukuDb;
use rusqlite::Result;

/// Bookmark data from undo log
#[derive(Debug)]
pub struct UndoLogData {
    pub operation: String,
    pub bookmark_id: usize,
    pub url: Option<String>,
    pub title: Option<String>,
    pub tags: Option<String>,
    pub desc: Option<String>,
    pub parent_id: Option<usize>,
    pub flags: Option<i32>,
}

/// Command types for undo operations
#[derive(Debug)]
pub enum UndoCommand {
    Add {
        bookmark_id: usize,
    },
    Update {
        bookmark_id: usize,
        url: String,
        title: String,
        tags: String,
        desc: String,
        parent_id: Option<usize>,
        flags: i32,
    },
    Delete {
        bookmark_id: usize,
        url: String,
        title: String,
        tags: String,
        desc: String,
        parent_id: Option<usize>,
        flags: i32,
    },
}

impl UndoCommand {
    /// Execute undo operation
    pub fn undo(&self, db: &BukuDb) -> Result<()> {
        match self {
            UndoCommand::Add { bookmark_id } => {
                // Undo ADD: delete the bookmark
                db.execute("DELETE FROM bookmarks WHERE id = ?1", [bookmark_id])?;
                Ok(())
            }
            UndoCommand::Update {
                bookmark_id,
                url,
                title,
                tags,
                desc,
                parent_id,
                flags,
            } => {
                // Undo UPDATE: restore old values
                db.execute(
                    "UPDATE bookmarks SET URL = ?1, metadata = ?2, tags = ?3, desc = ?4, parent_id = ?5, flags = ?6 WHERE id = ?7",
                    (url, title, tags, desc, parent_id, flags, bookmark_id),
                )?;
                Ok(())
            }
            UndoCommand::Delete {
                bookmark_id,
                url,
                title,
                tags,
                desc,
                parent_id,
                flags,
            } => {
                // Undo DELETE: restore the bookmark
                db.execute(
                    "INSERT INTO bookmarks (id, URL, metadata, tags, desc, parent_id, flags) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    (bookmark_id, url, title, tags, desc, parent_id, flags),
                )?;
                Ok(())
            }
        }
    }

    /// Create command from undo_log data
    pub fn from_undo_log(data: &UndoLogData) -> Option<Self> {
        match data.operation.as_str() {
            "ADD" => Some(UndoCommand::Add { bookmark_id: data.bookmark_id }),
            "UPDATE" => Some(UndoCommand::Update {
                bookmark_id: data.bookmark_id,
                url: data.url.clone()?,
                title: data.title.clone()?,
                tags: data.tags.clone()?,
                desc: data.desc.clone()?,
                parent_id: data.parent_id,
                flags: data.flags?,
            }),
            "DELETE" => Some(UndoCommand::Delete {
                bookmark_id: data.bookmark_id,
                url: data.url.clone()?,
                title: data.title.clone()?,
                tags: data.tags.clone()?,
                desc: data.desc.clone()?,
                parent_id: data.parent_id,
                flags: data.flags?,
            }),
            _ => None,
        }
    }
}
