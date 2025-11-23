use crate::models::bookmark::Bookmark;
use rusqlite::{Connection, Result};
use std::path::Path;

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

        Ok(BukuDb { conn })
    }

    fn log_undo(&self, operation: &str, bookmark_id: usize, data: &str) -> Result<()> {
        let timestamp = chrono::Utc::now().timestamp();
        self.conn.execute(
            "INSERT INTO undo_log (timestamp, operation, bookmark_id, data) VALUES (?1, ?2, ?3, ?4)",
            (timestamp, operation, bookmark_id, data),
        )?;
        Ok(())
    }

    pub fn add_rec(&self, url: &str, title: &str, tags: &str, desc: &str) -> Result<usize> {
        let tx = self.conn.unchecked_transaction()?;

        tx.execute(
            "INSERT INTO bookmarks (URL, metadata, tags, desc) VALUES (?1, ?2, ?3, ?4)",
            (url, title, tags, desc),
        )?;
        let id = tx.last_insert_rowid() as usize;

        let timestamp = chrono::Utc::now().timestamp();
        tx.execute(
            "INSERT INTO undo_log (timestamp, operation, bookmark_id, data) VALUES (?1, ?2, ?3, ?4)",
            (timestamp, "ADD", id, ""),
        )?;

        tx.commit()?;
        Ok(id)
    }

    pub fn get_rec_id(&self, url: &str) -> Result<Option<usize>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id FROM bookmarks WHERE URL = ?1")?;
        let mut rows = stmt.query([url])?;

        if let Some(row) = rows.next()? {
            Ok(Some(row.get(0)?))
        } else {
            Ok(None)
        }
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
                let timestamp = chrono::Utc::now().timestamp();
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
                let timestamp = chrono::Utc::now().timestamp();
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
        deep: bool,
        regex: bool,
    ) -> Result<Vec<Bookmark>> {
        let mut query = "SELECT id, URL, metadata, tags, desc FROM bookmarks WHERE ".to_string();
        let mut params: Vec<String> = Vec::new();
        let mut conditions = Vec::new();

        if regex {
            // SQLite doesn't support REGEXP by default without extension, but we can try or fallback to LIKE
            // For now, let's assume we handle regex in Rust or use LIKE if simple
            // Actually, rusqlite allows defining functions. We should define REGEXP if we want to support it fully.
            // But for this port, let's start with LIKE for non-regex search.
            // If regex is true, we might need to fetch all and filter in Rust if we don't add the function.
            // Let's fetch all and filter in Rust for regex for simplicity in this iteration.
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

        for (i, kw) in keywords.iter().enumerate() {
            let param_name = format!("?{}", i + 1);
            let term = if deep {
                format!("%{}%", kw)
            } else {
                // Default to substring match for now as per 'deep' description in help implies non-deep is word match?
                // Buku help says: --deep match substrings ('pen' matches 'opens')
                // So non-deep means word match? Or exact match?
                // Python buku uses LIKE %kw% by default for many things unless specified otherwise.
                // Let's stick to %kw% for now as it's most useful.
                format!("%{}%", kw)
            };

            // Simple search across all fields
            conditions.push(format!(
                "(URL LIKE {0} OR metadata LIKE {0} OR tags LIKE {0} OR desc LIKE {0})",
                param_name
            ));
            params.push(term);
        }

        if conditions.is_empty() {
            return self.get_rec_all();
        }

        let join_op = if any { " OR " } else { " AND " };
        query.push_str(&conditions.join(join_op));

        let mut stmt = self.conn.prepare(&query)?;
        // rusqlite params need to be &dyn ToSql.
        // We need to convert params to that.
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
        let mut stmt = self.conn.prepare(
            "SELECT id, operation, bookmark_id, data FROM undo_log ORDER BY timestamp DESC LIMIT 1",
        )?;
        let mut rows = stmt.query([])?;

        if let Some(row) = rows.next()? {
            let log_id: usize = row.get(0)?;
            let operation: String = row.get(1)?;
            let bookmark_id: usize = row.get(2)?;
            let data: String = row.get(3)?;

            match operation.as_str() {
                "ADD" => {
                    // Undo ADD: Delete the bookmark
                    self.conn
                        .execute("DELETE FROM bookmarks WHERE id = ?1", [bookmark_id])?;
                }
                "UPDATE" => {
                    // Undo UPDATE: Restore old data
                    if let Ok(old_bookmark) = serde_json::from_str::<Bookmark>(&data) {
                        self.conn.execute(
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
                        self.conn.execute(
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
            self.conn
                .execute("DELETE FROM undo_log WHERE id = ?1", [log_id])?;

            Ok(Some(operation))
        } else {
            Ok(None)
        }
    }
}
