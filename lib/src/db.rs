use crate::models::bookmark::Bookmark;
use rusqlite::{Connection, Result};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct BukuDb {
    conn: Connection,
}

impl BukuDb {
    pub fn init(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)?;

        conn.execute(
            "CREATE TABLE if not exists bookmarks (
                id integer PRIMARY KEY,
                URL text NOT NULL UNIQUE,
                metadata text default '',
                tags text default ',',
                desc text default '',
                flags integer default 0
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE if not exists undo_log (
                id integer PRIMARY KEY AUTOINCREMENT,
                timestamp integer,
                operation text,
                bookmark_id integer,
                data text
            )",
            [],
        )?;
        if cfg!(debug_assertions) {
            conn.execute(
                "DROP TABLE IF EXISTS bookmarks_fts",
                [],
            )?;
        }

        // Create FTS5 virtual table for fast full-text search
        // Using a regular FTS5 table (not content-less) for simplicity and reliability
        conn.execute(
            r#"CREATE VIRTUAL TABLE IF NOT EXISTS bookmarks_fts USING fts5(
                url,
                metadata,
                tags,
                desc,
                tokenize = 'unicode61'
            )"#,
            [],
        )?;

        if cfg!(debug_assertions) {
            // Drop existing triggers if they exist (to handle upgrades)
            conn.execute("DROP TRIGGER IF EXISTS bookmarks_ai", [])?;
            conn.execute("DROP TRIGGER IF EXISTS bookmarks_au", [])?;
            conn.execute("DROP TRIGGER IF EXISTS bookmarks_ad", [])?;
        }

        // Trigger to keep FTS5 table in sync on INSERT
        conn.execute(
            "CREATE TRIGGER IF NOT EXISTS bookmarks_ai AFTER INSERT ON bookmarks BEGIN
                INSERT INTO bookmarks_fts(rowid, url, metadata, tags, desc)
                VALUES (new.id, new.URL, new.metadata, new.tags, new.desc);
            END",
            [],
        )?;

        // Trigger to keep FTS5 table in sync on UPDATE
        conn.execute(
            "CREATE TRIGGER IF NOT EXISTS bookmarks_au AFTER UPDATE ON bookmarks BEGIN
                UPDATE bookmarks_fts
                SET url = new.URL, metadata = new.metadata, tags = new.tags, desc = new.desc
                WHERE rowid = old.id;
            END",
            [],
        )?;

        // Trigger to keep FTS5 table in sync on DELETE
        conn.execute(
            "CREATE TRIGGER IF NOT EXISTS bookmarks_ad AFTER DELETE ON bookmarks BEGIN
                DELETE FROM bookmarks_fts WHERE rowid = old.id;
            END",
            [],
        )?;

        // Populate FTS5 table if it's empty but bookmarks exist (migration)
        let fts_count: i64 =
            conn.query_row("SELECT COUNT(*) FROM bookmarks_fts", [], |row| row.get(0))?;
        let bookmarks_count: i64 =
            conn.query_row("SELECT COUNT(*) FROM bookmarks", [], |row| row.get(0))?;

        if fts_count == 0 && bookmarks_count > 0 {
            // Migrate existing bookmarks to FTS5
            conn.execute(
                "INSERT INTO bookmarks_fts(rowid, url, metadata, tags, desc)
                SELECT id, URL, metadata, tags, desc FROM bookmarks",
                [],
            )?;
        }

        Ok(BukuDb { conn })
    }

    pub fn add_rec(&self, url: &str, title: &str, tags: &str, desc: &str) -> Result<usize> {
        let tx = self.conn.unchecked_transaction()?;

        tx.execute(
            "INSERT INTO bookmarks (URL, metadata, tags, desc) VALUES (?1, ?2, ?3, ?4)",
            (url, title, tags, desc),
        )?;
        let id = tx.last_insert_rowid() as usize;

        // let timestamp = chrono::Utc::now().timestamp();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs() as i64; // i64 to match chrono::timestamp type
        tx.execute(
            "INSERT INTO undo_log (timestamp, operation, bookmark_id, data) VALUES (?1, ?2, ?3, ?4)",
            (timestamp, "ADD", id, ""),
        )?;

        tx.commit()?;
        Ok(id)
    }

    pub fn get_rec_by_id(&self, id: usize) -> Result<Option<Bookmark>> {
        let mut stmt = self
            .conn
            .prepare("SELECT URL, metadata, tags, desc FROM bookmarks WHERE id = ?1")?;
        let mut rows = stmt.query([id])?;

        if let Some(row) = rows.next()? {
            Ok(Some(Bookmark::new(
                id,
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
            )))
        } else {
            Ok(None)
        }
    }

    pub fn get_rec_all(&self) -> Result<Vec<Bookmark>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, URL, metadata, tags, desc FROM bookmarks")?;
        let rows = stmt.query_map([], |row| {
            Ok(Bookmark::new(
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        })?;

        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }
        Ok(records)
    }

    pub fn update_rec(
        &self,
        id: usize,
        url: Option<&str>,
        title: Option<&str>,
        tags: Option<&str>,
        desc: Option<&str>,
        immutable: Option<u8>,
    ) -> Result<()> {
        let mut query = "UPDATE bookmarks SET ".to_string();
        let mut updates = Vec::new();

        // We need to keep values alive if we want to use references in params.
        // But rusqlite execute takes &[&dyn ToSql].
        // A common trick is to use a vector of Box<dyn ToSql> but execute expects references.
        // Or we can just build the query and params dynamically but we need to be careful with lifetimes.

        // Let's try a different approach:
        // We can't easily mix string literals and references to locals in a single Vec for params if locals die.
        // But `url`, `title`, etc are references passed to the function, so they live long enough!
        // The problem is `immutable`. It's `Option<u8>`, so `i` is a u8 (Copy).
        // `&i` is a reference to a local stack variable `i` inside the `if let`.

        // We can shadow `immutable` to extend its lifetime or just use the Option directly if possible?
        // No, we need to pass the value.

        // Let's declare `immutable_val` outside.
        let tx = self.conn.unchecked_transaction()?;

        // Fetch current state for undo within transaction
        {
            let mut stmt =
                tx.prepare("SELECT URL, metadata, tags, desc FROM bookmarks WHERE id = ?1")?;
            let mut rows = stmt.query([id])?;
            let current = if let Some(row) = rows.next()? {
                Some(Bookmark::new(
                    id,
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                ))
            } else {
                None
            };

            // Log undo
            if let Some(ref bookmark) = current {
                let data = serde_json::to_string(bookmark).unwrap_or_default();
                let timestamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("Time went backwards")
                    .as_secs() as i64; // i64 to match chrono::timestamp type
                tx.execute(
                "INSERT INTO undo_log (timestamp, operation, bookmark_id, data) VALUES (?1, ?2, ?3, ?4)",
                (timestamp, "UPDATE", id, &data),
            )?;
            }
        }

        let immutable_val = immutable.unwrap_or(0);

        if url.is_some() {
            updates.push("URL = :url");
        }
        if title.is_some() {
            updates.push("metadata = :title");
        }
        if tags.is_some() {
            updates.push("tags = :tags");
        }
        if desc.is_some() {
            updates.push("desc = :desc");
        }
        if immutable.is_some() {
            updates.push("flags = :flags");
        }

        if updates.is_empty() {
            return Ok(());
        }

        query.push_str(&updates.join(", "));
        query.push_str(" WHERE id = :id");

        // Now construct params. We need to use named parameters.
        // rusqlite `execute` with named params requires a slice of `(&str, &dyn ToSql)`.
        let mut params: Vec<(&str, &dyn rusqlite::ToSql)> = Vec::new();

        if let Some(ref u) = url {
            params.push((":url", u));
        }
        if let Some(ref t) = title {
            params.push((":title", t));
        }
        if let Some(ref tg) = tags {
            params.push((":tags", tg));
        }
        if let Some(ref d) = desc {
            params.push((":desc", d));
        }
        if immutable.is_some() {
            params.push((":flags", &immutable_val));
        }
        params.push((":id", &id));

        tx.execute(&query, params.as_slice())?;
        tx.commit()?;
        Ok(())
    }

    pub fn delete_rec(&self, id: usize) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;

        // Fetch current state for undo within transaction
        {
            let mut stmt =
                tx.prepare("SELECT URL, metadata, tags, desc FROM bookmarks WHERE id = ?1")?;
            let mut rows = stmt.query([id])?;
            let current = if let Some(row) = rows.next()? {
                Some(Bookmark::new(
                    id,
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                ))
            } else {
                None
            };

            // Log undo
            if let Some(ref bookmark) = current {
                let data = serde_json::to_string(bookmark).unwrap_or_default();
                let timestamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("Time went backwards")
                    .as_secs() as i64; // i64 to match chrono::timestamp type
                tx.execute(
                    "INSERT INTO undo_log (timestamp, operation, bookmark_id, data) VALUES (?1, ?2, ?3, ?4)",
                    (timestamp, "DELETE", id, &data),
                )?;
            }
        }

        tx.execute("DELETE FROM bookmarks WHERE id = ?1", [id])?;
        tx.commit()?;
        Ok(())
    }

    pub fn search(
        &self,
        keywords: &[String],
        any: bool,
        _deep: bool, // Deep is implicit with FTS5
        regex: bool,
    ) -> Result<Vec<Bookmark>> {
        // Handle regex search separately (fallback to old method)
        if regex {
            let all_recs = self.get_rec_all()?;
            let re = regex::Regex::new(&keywords[0])
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

            let filtered = all_recs
                .into_iter()
                .filter(|b| {
                    re.is_match(&b.url)
                        || re.is_match(&b.title)
                        || re.is_match(&b.tags)
                        || re.is_match(&b.description)
                })
                .collect();
            return Ok(filtered);
        }

        // No keywords - return all
        if keywords.is_empty() {
            return self.get_rec_all();
        }

        // Build FTS5 query
        let query = if keywords.len() == 1
            && (keywords[0].contains('"')
                || keywords[0].contains(" OR ")
                || keywords[0].contains(" AND "))
        {
            // User provided FTS5 query syntax - use as is
            keywords[0].clone()
        } else {
            // Simple keywords - join based on any/all flag
            let join_op = if any { " OR " } else { " AND " };
            keywords.join(join_op)
        };

        // Query FTS5 table to get matching bookmark IDs (ranked by relevance)
        let mut stmt = self.conn.prepare(
            "SELECT rowid FROM bookmarks_fts WHERE bookmarks_fts MATCH ?1 ORDER BY rank",
        )?;

        let ids: Vec<usize> = stmt
            .query_map([&query], |row| row.get::<_, i64>(0).map(|id| id as usize))?
            .collect::<Result<Vec<_>>>()?;

        if ids.is_empty() {
            return Ok(Vec::new());
        }

        // Fetch full bookmark data for matching IDs
        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query_str = format!(
            "SELECT id, URL, metadata, tags, desc FROM bookmarks WHERE id IN ({})",
            placeholders
        );

        let mut stmt = self.conn.prepare(&query_str)?;
        let params: Vec<&dyn rusqlite::ToSql> =
            ids.iter().map(|id| id as &dyn rusqlite::ToSql).collect();

        let bookmarks = stmt
            .query_map(params.as_slice(), |row| {
                Ok(Bookmark::new(
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })?
            .collect::<Result<Vec<_>>>()?;

        Ok(bookmarks)
    }

    pub fn search_tags(&self, tags: &[String]) -> Result<Vec<Bookmark>> {
        let mut query = "SELECT id, URL, metadata, tags, desc FROM bookmarks WHERE ".to_string();
        let mut params: Vec<String> = Vec::new();
        let mut conditions = Vec::new();

        for (i, tag) in tags.iter().enumerate() {
            let param_name = format!("?{}", i + 1);
            conditions.push(format!("tags LIKE {}", param_name));
            params.push(format!("%{}%", tag));
        }

        if conditions.is_empty() {
            return self.get_rec_all();
        }

        query.push_str(&conditions.join(" OR "));

        let mut stmt = self.conn.prepare(&query)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(|s| s as &dyn rusqlite::ToSql).collect();

        let rows = stmt.query_map(params_refs.as_slice(), |row| {
            Ok(Bookmark::new(
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        })?;

        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }
        Ok(records)
    }

    pub fn undo_last(&self) -> Result<Option<String>> {
        let tx = self.conn.unchecked_transaction()?;

        let mut stmt = tx.prepare(
            "SELECT id, operation, bookmark_id, data FROM undo_log ORDER BY id DESC LIMIT 1",
        )?;
        let mut rows = stmt.query([])?;

        if let Some(row) = rows.next()? {
            let log_id: usize = row.get(0)?;
            let operation: String = row.get(1)?;
            let bookmark_id: usize = row.get(2)?;
            let data: String = row.get(3)?;
            drop(rows);
            drop(stmt);

            match operation.as_str() {
                "ADD" => {
                    // Undo ADD: Delete the bookmark
                    tx.execute("DELETE FROM bookmarks WHERE id = ?1", [bookmark_id])?;
                }
                "UPDATE" => {
                    // Undo UPDATE: Restore old data
                    if let Ok(old_bookmark) = serde_json::from_str::<Bookmark>(&data) {
                        tx.execute(
                            "UPDATE bookmarks SET URL = ?1, metadata = ?2, tags = ?3, desc = ?4 WHERE id = ?5",
                            (
                                old_bookmark.url,
                                old_bookmark.title,
                                old_bookmark.tags,
                                old_bookmark.description,
                                bookmark_id,
                            ),
                        )?;
                    }
                }
                "DELETE" => {
                    // Undo DELETE: Re-insert the bookmark with original ID
                    if let Ok(old_bookmark) = serde_json::from_str::<Bookmark>(&data) {
                        tx.execute(
                            "INSERT INTO bookmarks (id, URL, metadata, tags, desc) VALUES (?1, ?2, ?3, ?4, ?5)",
                            (
                                old_bookmark.id,
                                old_bookmark.url,
                                old_bookmark.title,
                                old_bookmark.tags,
                                old_bookmark.description,
                            ),
                        )?;
                    }
                }
                _ => {}
            }

            // Remove log entry
            tx.execute("DELETE FROM undo_log WHERE id = ?1", [log_id])?;

            tx.commit()?;
            Ok(Some(operation))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_test_db() -> (BukuDb, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = BukuDb::init(&db_path).unwrap();
        (db, temp_dir)
    }

    #[test]
    fn test_add_rec() {
        let (db, _temp) = setup_test_db();
        let id = db
            .add_rec(
                "https://example.com",
                "Example Site",
                ",test,",
                "A test bookmark",
            )
            .unwrap();
        assert!(id > 0);
    }

    #[test]
    fn test_add_duplicate_url() {
        let (db, _temp) = setup_test_db();
        db.add_rec("https://example.com", "Example", ",test,", "Test")
            .unwrap();
        let result = db.add_rec("https://example.com", "Example2", ",test,", "Test2");
        assert!(result.is_err());
    }

    #[test]
    fn test_get_rec_by_id() {
        let (db, _temp) = setup_test_db();
        let id = db
            .add_rec("https://example.com", "Example", ",test,", "Description")
            .unwrap();

        let bookmark = db.get_rec_by_id(id).unwrap().unwrap();
        assert_eq!(bookmark.id, id);
        assert_eq!(bookmark.url, "https://example.com");
        assert_eq!(bookmark.title, "Example");
        assert_eq!(bookmark.tags, ",test,");
        assert_eq!(bookmark.description, "Description");
    }

    #[test]
    fn test_get_rec_by_id_not_found() {
        let (db, _temp) = setup_test_db();
        let bookmark = db.get_rec_by_id(999).unwrap();
        assert!(bookmark.is_none());
    }

    #[test]
    fn test_get_rec_all() {
        let (db, _temp) = setup_test_db();
        db.add_rec("https://example1.com", "Example 1", ",test,", "Desc1")
            .unwrap();
        db.add_rec("https://example2.com", "Example 2", ",test,", "Desc2")
            .unwrap();

        let bookmarks = db.get_rec_all().unwrap();
        assert_eq!(bookmarks.len(), 2);
    }

    #[test]
    fn test_update_rec() {
        let (db, _temp) = setup_test_db();
        let id = db
            .add_rec("https://example.com", "Original", ",test,", "Original desc")
            .unwrap();

        db.update_rec(
            id,
            Some("https://updated.com"),
            Some("Updated"),
            Some(",updated,"),
            Some("Updated desc"),
            None,
        )
        .unwrap();

        let bookmark = db.get_rec_by_id(id).unwrap().unwrap();
        assert_eq!(bookmark.url, "https://updated.com");
        assert_eq!(bookmark.title, "Updated");
        assert_eq!(bookmark.tags, ",updated,");
        assert_eq!(bookmark.description, "Updated desc");
    }

    #[test]
    fn test_update_partial() {
        let (db, _temp) = setup_test_db();
        let id = db
            .add_rec("https://example.com", "Original", ",test,", "Original desc")
            .unwrap();

        db.update_rec(id, None, Some("New Title"), None, None, None)
            .unwrap();

        let bookmark = db.get_rec_by_id(id).unwrap().unwrap();
        assert_eq!(bookmark.url, "https://example.com"); // unchanged
        assert_eq!(bookmark.title, "New Title"); // changed
        assert_eq!(bookmark.tags, ",test,"); // unchanged
    }

    #[test]
    fn test_delete_rec() {
        let (db, _temp) = setup_test_db();
        let id = db
            .add_rec("https://example.com", "Example", ",test,", "Desc")
            .unwrap();

        db.delete_rec(id).unwrap();

        let bookmark = db.get_rec_by_id(id).unwrap();
        assert!(bookmark.is_none());
    }

    #[test]
    fn test_search_keyword() {
        let (db, _temp) = setup_test_db();
        db.add_rec(
            "https://rust-lang.org",
            "Rust",
            ",programming,",
            "Rust language",
        )
        .unwrap();
        db.add_rec(
            "https://python.org",
            "Python",
            ",programming,",
            "Python language",
        )
        .unwrap();

        let results = db
            .search(&vec!["rust".to_string()], true, false, false)
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Rust");
    }

    #[test]
    fn test_search_multiple_any() {
        let (db, _temp) = setup_test_db();
        db.add_rec(
            "https://rust-lang.org",
            "Rust",
            ",programming,",
            "Systems programming",
        )
        .unwrap();
        db.add_rec(
            "https://python.org",
            "Python",
            ",programming,",
            "Python scripting",
        )
        .unwrap();

        let results = db
            .search(
                &vec!["rust".to_string(), "python".to_string()],
                true,
                false,
                false,
            )
            .unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_search_multiple_all() {
        let (db, _temp) = setup_test_db();
        db.add_rec(
            "https://rust-lang.org",
            "Rust Programming",
            ",rust,",
            "Learn Rust",
        )
        .unwrap();
        db.add_rec(
            "https://python.org",
            "Python",
            ",python,",
            "Python language",
        )
        .unwrap();

        let results = db
            .search(
                &vec!["rust".to_string(), "programming".to_string()],
                false,
                false,
                false,
            )
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Rust Programming");
    }

    #[test]
    fn test_search_tags() {
        let (db, _temp) = setup_test_db();
        db.add_rec(
            "https://rust-lang.org",
            "Rust",
            ",programming,rust,",
            "Rust language",
        )
        .unwrap();
        db.add_rec(
            "https://python.org",
            "Python",
            ",programming,python,",
            "Python language",
        )
        .unwrap();

        let results = db.search_tags(&vec!["rust".to_string()]).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Rust");
    }

    #[test]
    fn test_undo_add() {
        let (db, _temp) = setup_test_db();
        let id = db
            .add_rec("https://example.com", "Example", ",test,", "Desc")
            .unwrap();

        // Verify it was added
        assert!(db.get_rec_by_id(id).unwrap().is_some());

        // Undo the add
        let op = db.undo_last().unwrap();
        assert_eq!(op, Some("ADD".to_string()));

        // Verify it was deleted
        assert!(db.get_rec_by_id(id).unwrap().is_none());
    }

    #[test]
    fn test_undo_update() {
        let (db, _temp) = setup_test_db();
        let id = db
            .add_rec("https://example.com", "Original", ",test,", "Original desc")
            .unwrap();

        db.update_rec(id, None, Some("Updated"), None, None, None)
            .unwrap();

        // Verify it was updated
        let bookmark = db.get_rec_by_id(id).unwrap().unwrap();
        assert_eq!(bookmark.title, "Updated");

        // Undo the update (this should revert to original state)
        let op = db.undo_last().unwrap();
        assert_eq!(op, Some("UPDATE".to_string()));

        // Verify it was reverted
        let bookmark = db.get_rec_by_id(id).unwrap();
        assert!(
            bookmark.is_some(),
            "Bookmark should exist after undo update"
        );
        assert_eq!(bookmark.unwrap().title, "Original");
    }

    #[test]
    fn test_undo_delete() {
        let (db, _temp) = setup_test_db();
        let id = db
            .add_rec("https://example.com", "Example", ",test,", "Desc")
            .unwrap();

        let original = db.get_rec_by_id(id).unwrap().unwrap();

        db.delete_rec(id).unwrap();

        // Verify it was deleted
        assert!(db.get_rec_by_id(id).unwrap().is_none());

        // Undo the delete
        let op = db.undo_last().unwrap();
        assert_eq!(op, Some("DELETE".to_string()));

        // Verify it was restored
        let restored = db.get_rec_by_id(id).unwrap();
        assert!(
            restored.is_some(),
            "Bookmark should exist after undo delete"
        );
        let restored = restored.unwrap();
        assert_eq!(restored.url, original.url);
        assert_eq!(restored.title, original.title);
    }

    #[test]
    fn test_undo_empty() {
        let (db, _temp) = setup_test_db();
        let result = db.undo_last().unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_transaction_atomicity() {
        let (db, _temp) = setup_test_db();

        // Add a bookmark
        let id = db
            .add_rec("https://example.com", "Example", ",test,", "Desc")
            .unwrap();

        // Try to add duplicate (should fail)
        let result = db.add_rec("https://example.com", "Duplicate", ",test,", "Desc");
        assert!(result.is_err());

        // Verify original is still there
        let bookmark = db.get_rec_by_id(id).unwrap().unwrap();
        assert_eq!(bookmark.title, "Example");

        // Verify undo log only has one entry (the successful add)
        let undo = db.undo_last().unwrap();
        assert_eq!(undo, Some("ADD".to_string()));

        // Verify no more undo entries
        let undo2 = db.undo_last().unwrap();
        assert!(undo2.is_none());
    }

    #[test]
    fn test_empty_search() {
        let (db, _temp) = setup_test_db();
        let results = db.search(&vec![], true, false, false).unwrap();
        assert_eq!(results.len(), 0);
    }
}
