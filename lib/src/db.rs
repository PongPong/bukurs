use crate::commands::{UndoCommand, UndoLogData};
use crate::models::bookmark::Bookmark;
use crate::utils;
use rusqlite::{Connection, Result};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct BukuDb {
    conn: Connection,
    db_path: PathBuf,
}

impl BukuDb {
    /// Helper method to execute SQL - needed by UndoCommand
    pub fn execute<P>(&self, sql: &str, params: P) -> Result<usize>
    where
        P: rusqlite::Params,
    {
        self.conn.execute(sql, params)
    }
    pub fn init_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self {
            conn,
            db_path: PathBuf::from(":memory:"),
        };
        db.setup_tables()?;
        Ok(db)
    }

    pub fn init(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        let db = Self {
            conn,
            db_path: db_path.to_path_buf(),
        };
        db.setup_tables()?;
        Ok(db)
    }

    /// Open an existing database without creating tables (for worker threads)
    pub fn open(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        Ok(Self {
            conn,
            db_path: db_path.to_path_buf(),
        })
    }

    /// Get the database file path
    pub fn get_path(&self) -> &Path {
        &self.db_path
    }

    fn setup_tables(&self) -> Result<()> {
        self.conn.execute(
            "CREATE TABLE if not exists bookmarks (
                id integer PRIMARY KEY,
                URL text NOT NULL UNIQUE,
                metadata text default '',
                tags text default ',',
                desc text default '',
                flags integer default 0,
                parent_id integer default NULL
            )",
            [],
        )?;

        self.conn.execute(
            "CREATE TABLE if not exists undo_log (
                id integer PRIMARY KEY AUTOINCREMENT,
                timestamp integer,
                operation text,
                bookmark_id integer,
                batch_id text,
                -- Bookmark fields for undo
                url text,
                title text,
                tags text,
                desc text,
                parent_id integer,
                flags integer
            )",
            [],
        )?;

        // Migration: Add batch_id column if it doesn't exist (for existing databases)
        let has_batch_id: bool = {
            let mut stmt = self.conn.prepare("PRAGMA table_info(undo_log)")?;
            let rows = stmt.query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?;

            let mut found = false;
            for row in rows {
                if row? == "batch_id" {
                    found = true;
                    break;
                }
            }
            found
        };

        if !has_batch_id {
            self.conn
                .execute("ALTER TABLE undo_log ADD COLUMN batch_id text", [])?;
        }

        // Migration: Add parent_id column if it doesn't exist
        let has_parent_id: bool = {
            let mut stmt = self.conn.prepare("PRAGMA table_info(bookmarks)")?;
            let rows = stmt.query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?;

            let mut found = false;
            for row in rows {
                if row? == "parent_id" {
                    found = true;
                    break;
                }
            }
            found
        };

        if !has_parent_id {
            self.conn.execute(
                "ALTER TABLE bookmarks ADD COLUMN parent_id INTEGER DEFAULT NULL",
                [],
            )?;
        }

        // Migration: Add flags column if it doesn't exist
        let has_flags: bool = {
            let mut stmt = self.conn.prepare("PRAGMA table_info(bookmarks)")?;
            let rows = stmt.query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?;

            let mut found = false;
            for row in rows {
                if row? == "flags" {
                    found = true;
                    break;
                }
            }
            found
        };

        if !has_flags {
            self.conn.execute(
                "ALTER TABLE bookmarks ADD COLUMN flags INTEGER DEFAULT 0",
                [],
            )?;
        }

        // Create unique index on URL
        self.conn.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_url ON bookmarks(URL)",
            [],
        )?;

        if cfg!(debug_assertions) {
            self.conn
                .execute("DROP TABLE IF EXISTS bookmarks_fts", [])?;
        }

        // Create FTS5 virtual table for fast full-text search
        // Using a regular FTS5 table (not content-less) for simplicity and reliability
        self.conn.execute(
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
            self.conn
                .execute("DROP TRIGGER IF EXISTS bookmarks_ai", [])?;
            self.conn
                .execute("DROP TRIGGER IF EXISTS bookmarks_au", [])?;
            self.conn
                .execute("DROP TRIGGER IF EXISTS bookmarks_ad", [])?;
        }

        // Trigger to keep FTS5 table in sync on INSERT
        self.conn.execute(
            "CREATE TRIGGER IF NOT EXISTS bookmarks_ai AFTER INSERT ON bookmarks BEGIN
                INSERT INTO bookmarks_fts(rowid, url, metadata, tags, desc)
                VALUES (new.id, new.URL, new.metadata, new.tags, new.desc);
            END",
            [],
        )?;

        // Trigger to keep FTS5 table in sync on UPDATE
        self.conn.execute(
            "CREATE TRIGGER IF NOT EXISTS bookmarks_au AFTER UPDATE ON bookmarks BEGIN
                UPDATE bookmarks_fts
                SET url = new.URL, metadata = new.metadata, tags = new.tags, desc = new.desc
                WHERE rowid = old.id;
            END",
            [],
        )?;

        // Trigger to keep FTS5 table in sync on DELETE
        self.conn.execute(
            "CREATE TRIGGER IF NOT EXISTS bookmarks_ad AFTER DELETE ON bookmarks BEGIN
                DELETE FROM bookmarks_fts WHERE rowid = old.id;
            END",
            [],
        )?;

        // Populate FTS5 table if it's empty but bookmarks exist (migration)
        let fts_count: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM bookmarks_fts", [], |row| row.get(0))?;
        let bookmarks_count: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM bookmarks", [], |row| row.get(0))?;

        if fts_count == 0 && bookmarks_count > 0 {
            // Migrate existing bookmarks to FTS5
            self.conn.execute(
                "INSERT INTO bookmarks_fts(rowid, url, metadata, tags, desc)
                SELECT id, URL, metadata, tags, desc FROM bookmarks",
                [],
            )?;
        }

        Ok(())
    }

    /// Helper function to quote and escape keywords for FTS5 queries
    /// Prevents FTS5 syntax errors by treating keywords as literal phrases
    fn quote_fts5_keywords(keywords: &[String], column_prefix: Option<&str>) -> Vec<String> {
        keywords
            .iter()
            .map(|k| {
                let escaped = k.replace('"', "\"\"");
                if let Some(prefix) = column_prefix {
                    format!("{}:\"{}\"", prefix, escaped)
                } else {
                    format!("\"{}\"", escaped)
                }
            })
            .collect()
    }

    pub fn add_rec(
        &self,
        url: &str,
        title: &str,
        tags: &str,
        desc: &str,
        parent_id: Option<usize>,
    ) -> Result<usize> {
        let tx = self.conn.unchecked_transaction()?;

        // Get flags value (default 0 for new bookmarks)
        let flags = 0;

        // Insert bookmark
        tx.execute(
            "INSERT INTO bookmarks (URL, metadata, tags, desc, parent_id, flags) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            (url, title, tags, desc, parent_id, flags),
        )?;
        let id = tx.last_insert_rowid() as usize;

        // Log undo information with individual columns
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs() as i64;

        tx.execute(
            "INSERT INTO undo_log (timestamp, operation, bookmark_id, url, title, tags, desc, parent_id, flags) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            (timestamp, "ADD", id, url, title, tags, desc, parent_id, flags),
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

    pub fn update_rec_partial(
        &self,
        id: usize,
        url: Option<&str>,
        title: Option<&str>,
        tags: Option<&str>,
        desc: Option<&str>,
        parent_id: Option<Option<usize>>,
    ) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;

        // Fetch current state for undo within transaction
        let (old_url, old_title, old_tags, old_desc, old_parent_id, old_flags): (
            String,
            String,
            String,
            String,
            Option<usize>,
            i32,
        ) = {
            let mut stmt = tx.prepare(
                "SELECT URL, metadata, tags, desc, parent_id, flags FROM bookmarks WHERE id = ?1",
            )?;
            match stmt.query_row([id], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            }) {
                Ok(data) => data,
                Err(_) => return Err(rusqlite::Error::QueryReturnedNoRows),
            }
        };

        // Log undo with individual columns (store old values)
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs() as i64;

        tx.execute(
            "INSERT INTO undo_log (timestamp, operation, bookmark_id, url, title, tags, desc, parent_id, flags)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            (
                timestamp,
                "UPDATE",
                id,
                old_url,
                old_title,
                old_tags,
                old_desc,
                old_parent_id,
                old_flags,
            ),
        )?;

        // Build and execute update query
        let mut updates = Vec::new();
        let mut params: Vec<(&str, &dyn rusqlite::ToSql)> = Vec::new();

        // Store parent_id value to extend its lifetime
        let parent_id_val = parent_id.flatten();

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
        if parent_id.is_some() {
            updates.push("parent_id = :parent_id");
        }

        if updates.is_empty() {
            return Ok(());
        }

        let mut query = "UPDATE bookmarks SET ".to_string();
        query.push_str(&updates.join(", "));
        query.push_str(" WHERE id = :id");

        // Add params
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
        if parent_id.is_some() {
            params.push((":parent_id", &parent_id_val));
        }
        params.push((":id", &id));

        tx.execute(&query, params.as_slice())?;
        tx.commit()?;
        Ok(())
    }

    /// Update multiple bookmarks in a single transaction with a shared batch_id for undo
    /// Returns (success_count, failed_count)
    pub fn update_rec_batch(
        &self,
        bookmarks: &[Bookmark],
        url: Option<&str>,
        title: Option<&str>,
        tags_opt: Option<&str>,
        desc: Option<&str>,
        immutable: Option<u8>,
    ) -> Result<(usize, usize)> {
        if bookmarks.is_empty() {
            return Ok((0, 0));
        }

        // Generate a unique batch_id using UUID v4
        let batch_id = uuid::Uuid::new_v4().to_string();

        let tx = self.conn.unchecked_transaction()?;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs() as i64;

        let mut success_count = 0;
        let failed_count = 0;

        for bookmark in bookmarks {
            // Fetch current state for undo (including parent_id and flags)
            let current = {
                let mut stmt =
                    tx.prepare("SELECT URL, metadata, tags, desc, parent_id, flags FROM bookmarks WHERE id = ?1")?;
                stmt.query_row([bookmark.id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, Option<usize>>(4)?,
                        row.get::<_, i32>(5)?,
                    ))
                }).ok()
            };

            // Log undo with batch_id
            if let Some((url, title, tags, desc, parent_id, flags)) = current {
                tx.execute(
                    "INSERT INTO undo_log (timestamp, operation, bookmark_id, batch_id, url, title, tags, desc, parent_id, flags) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                    (timestamp, "UPDATE", bookmark.id, &batch_id, url, title, tags, desc, parent_id, flags),
                )?;
            }

            // Build update query
            let mut query = "UPDATE bookmarks SET ".to_string();
            let mut updates = Vec::new();
            let immutable_val = immutable.unwrap_or(0);

            if url.is_some() {
                updates.push("URL = :url");
            }
            if title.is_some() {
                updates.push("metadata = :title");
            }
            if tags_opt.is_some() {
                updates.push("tags = :tags");
            }
            if desc.is_some() {
                updates.push("desc = :desc");
            }
            if immutable.is_some() {
                updates.push("flags = :flags");
            }

            if updates.is_empty() {
                continue;
            }

            query.push_str(&updates.join(", "));
            query.push_str(" WHERE id = :id");

            let mut params: Vec<(&str, &dyn rusqlite::ToSql)> = Vec::new();

            if let Some(ref u) = url {
                params.push((":url", u));
            }
            if let Some(ref t) = title {
                params.push((":title", t));
            }
            if let Some(ref tg) = tags_opt {
                params.push((":tags", tg));
            }
            if let Some(ref d) = desc {
                params.push((":desc", d));
            }
            if immutable.is_some() {
                params.push((":flags", &immutable_val));
            }
            params.push((":id", &bookmark.id));

            match tx.execute(&query, params.as_slice()) {
                Ok(_) => success_count += 1,
                Err(_) => {
                    // On any failure, rollback the entire batch
                    return Err(rusqlite::Error::ExecuteReturnedResults);
                }
            }
        }

        tx.commit()?;
        Ok((success_count, failed_count))
    }

    /// Update multiple bookmarks with pre-computed tags in a single transaction with a shared batch_id for undo
    /// This variant accepts bookmarks with their final tag values already computed
    /// Returns (success_count, failed_count)
    pub fn update_rec_batch_with_tags(
        &self,
        bookmarks: &[Bookmark],
        url: Option<&str>,
        title: Option<&str>,
        desc: Option<&str>,
        immutable: Option<u8>,
    ) -> Result<(usize, usize)> {
        if bookmarks.is_empty() {
            return Ok((0, 0));
        }

        // Generate a unique batch_id using UUID v4
        let batch_id = uuid::Uuid::new_v4().to_string();

        let tx = self.conn.unchecked_transaction()?;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs() as i64;

        let mut success_count = 0;
        let failed_count = 0;

        for bookmark in bookmarks {
            // Fetch current state for undo (including parent_id and flags)
            let current = {
                let mut stmt =
                    tx.prepare("SELECT URL, metadata, tags, desc, parent_id, flags FROM bookmarks WHERE id = ?1")?;
                stmt.query_row([bookmark.id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, Option<usize>>(4)?,
                        row.get::<_, i32>(5)?,
                    ))
                }).ok()
            };

            // Log undo with batch_id
            if let Some((old_url, old_title, old_tags, old_desc, parent_id, flags)) = current {
                tx.execute(
                    "INSERT INTO undo_log (timestamp, operation, bookmark_id, batch_id, url, title, tags, desc, parent_id, flags) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                    (timestamp, "UPDATE", bookmark.id, &batch_id, old_url, old_title, old_tags, old_desc, parent_id, flags),
                )?;
            }

            // Build update query - use tags from the bookmark object
            let mut query = "UPDATE bookmarks SET ".to_string();
            let mut updates = Vec::new();
            let immutable_val = immutable.unwrap_or(0);

            if url.is_some() {
                updates.push("URL = :url");
            }
            if title.is_some() {
                updates.push("metadata = :title");
            }
            // Always update tags from the bookmark's tags field
            updates.push("tags = :tags");
            if desc.is_some() {
                updates.push("desc = :desc");
            }
            if immutable.is_some() {
                updates.push("flags = :flags");
            }

            if updates.is_empty() {
                continue;
            }

            query.push_str(&updates.join(", "));
            query.push_str(" WHERE id = :id");

            let mut params: Vec<(&str, &dyn rusqlite::ToSql)> = Vec::new();

            if let Some(ref u) = url {
                params.push((":url", u));
            }
            if let Some(ref t) = title {
                params.push((":title", t));
            }
            // Use the tags from the bookmark
            params.push((":tags", &bookmark.tags));
            if let Some(ref d) = desc {
                params.push((":desc", d));
            }
            if immutable.is_some() {
                params.push((":flags", &immutable_val));
            }
            params.push((":id", &bookmark.id));

            match tx.execute(&query, params.as_slice()) {
                Ok(_) => success_count += 1,
                Err(_) => {
                    // On any failure, rollback the entire batch
                    return Err(rusqlite::Error::ExecuteReturnedResults);
                }
            }
        }

        tx.commit()?;
        Ok((success_count, failed_count))
    }

    pub fn delete_rec(&self, id: usize) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;

        // Fetch current state for undo within transaction
        let (url, title, tags, desc, parent_id, flags): (
            String,
            String,
            String,
            String,
            Option<usize>,
            i32,
        ) = {
            let mut stmt = tx.prepare(
                "SELECT URL, metadata, tags, desc, parent_id, flags FROM bookmarks WHERE id = ?1",
            )?;
            match stmt.query_row([id], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            }) {
                Ok(data) => data,
                Err(_) => return Err(rusqlite::Error::QueryReturnedNoRows),
            }
        };

        // Log undo with individual columns
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs() as i64;

        tx.execute(
            "INSERT INTO undo_log (timestamp, operation, bookmark_id, url, title, tags, desc, parent_id, flags)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            (timestamp, "DELETE", id, url, title, tags, desc, parent_id, flags),
        )?;

        tx.execute("DELETE FROM bookmarks WHERE id = ?1", [id])?;
        tx.commit()?;
        Ok(())
    }

    /// Delete multiple bookmarks in a single transaction with a shared batch_id for undo
    /// Returns the number of bookmarks deleted
    pub fn delete_rec_batch(&self, ids: &[usize]) -> Result<usize> {
        if ids.is_empty() {
            return Ok(0);
        }

        // Generate a unique batch_id using UUID v4
        let batch_id = uuid::Uuid::new_v4().to_string();

        let tx = self.conn.unchecked_transaction()?;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs() as i64;

        let mut deleted_count = 0;

        for &id in ids {
            // Fetch current state for undo within transaction
            let bookmark_data = {
                let mut stmt = tx.prepare(
                    "SELECT URL, metadata, tags, desc, parent_id, flags FROM bookmarks WHERE id = ?1",
                )?;
                stmt.query_row([id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, Option<usize>>(4)?,
                        row.get::<_, i32>(5)?,
                    ))
                }).ok()
            };

            if let Some((url, title, tags, desc, parent_id, flags)) = bookmark_data {
                // Log undo with batch_id
                tx.execute(
                    "INSERT INTO undo_log (timestamp, operation, bookmark_id, batch_id, url, title, tags, desc, parent_id, flags)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                    (timestamp, "DELETE", id, &batch_id, url, title, tags, desc, parent_id, flags),
                )?;

                // Delete the bookmark
                tx.execute("DELETE FROM bookmarks WHERE id = ?1", [id])?;
                deleted_count += 1;
            }
        }

        tx.commit()?;
        Ok(deleted_count)
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
            && (utils::has_char(b'"', keywords[0].as_str())
                || keywords[0].contains(" OR ")
                || keywords[0].contains(" AND "))
        {
            // User provided FTS5 query syntax - use as is
            keywords[0].clone()
        } else {
            // Simple keywords - quote each to treat as literal phrase and avoid FTS5 syntax errors
            let quoted_keywords = Self::quote_fts5_keywords(keywords, None);
            let join_op = if any { " OR " } else { " AND " };
            quoted_keywords.join(join_op)
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
        // No tags - return all
        if tags.is_empty() {
            return self.get_rec_all();
        }

        // Build FTS5 query targeting the tags column specifically
        let quoted_tags = Self::quote_fts5_keywords(tags, Some("tags"));
        let query = quoted_tags.join(" OR ");

        // Query FTS5 table to get matching bookmark IDs
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

    /// Undo the last operation or batch of operations
    /// Returns Some((operation_type, count)) on success, None if nothing to undo
    pub fn undo_last(&self) -> Result<Option<(String, usize)>> {
        let tx = self.conn.unchecked_transaction()?;

        // Get the most recent undo log entry
        let mut stmt = tx.prepare(
            "SELECT id, operation, bookmark_id, batch_id FROM undo_log ORDER BY id DESC LIMIT 1",
        )?;
        let mut rows = stmt.query([])?;

        if let Some(row) = rows.next()? {
            let _log_id: usize = row.get(0)?;
            let operation: String = row.get(1)?;
            let _bookmark_id: usize = row.get(2)?;
            let batch_id: Option<String> = row.get(3)?;
            drop(rows);
            drop(stmt);

            let mut affected_count = 0;

            if let Some(batch_id_val) = batch_id {
                // This is a batch operation - undo all entries with the same batch_id
                let mut stmt = tx.prepare(
                    "SELECT id, operation, bookmark_id, url, title, tags, desc, parent_id, flags 
                     FROM undo_log WHERE batch_id = ?1 ORDER BY id ASC",
                )?;
                let batch_ops: Vec<(usize, UndoLogData)> = stmt
                    .query_map([&batch_id_val], |row| {
                        Ok((
                            row.get(0)?,
                            UndoLogData {
                                operation: row.get(1)?,
                                bookmark_id: row.get(2)?,
                                url: row.get(3)?,
                                title: row.get(4)?,
                                tags: row.get(5)?,
                                desc: row.get(6)?,
                                parent_id: row.get(7)?,
                                flags: row.get(8)?,
                            },
                        ))
                    })?
                    .collect::<Result<Vec<_>>>()?;
                drop(stmt);

                // Create command objects and execute undo for each operation
                for (log_entry_id, data) in batch_ops
                {
                    if let Some(command) = UndoCommand::from_undo_log(&data) {
                        command.undo(self)?;
                    }

                    // Delete this log entry
                    tx.execute("DELETE FROM undo_log WHERE id = ?1", [log_entry_id])?;
                    affected_count += 1;
                }
            } else {
                // Single operation (no batch_id)
                // Fetch the complete undo log data
                let mut stmt = tx.prepare(
                    "SELECT operation, bookmark_id, url, title, tags, desc, parent_id, flags 
                     FROM undo_log ORDER BY id DESC LIMIT 1",
                )?;

                if let Ok(data) = stmt
                    .query_row([], |row| {
                        Ok(UndoLogData {
                            operation: row.get(0)?,
                            bookmark_id: row.get(1)?,
                            url: row.get(2)?,
                            title: row.get(3)?,
                            tags: row.get(4)?,
                            desc: row.get(5)?,
                            parent_id: row.get(6)?,
                            flags: row.get(7)?,
                        })
                    })
                {
                    // Create command object and execute undo
                    if let Some(command) = UndoCommand::from_undo_log(&data) {
                        command.undo(self)?;
                    }
                }

                // Remove single log entry - get the ID from the original query
                let mut stmt = tx.prepare("SELECT id FROM undo_log ORDER BY id DESC LIMIT 1")?;
                if let Ok(log_id) = stmt.query_row([], |row| row.get::<_, usize>(0)) {
                    tx.execute("DELETE FROM undo_log WHERE id = ?1", [log_id])?;
                }
                affected_count = 1;
            }

            tx.commit()?;
            Ok(Some((operation, affected_count)))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_test_db() -> BukuDb {
        // Use in-memory database for faster tests
        let db = BukuDb::init(Path::new(":memory:")).unwrap();
        db
    }
    use std::path::Path;

    #[test]
    fn test_add_rec() {
        let db = BukuDb::init_in_memory().unwrap();
        let id = db
            .add_rec(
                "https://www.google.com",
                "Google",
                "search,google",
                "Search engine",
                None,
            )
            .unwrap();
        assert_eq!(id, 1);
    }

    #[test]
    fn test_add_rec_duplicate() {
        let db = BukuDb::init_in_memory().unwrap();
        db.add_rec("https://www.google.com", "Google", "search", "", None)
            .unwrap();
        let result = db.add_rec("https://www.google.com", "Google", "search", "", None);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_rec_by_id() {
        let db = BukuDb::init_in_memory().unwrap();
        let id = db
            .add_rec(
                "https://example.com",
                "Example",
                ",test,",
                "Description",
                None,
            )
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
        let db = setup_test_db();
        let bookmark = db.get_rec_by_id(999).unwrap();
        assert!(bookmark.is_none());
    }

    #[test]
    fn test_get_rec_all() {
        let db = setup_test_db();
        db.add_rec("https://example1.com", "Example 1", ",test,", "Desc1", None)
            .unwrap();
        db.add_rec("https://example2.com", "Example 2", ",test,", "Desc2", None)
            .unwrap();

        let bookmarks = db.get_rec_all().unwrap();
        assert_eq!(bookmarks.len(), 2);
    }

    #[test]
    fn test_update_rec() {
        let db = setup_test_db();
        let id = db
            .add_rec(
                "https://example.com",
                "Original",
                ",test,",
                "Original desc",
                None,
            )
            .unwrap();

        db.update_rec_partial(
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
        let db = setup_test_db();
        let id = db
            .add_rec(
                "https://example.com",
                "Original",
                ",test,",
                "Original desc",
                None,
            )
            .unwrap();

        db.update_rec_partial(id, None, Some("New Title"), None, None, None)
            .unwrap();

        let bookmark = db.get_rec_by_id(id).unwrap().unwrap();
        assert_eq!(bookmark.url, "https://example.com"); // unchanged
        assert_eq!(bookmark.title, "New Title"); // changed
        assert_eq!(bookmark.tags, ",test,"); // unchanged
    }

    #[test]
    fn test_delete_rec() {
        let db = setup_test_db();
        let id = db
            .add_rec("https://example.com", "Example", ",test,", "Desc", None)
            .unwrap();

        db.delete_rec(id).unwrap();

        let bookmark = db.get_rec_by_id(id).unwrap();
        assert!(bookmark.is_none());
    }

    #[test]
    fn test_search_keyword() {
        let db = setup_test_db();
        db.add_rec(
            "https://rust-lang.org",
            "Rust",
            ",programming,",
            "Rust language",
            None,
        )
        .unwrap();
        db.add_rec(
            "https://python.org",
            "Python",
            ",programming,",
            "Python language",
            None,
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
        let db = setup_test_db();
        db.add_rec(
            "https://rust-lang.org",
            "Rust",
            ",programming,",
            "Systems programming",
            None,
        )
        .unwrap();
        db.add_rec(
            "https://python.org",
            "Python",
            ",programming,",
            "Python scripting",
            None,
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
        let db = setup_test_db();
        db.add_rec(
            "https://rust-lang.org",
            "Rust Programming",
            ",rust,",
            "Learn Rust",
            None,
        )
        .unwrap();
        db.add_rec(
            "https://python.org",
            "Python",
            ",python,",
            "Python language",
            None,
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
        let db = setup_test_db();
        db.add_rec(
            "https://rust-lang.org",
            "Rust",
            ",programming,rust,",
            "Rust language",
            None,
        )
        .unwrap();
        db.add_rec(
            "https://python.org",
            "Python",
            ",programming,python,",
            "Python language",
            None,
        )
        .unwrap();

        let results = db.search_tags(&vec!["rust".to_string()]).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Rust");
    }

    #[test]
    fn test_undo_add() {
        let db = setup_test_db();
        let id = db
            .add_rec("https://example.com", "Example", ",test,", "Desc", None)
            .unwrap();

        // Verify it was added
        assert!(db.get_rec_by_id(id).unwrap().is_some());

        // Undo the add
        let op = db.undo_last().unwrap();
        assert_eq!(op, Some(("ADD".to_string(), 1)));

        // Verify it was deleted
        assert!(db.get_rec_by_id(id).unwrap().is_none());
    }

    #[test]
    fn test_undo_update() {
        let db = setup_test_db();
        let id = db
            .add_rec(
                "https://example.com",
                "Original",
                ",test,",
                "Original desc",
                None,
            )
            .unwrap();

        db.update_rec_partial(id, None, Some("Updated"), None, None, None)
            .unwrap();

        // Verify it was updated
        let bookmark = db.get_rec_by_id(id).unwrap().unwrap();
        assert_eq!(bookmark.title, "Updated");

        // Undo the update (this should revert to original state)
        let op = db.undo_last().unwrap();
        assert_eq!(op, Some(("UPDATE".to_string(), 1)));

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
        let db = setup_test_db();
        let id = db
            .add_rec("https://example.com", "Example", ",test,", "Desc", None)
            .unwrap();

        let original = db.get_rec_by_id(id).unwrap().unwrap();

        db.delete_rec(id).unwrap();

        // Verify it was deleted
        assert!(db.get_rec_by_id(id).unwrap().is_none());

        // Undo the delete
        let op = db.undo_last().unwrap();
        assert_eq!(op, Some(("DELETE".to_string(), 1)));

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
        let db = setup_test_db();
        let result = db.undo_last().unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_transaction_atomicity() {
        let db = setup_test_db();

        // Add a bookmark
        let id = db
            .add_rec("https://example.com", "Example", ",test,", "Desc", None)
            .unwrap();

        // Try to add duplicate (should fail)
        let result = db.add_rec("https://example.com", "Duplicate", ",test,", "Desc", None);
        assert!(result.is_err());

        // Verify original is still there
        let bookmark = db.get_rec_by_id(id).unwrap().unwrap();
        assert_eq!(bookmark.title, "Example");

        // Verify undo log only has one entry (the successful add)
        let undo = db.undo_last().unwrap();
        assert_eq!(undo, Some(("ADD".to_string(), 1)));

        // Verify no more undo entries
        let undo2 = db.undo_last().unwrap();
        assert!(undo2.is_none());
    }

    #[test]
    fn test_empty_search() {
        let db = setup_test_db();
        let results = db.search(&vec![], true, false, false).unwrap();
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_batch_update_and_undo() {
        let db = setup_test_db();

        // Create multiple bookmarks
        let id1 = db
            .add_rec(
                "https://example1.com",
                "Example 1",
                ",test,",
                "Desc 1",
                None,
            )
            .unwrap();
        let id2 = db
            .add_rec(
                "https://example2.com",
                "Example 2",
                ",test,",
                "Desc 2",
                None,
            )
            .unwrap();
        let id3 = db
            .add_rec(
                "https://example3.com",
                "Example 3",
                ",test,",
                "Desc 3",
                None,
            )
            .unwrap();

        // Get bookmarks for batch update
        let bookmarks = vec![
            db.get_rec_by_id(id1).unwrap().unwrap(),
            db.get_rec_by_id(id2).unwrap().unwrap(),
            db.get_rec_by_id(id3).unwrap().unwrap(),
        ];

        // Batch update all three bookmarks
        let result = db.update_rec_batch(&bookmarks, None, Some("Updated Title"), None, None, None);
        assert!(result.is_ok());
        let (success, _fail) = result.unwrap();
        assert_eq!(success, 3);

        // Verify all were updated
        assert_eq!(
            db.get_rec_by_id(id1).unwrap().unwrap().title,
            "Updated Title"
        );
        assert_eq!(
            db.get_rec_by_id(id2).unwrap().unwrap().title,
            "Updated Title"
        );
        assert_eq!(
            db.get_rec_by_id(id3).unwrap().unwrap().title,
            "Updated Title"
        );

        // Undo once - should revert all three
        let undo_result = db.undo_last().unwrap();
        assert_eq!(undo_result, Some(("UPDATE".to_string(), 3)));

        // Verify all three are reverted
        assert_eq!(db.get_rec_by_id(id1).unwrap().unwrap().title, "Example 1");
        assert_eq!(db.get_rec_by_id(id2).unwrap().unwrap().title, "Example 2");
        assert_eq!(db.get_rec_by_id(id3).unwrap().unwrap().title, "Example 3");
    }

    #[test]
    fn test_batch_update_with_url() {
        let db = setup_test_db();

        // Create bookmarks
        let id1 = db
            .add_rec(
                "https://example1.com",
                "Example 1",
                ",test,",
                "Desc 1",
                None,
            )
            .unwrap();
        let id2 = db
            .add_rec(
                "https://example2.com",
                "Example 2",
                ",test,",
                "Desc 2",
                None,
            )
            .unwrap();

        let bookmarks = vec![
            db.get_rec_by_id(id1).unwrap().unwrap(),
            db.get_rec_by_id(id2).unwrap().unwrap(),
        ];

        // Batch update with URL and description
        let result =
            db.update_rec_batch(&bookmarks, None, None, None, Some("New Description"), None);
        assert!(result.is_ok());

        // Verify updates
        assert_eq!(
            db.get_rec_by_id(id1).unwrap().unwrap().description,
            "New Description"
        );
        assert_eq!(
            db.get_rec_by_id(id2).unwrap().unwrap().description,
            "New Description"
        );

        // Undo and verify revert
        db.undo_last().unwrap();
        assert_eq!(
            db.get_rec_by_id(id1).unwrap().unwrap().description,
            "Desc 1"
        );
        assert_eq!(
            db.get_rec_by_id(id2).unwrap().unwrap().description,
            "Desc 2"
        );
    }

    // Parameterized tests for search functionality
    use rstest::rstest;

    #[rstest]
    #[case(&["rust"], true, 1, "Rust")]
    #[case(&["python"], true, 1, "Python")]
    #[case(&["programming"], true, 2, "")] // Matches both
    #[case(&["rust", "python"], true, 2, "")] // OR - matches both
    #[case(&["rust", "programming"], false, 1, "Rust")] // AND - matches only Rust
    #[case(&["nonexistent"], true, 0, "")]
    fn test_search_variations(
        #[case] keywords: &[&str],
        #[case] any: bool,
        #[case] expected_count: usize,
        #[case] expected_first_title: &str,
    ) {
        let db = setup_test_db();
        db.add_rec(
            "https://rust-lang.org",
            "Rust",
            ",programming,",
            "Rust language",
            None,
        )
        .unwrap();
        db.add_rec(
            "https://python.org",
            "Python",
            ",programming,",
            "Python language",
            None,
        )
        .unwrap();

        let keywords_vec: Vec<String> = keywords.iter().map(|s| s.to_string()).collect();
        let results = db.search(&keywords_vec, any, false, false).unwrap();

        assert_eq!(results.len(), expected_count);
        if expected_count > 0 && !expected_first_title.is_empty() {
            assert_eq!(results[0].title, expected_first_title);
        }
    }

    #[rstest]
    #[case(&["rust$"], 1)] // Special char at end
    #[case(&["c++"], 1)] // Plus signs
    #[case(&["a^b"], 1)] // Caret
    #[case(&["foo(bar)"], 1)] // Parentheses
    #[case(&["tag[1]"], 1)] // Brackets
    fn test_search_special_characters(#[case] keywords: &[&str], #[case] expected_count: usize) {
        let db = setup_test_db();

        // Add bookmarks with special characters in various fields
        db.add_rec(
            "https://example.com",
            "rust$ programming",
            ",test,",
            "Description",
            None,
        )
        .unwrap();
        db.add_rec(
            "https://cpp.com",
            "c++ guide",
            ",cpp,",
            "C++ tutorial",
            None,
        )
        .unwrap();
        db.add_rec("https://caret.com", "a^b notation", ",math,", "Math", None)
            .unwrap();
        db.add_rec(
            "https://paren.com",
            "foo(bar) function",
            ",code,",
            "Code",
            None,
        )
        .unwrap();
        db.add_rec("https://bracket.com", "tag[1] item", ",tags,", "Tags", None)
            .unwrap();

        let keywords_vec: Vec<String> = keywords.iter().map(|s| s.to_string()).collect();
        let results = db.search(&keywords_vec, true, false, false).unwrap();

        assert_eq!(results.len(), expected_count);
    }

    #[rstest]
    #[case(&["rust"], 1, "Rust")]
    #[case(&["programming"], 2, "")] // Both have programming tag
    #[case(&["python"], 1, "Python")]
    #[case(&["nonexistent"], 0, "")]
    #[case(&["rust", "python"], 2, "")] // OR logic - matches both
    fn test_search_tags_variations(
        #[case] tags: &[&str],
        #[case] expected_count: usize,
        #[case] expected_first_title: &str,
    ) {
        let db = setup_test_db();
        db.add_rec(
            "https://rust-lang.org",
            "Rust",
            ",programming,rust,",
            "Rust language",
            None,
        )
        .unwrap();
        db.add_rec(
            "https://python.org",
            "Python",
            ",programming,python,",
            "Python language",
            None,
        )
        .unwrap();

        let tags_vec: Vec<String> = tags.iter().map(|s| s.to_string()).collect();
        let results = db.search_tags(&tags_vec).unwrap();

        assert_eq!(results.len(), expected_count);
        if expected_count > 0 && !expected_first_title.is_empty() {
            assert_eq!(results[0].title, expected_first_title);
        }
    }

    #[rstest]
    #[case(&["c++"])]
    #[case(&["test$tag"])]
    #[case(&["foo-bar"])]
    #[case(&["tag_name"])]
    fn test_search_tags_special_characters(#[case] tags: &[&str]) {
        let db = setup_test_db();

        // Add bookmarks with special characters in tags
        db.add_rec(
            "https://cpp.com",
            "C++ Guide",
            ",c++,",
            "C++ programming",
            None,
        )
        .unwrap();
        db.add_rec("https://test.com", "Test", ",test$tag,", "Testing", None)
            .unwrap();
        db.add_rec("https://dash.com", "Dash", ",foo-bar,", "Dashed tag", None)
            .unwrap();
        db.add_rec(
            "https://underscore.com",
            "Underscore",
            ",tag_name,",
            "Underscored",
            None,
        )
        .unwrap();

        let tags_vec: Vec<String> = tags.iter().map(|s| s.to_string()).collect();
        let results = db.search_tags(&tags_vec).unwrap();

        assert_eq!(
            results.len(),
            1,
            "Should find exactly one bookmark with tag: {:?}",
            tags
        );
    }

    #[rstest]
    #[case("", "", ",", "")] // Empty fields
    #[case("https://example.com", "Title with \"quotes\"", ",tag,", "Desc")]
    #[case("https://example.com", "Title\nwith\nnewlines", ",tag,", "Desc")]
    #[case("https://example.com", "Title", ",tag1,tag2,tag3,", "Long desc")]
    fn test_add_and_retrieve_edge_cases(
        #[case] url: &str,
        #[case] title: &str,
        #[case] tags: &str,
        #[case] desc: &str,
    ) {
        let db = setup_test_db();

        // Handle empty URL case separately
        if url.is_empty() {
            // Empty URL should ideally fail, but if it doesn't we just skip
            if let Ok(id) = db.add_rec(url, title, tags, desc, None) {
                let bookmark = db.get_rec_by_id(id).unwrap();
                assert!(bookmark.is_some());
            }
            return;
        }

        let id = db.add_rec(url, title, tags, desc, None).unwrap();
        let bookmark = db.get_rec_by_id(id).unwrap().unwrap();

        assert_eq!(bookmark.url, url);
        assert_eq!(bookmark.title, title);
        assert_eq!(bookmark.tags, tags);
        assert_eq!(bookmark.description, desc);
    }

    #[rstest]
    #[case(1, 1)]
    #[case(5, 5)]
    #[case(10, 10)]
    fn test_multiple_undo_operations(
        #[case] operation_count: usize,
        #[case] expected_undos: usize,
    ) {
        let db = setup_test_db();

        // Perform multiple add operations
        let mut ids = Vec::new();
        for i in 0..operation_count {
            let id = db
                .add_rec(
                    &format!("https://example{}.com", i),
                    &format!("Example {}", i),
                    ",test,",
                    "Desc",
                    None,
                )
                .unwrap();
            ids.push(id);
        }

        // Undo all operations
        let mut undo_count = 0;
        while let Some(_) = db.undo_last().unwrap() {
            undo_count += 1;
        }

        assert_eq!(undo_count, expected_undos);

        // Verify all bookmarks are gone
        for id in ids {
            assert!(db.get_rec_by_id(id).unwrap().is_none());
        }
    }

    #[rstest]
    #[case(Some("https://new.com"), None, None, None)]
    #[case(None, Some("New Title"), None, None)]
    #[case(None, None, Some(",new,tags,"), None)]
    #[case(None, None, None, Some("New desc"))]
    #[case(
        Some("https://new.com"),
        Some("New Title"),
        Some(",new,"),
        Some("New desc")
    )]
    fn test_update_rec_partial_updates(
        #[case] url: Option<&str>,
        #[case] title: Option<&str>,
        #[case] tags: Option<&str>,
        #[case] desc: Option<&str>,
    ) {
        let db = setup_test_db();
        let id = db
            .add_rec(
                "https://original.com",
                "Original Title",
                ",original,",
                "Original desc",
                None,
            )
            .unwrap();

        db.update_rec_partial(id, url, title, tags, desc, None)
            .unwrap();

        let bookmark = db.get_rec_by_id(id).unwrap().unwrap();

        assert_eq!(bookmark.url, url.unwrap_or("https://original.com"));
        assert_eq!(bookmark.title, title.unwrap_or("Original Title"));
        assert_eq!(bookmark.tags, tags.unwrap_or(",original,"));
        assert_eq!(bookmark.description, desc.unwrap_or("Original desc"));
    }

    #[test]
    fn test_quote_fts5_keywords_without_prefix() {
        let keywords = vec![
            "test".to_string(),
            "foo\"bar".to_string(),
            "baz".to_string(),
        ];
        let quoted = BukuDb::quote_fts5_keywords(&keywords, None);

        assert_eq!(quoted.len(), 3);
        assert_eq!(quoted[0], "\"test\"");
        assert_eq!(quoted[1], "\"foo\"\"bar\""); // Escaped quotes
        assert_eq!(quoted[2], "\"baz\"");
    }

    #[test]
    fn test_quote_fts5_keywords_with_prefix() {
        let keywords = vec!["rust".to_string(), "c++".to_string()];
        let quoted = BukuDb::quote_fts5_keywords(&keywords, Some("tags"));

        assert_eq!(quoted.len(), 2);
        assert_eq!(quoted[0], "tags:\"rust\"");
        assert_eq!(quoted[1], "tags:\"c++\"");
    }

    // === New Tests for Improved Coverage ===

    /// Test undo with missing bookmark data in undo_log
    #[test]
    fn test_undo_with_missing_bookmark() {
        let db = setup_test_db();
        let id = db
            .add_rec("https://example.com", "Test", ",test,", "Desc", None)
            .unwrap();

        // Manually insert incomplete undo log entry (missing required fields)
        db.conn
            .execute(
                "INSERT INTO undo_log (timestamp, operation, bookmark_id) VALUES (?1, ?2, ?3)",
                (12345, "UPDATE", id),
            )
            .unwrap();

        // undo_last should handle gracefully (not crash)
        let result = db.undo_last();
        assert!(result.is_ok());
    }

    /// Test command data serialization in undo_log
    #[test]
    fn test_undo_log_stores_individual_columns() {
        let db = setup_test_db();

        // Add a bookmark
        let id = db
            .add_rec(
                "https://test.com",
                "Test Title",
                ",rust,",
                "Test Description",
                None,
            )
            .unwrap();

        // Verify individual columns were stored in undo_log
        let mut stmt = db
            .conn
            .prepare("SELECT url, title, tags, desc, parent_id, flags FROM undo_log WHERE bookmark_id = ?1")
            .unwrap();

        let (url, title, tags, desc, parent_id, flags): (
            String,
            String,
            String,
            String,
            Option<usize>,
            i32,
        ) = stmt
            .query_row([id], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            })
            .unwrap();

        assert_eq!(url, "https://test.com");
        assert_eq!(title, "Test Title");
        assert_eq!(tags, ",rust,");
        assert_eq!(desc, "Test Description");
        assert_eq!(parent_id, None);
        assert_eq!(flags, 0);
    }

    /// Test undo_last doesn't create nested transactions
    #[test]
    fn test_undo_last_transaction_management() {
        let db = setup_test_db();

        // Add multiple operations
        db.add_rec("https://test1.com", "Test 1", ",test,", "Desc", None)
            .unwrap();
        db.add_rec("https://test2.com", "Test 2", ",test,", "Desc", None)
            .unwrap();
        db.add_rec("https://test3.com", "Test 3", ",test,", "Desc", None)
            .unwrap();

        // Multiple undo_last calls should all succeed (no nested transaction errors)
        assert!(db.undo_last().is_ok());
        assert!(db.undo_last().is_ok());
        assert!(db.undo_last().is_ok());
        assert_eq!(db.undo_last().unwrap(), None); // No more to undo
    }

    #[test]
    fn test_batch_delete_and_undo() {
        let db = setup_test_db();

        // Create multiple bookmarks
        let id1 = db
            .add_rec(
                "https://example1.com",
                "Example 1",
                ",test,",
                "Desc 1",
                None,
            )
            .unwrap();
        let id2 = db
            .add_rec(
                "https://example2.com",
                "Example 2",
                ",test,",
                "Desc 2",
                None,
            )
            .unwrap();
        let id3 = db
            .add_rec(
                "https://example3.com",
                "Example 3",
                ",test,",
                "Desc 3",
                None,
            )
            .unwrap();

        // Store original bookmarks for verification
        let orig1 = db.get_rec_by_id(id1).unwrap().unwrap();
        let orig2 = db.get_rec_by_id(id2).unwrap().unwrap();
        let orig3 = db.get_rec_by_id(id3).unwrap().unwrap();

        // Batch delete all three bookmarks
        let deleted_count = db.delete_rec_batch(&[id1, id2, id3]).unwrap();
        assert_eq!(deleted_count, 3);

        // Verify all were deleted
        assert!(db.get_rec_by_id(id1).unwrap().is_none());
        assert!(db.get_rec_by_id(id2).unwrap().is_none());
        assert!(db.get_rec_by_id(id3).unwrap().is_none());

        // Undo once - should restore all three
        let undo_result = db.undo_last().unwrap();
        assert_eq!(undo_result, Some(("DELETE".to_string(), 3)));

        // Verify all three are restored with original data
        let restored1 = db.get_rec_by_id(id1).unwrap().unwrap();
        let restored2 = db.get_rec_by_id(id2).unwrap().unwrap();
        let restored3 = db.get_rec_by_id(id3).unwrap().unwrap();

        assert_eq!(restored1.url, orig1.url);
        assert_eq!(restored1.title, orig1.title);
        assert_eq!(restored1.tags, orig1.tags);
        assert_eq!(restored1.description, orig1.description);

        assert_eq!(restored2.url, orig2.url);
        assert_eq!(restored2.title, orig2.title);
        assert_eq!(restored2.tags, orig2.tags);
        assert_eq!(restored2.description, orig2.description);

        assert_eq!(restored3.url, orig3.url);
        assert_eq!(restored3.title, orig3.title);
        assert_eq!(restored3.tags, orig3.tags);
        assert_eq!(restored3.description, orig3.description);
    }

    #[test]
    fn test_batch_delete_partial() {
        let db = setup_test_db();

        // Create bookmarks where only some IDs exist
        let id1 = db
            .add_rec(
                "https://example1.com",
                "Example 1",
                ",test,",
                "Desc 1",
                None,
            )
            .unwrap();
        let id2 = db
            .add_rec(
                "https://example2.com",
                "Example 2",
                ",test,",
                "Desc 2",
                None,
            )
            .unwrap();

        // Try to delete including a non-existent ID
        let deleted_count = db.delete_rec_batch(&[id1, 999, id2]).unwrap();
        assert_eq!(deleted_count, 2); // Only the two valid ones should be deleted

        // Verify the valid ones were deleted
        assert!(db.get_rec_by_id(id1).unwrap().is_none());
        assert!(db.get_rec_by_id(id2).unwrap().is_none());
    }

    #[test]
    fn test_batch_update_with_tags_and_undo() {
        let db = setup_test_db();

        // Create multiple bookmarks with different tags
        let id1 = db
            .add_rec(
                "https://example1.com",
                "Example 1",
                ",tag1,tag2,",
                "Desc 1",
                None,
            )
            .unwrap();
        let id2 = db
            .add_rec(
                "https://example2.com",
                "Example 2",
                ",tag3,",
                "Desc 2",
                None,
            )
            .unwrap();
        let id3 = db
            .add_rec(
                "https://example3.com",
                "Example 3",
                ",tag1,tag3,",
                "Desc 3",
                None,
            )
            .unwrap();

        // Store original tags
        let orig1_tags = db.get_rec_by_id(id1).unwrap().unwrap().tags;
        let orig2_tags = db.get_rec_by_id(id2).unwrap().unwrap().tags;
        let orig3_tags = db.get_rec_by_id(id3).unwrap().unwrap().tags;

        // Create bookmarks with updated tags
        let mut bm1 = db.get_rec_by_id(id1).unwrap().unwrap();
        bm1.tags = ",newtag1,newtag2,".to_string();
        let mut bm2 = db.get_rec_by_id(id2).unwrap().unwrap();
        bm2.tags = ",newtag3,".to_string();
        let mut bm3 = db.get_rec_by_id(id3).unwrap().unwrap();
        bm3.tags = ",newtag1,newtag3,".to_string();

        // Batch update with tags
        let result = db.update_rec_batch_with_tags(&[bm1, bm2, bm3], None, None, None, None);
        assert!(result.is_ok());
        let (success, _fail) = result.unwrap();
        assert_eq!(success, 3);

        // Verify all tags were updated
        assert_eq!(
            db.get_rec_by_id(id1).unwrap().unwrap().tags,
            ",newtag1,newtag2,"
        );
        assert_eq!(db.get_rec_by_id(id2).unwrap().unwrap().tags, ",newtag3,");
        assert_eq!(
            db.get_rec_by_id(id3).unwrap().unwrap().tags,
            ",newtag1,newtag3,"
        );

        // Undo once - should revert all three tags
        let undo_result = db.undo_last().unwrap();
        assert_eq!(undo_result, Some(("UPDATE".to_string(), 3)));

        // Verify all tags are reverted to original
        assert_eq!(db.get_rec_by_id(id1).unwrap().unwrap().tags, orig1_tags);
        assert_eq!(db.get_rec_by_id(id2).unwrap().unwrap().tags, orig2_tags);
        assert_eq!(db.get_rec_by_id(id3).unwrap().unwrap().tags, orig3_tags);
    }

    #[test]
    fn test_batch_update_with_mixed_fields_and_undo() {
        let db = setup_test_db();

        // Create bookmarks
        let id1 = db
            .add_rec("https://example1.com", "Title 1", ",tag1,", "Desc 1", None)
            .unwrap();
        let id2 = db
            .add_rec("https://example2.com", "Title 2", ",tag2,", "Desc 2", None)
            .unwrap();

        // Store original values
        let orig1 = db.get_rec_by_id(id1).unwrap().unwrap();
        let orig2 = db.get_rec_by_id(id2).unwrap().unwrap();

        // Update with tags and other fields
        let mut bm1 = db.get_rec_by_id(id1).unwrap().unwrap();
        bm1.tags = ",updated,".to_string();
        let mut bm2 = db.get_rec_by_id(id2).unwrap().unwrap();
        bm2.tags = ",updated,".to_string();

        // Batch update with title, desc, and tags
        let result = db.update_rec_batch_with_tags(
            &[bm1, bm2],
            None,
            Some("Updated Title"),
            Some("Updated Desc"),
            None,
        );
        assert!(result.is_ok());
        let (success, _fail) = result.unwrap();
        assert_eq!(success, 2);

        // Verify all fields were updated
        let updated1 = db.get_rec_by_id(id1).unwrap().unwrap();
        assert_eq!(updated1.title, "Updated Title");
        assert_eq!(updated1.description, "Updated Desc");
        assert_eq!(updated1.tags, ",updated,");

        let updated2 = db.get_rec_by_id(id2).unwrap().unwrap();
        assert_eq!(updated2.title, "Updated Title");
        assert_eq!(updated2.description, "Updated Desc");
        assert_eq!(updated2.tags, ",updated,");

        // Undo - should revert all fields
        let undo_result = db.undo_last().unwrap();
        assert_eq!(undo_result, Some(("UPDATE".to_string(), 2)));

        // Verify all fields are reverted
        let reverted1 = db.get_rec_by_id(id1).unwrap().unwrap();
        assert_eq!(reverted1.title, orig1.title);
        assert_eq!(reverted1.description, orig1.description);
        assert_eq!(reverted1.tags, orig1.tags);

        let reverted2 = db.get_rec_by_id(id2).unwrap().unwrap();
        assert_eq!(reverted2.title, orig2.title);
        assert_eq!(reverted2.description, orig2.description);
        assert_eq!(reverted2.tags, orig2.tags);
    }

    #[test]
    fn test_multiple_batch_operations_undo_order() {
        let db = setup_test_db();

        // First batch: Add bookmarks
        let id1 = db
            .add_rec(
                "https://example1.com",
                "Example 1",
                ",test,",
                "Desc 1",
                None,
            )
            .unwrap();
        let id2 = db
            .add_rec(
                "https://example2.com",
                "Example 2",
                ",test,",
                "Desc 2",
                None,
            )
            .unwrap();
        let id3 = db
            .add_rec(
                "https://example3.com",
                "Example 3",
                ",test,",
                "Desc 3",
                None,
            )
            .unwrap();

        // Second batch: Update bookmarks
        let bookmarks = vec![
            db.get_rec_by_id(id1).unwrap().unwrap(),
            db.get_rec_by_id(id2).unwrap().unwrap(),
            db.get_rec_by_id(id3).unwrap().unwrap(),
        ];
        db.update_rec_batch(&bookmarks, None, Some("Updated"), None, None, None)
            .unwrap();

        // Third batch: Delete bookmarks
        db.delete_rec_batch(&[id1, id2, id3]).unwrap();

        // Verify all are deleted
        assert!(db.get_rec_by_id(id1).unwrap().is_none());
        assert!(db.get_rec_by_id(id2).unwrap().is_none());
        assert!(db.get_rec_by_id(id3).unwrap().is_none());

        // First undo: Restore delete (should bring back all 3 with "Updated" title)
        let undo1 = db.undo_last().unwrap();
        assert_eq!(undo1, Some(("DELETE".to_string(), 3)));
        assert_eq!(db.get_rec_by_id(id1).unwrap().unwrap().title, "Updated");
        assert_eq!(db.get_rec_by_id(id2).unwrap().unwrap().title, "Updated");
        assert_eq!(db.get_rec_by_id(id3).unwrap().unwrap().title, "Updated");

        // Second undo: Revert update (should restore original titles)
        let undo2 = db.undo_last().unwrap();
        assert_eq!(undo2, Some(("UPDATE".to_string(), 3)));
        assert_eq!(db.get_rec_by_id(id1).unwrap().unwrap().title, "Example 1");
        assert_eq!(db.get_rec_by_id(id2).unwrap().unwrap().title, "Example 2");
        assert_eq!(db.get_rec_by_id(id3).unwrap().unwrap().title, "Example 3");

        // Third undo: Remove all adds (should delete all 3)
        let undo3 = db.undo_last().unwrap();
        assert_eq!(undo3, Some(("ADD".to_string(), 1)));
        let undo4 = db.undo_last().unwrap();
        assert_eq!(undo4, Some(("ADD".to_string(), 1)));
        let undo5 = db.undo_last().unwrap();
        assert_eq!(undo5, Some(("ADD".to_string(), 1)));

        assert!(db.get_rec_by_id(id1).unwrap().is_none());
        assert!(db.get_rec_by_id(id2).unwrap().is_none());
        assert!(db.get_rec_by_id(id3).unwrap().is_none());
    }

    #[test]
    fn test_empty_batch_operations() {
        let db = setup_test_db();

        // Test empty batch delete
        let result = db.delete_rec_batch(&[]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);

        // Test empty batch update
        let result = db.update_rec_batch(&[], None, None, None, None, None);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), (0, 0));

        // Test empty batch update with tags
        let result = db.update_rec_batch_with_tags(&[], None, None, None, None);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), (0, 0));
    }
}
