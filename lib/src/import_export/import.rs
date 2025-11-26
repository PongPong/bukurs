use crate::db::BukuDb;
use crate::utils;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;

/// Trait for importing bookmarks from different formats
pub trait BookmarkImporter {
    fn import(&self, db: &BukuDb, path: &Path) -> crate::error::Result<usize>;
}

/// Parsed bookmark ready for import
#[derive(Debug, Clone)]
pub struct ParsedBookmark {
    pub url: String,
    pub title: String,
    pub tags: String,
    pub desc: String,
    pub parent_id: Option<usize>,
}

use std::sync::mpsc::{sync_channel, SyncSender};

/// Parse HTML bookmarks and stream them to a channel
pub fn parse_html_bookmarks_stream(
    path: &Path,
    tx: SyncSender<ParsedBookmark>,
) -> crate::error::Result<()> {
    let html = std::fs::read_to_string(path)?;
    let dom = tl::parse(&html, tl::ParserOptions::default())?;
    let parser = dom.parser();

    let mut folder_stack: Vec<String> = Vec::new();

    // Parse HTML nodes
    for node in dom.nodes() {
        if let Some(tag) = node.as_tag() {
            let tag_name = tag.name().as_utf8_str();

            match tag_name.as_ref() {
                // H3 tags represent folder names
                "H3" | "h3" => {
                    if let Some(folder_name) =
                        utils::trim_both_simd(tag.inner_text(parser).as_ref())
                            .to_string()
                            .into()
                    {
                        if !folder_name.is_empty() {
                            folder_stack.push(folder_name);
                        }
                    }
                }
                // A tags are bookmarks
                "A" | "a" => {
                    if let Some(href) = tag
                        .attributes()
                        .get("HREF")
                        .or_else(|| tag.attributes().get("href"))
                    {
                        let url = href
                            .map(|h| h.as_utf8_str().to_string())
                            .unwrap_or_default();

                        // Skip empty URLs or special URLs
                        if url.is_empty()
                            || url.starts_with("place:")
                            || url.starts_with("javascript:")
                        {
                            continue;
                        }

                        let title =
                            utils::trim_both_simd(tag.inner_text(parser).as_ref()).to_string();

                        // Extract tags from TAGS attribute or use folder path
                        let tags = if let Some(tags_attr) = tag
                            .attributes()
                            .get("TAGS")
                            .or_else(|| tag.attributes().get("tags"))
                        {
                            tags_attr
                                .map(|t| format!(",{},", t.as_utf8_str().trim_matches(',')))
                                .unwrap_or_else(|| {
                                    if folder_stack.is_empty() {
                                        ",".to_string()
                                    } else {
                                        format!(",{},", folder_stack.join(","))
                                    }
                                })
                        } else if folder_stack.is_empty() {
                            ",".to_string()
                        } else {
                            format!(",{},", folder_stack.join(","))
                        };

                        let bookmark = ParsedBookmark {
                            url,
                            title,
                            tags,
                            desc: String::new(),
                            parent_id: None, // Default to None for now
                        };

                        // Send to channel, blocking if full
                        if tx.send(bookmark).is_err() {
                            // Receiver dropped, stop parsing
                            return Ok(());
                        }
                    }
                }
                // /DL closes a folder level
                "/DL" | "/dl" => {
                    folder_stack.pop();
                }
                _ => {}
            }
        }
    }

    Ok(())
}

/// Parse HTML bookmarks without inserting into database (non-streaming version for backward compatibility)
pub fn parse_html_bookmarks(path: &Path) -> Result<Vec<ParsedBookmark>, crate::error::BukursError> {
    let (tx, rx) = sync_channel(1000);
    let path_buf = path.to_path_buf();

    std::thread::spawn(move || {
        let _ = parse_html_bookmarks_stream(&path_buf, tx);
    });

    let mut bookmarks = Vec::new();
    while let Ok(bookmark) = rx.recv() {
        bookmarks.push(bookmark);
    }

    Ok(bookmarks)
}

/// Import bookmarks in parallel using multiple threads and streaming
pub fn import_bookmarks_parallel(
    db: &BukuDb,
    file_path: &str,
    num_threads: usize,
) -> crate::error::Result<usize> {
    let path = Path::new(file_path).to_path_buf();
    // Create a bounded channel for backpressure (buffer size 100)
    let (tx, rx) = sync_channel::<ParsedBookmark>(100);

    // Spawn Producer (Parser) in a separate thread
    thread::spawn(move || {
        if let Err(e) = parse_html_bookmarks_stream(&path, tx) {
            eprintln!("Error parsing bookmarks: {}", e);
        }
    });

    let num_threads = num_threads.max(1);
    let rx = Arc::new(Mutex::new(rx));
    let imported_count = Arc::new(Mutex::new(0));
    let db_path = db.get_path().to_path_buf();

    // Spawn Consumers (Workers)
    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let rx = Arc::clone(&rx);
            let imported = Arc::clone(&imported_count);
            let db_path = db_path.clone();

            thread::spawn(move || {
                // Each thread opens its own DB connection
                if let Ok(thread_db) = BukuDb::open(&db_path) {
                    let mut local_count = 0;

                    loop {
                        // Critical section: get next item from channel
                        let bookmark = {
                            let lock = rx.lock().unwrap();
                            match lock.recv() {
                                Ok(b) => b,
                                Err(_) => break, // Channel closed and empty
                            }
                        };

                        // Insert into DB (outside lock)
                        match thread_db.add_rec(
                            &bookmark.url,
                            &bookmark.title,
                            &bookmark.tags,
                            &bookmark.desc,
                            bookmark.parent_id,
                        ) {
                            Ok(_) => local_count += 1,
                            Err(rusqlite::Error::SqliteFailure(err, _))
                                if err.code == rusqlite::ErrorCode::ConstraintViolation =>
                            {
                                // Skip duplicates
                            }
                            Err(_) => {} // Skip errors but continue
                        }
                    }

                    *imported.lock().unwrap() += local_count;
                }
            })
        })
        .collect();

    // Wait for all threads to complete
    for handle in handles {
        handle.join().unwrap();
    }

    let count = *imported_count.lock().unwrap();
    Ok(count)
}

/// HTML/Netscape Bookmark File importer
pub struct HtmlImporter;

impl BookmarkImporter for HtmlImporter {
    fn import(&self, db: &BukuDb, path: &Path) -> crate::error::Result<usize> {
        // Use the new parsing function
        let bookmarks = parse_html_bookmarks(path)?;
        let mut imported_count = 0;

        for bookmark in bookmarks {
            match db.add_rec(
                &bookmark.url,
                &bookmark.title,
                &bookmark.tags,
                &bookmark.desc,
                bookmark.parent_id,
            ) {
                Ok(_) => imported_count += 1,
                Err(rusqlite::Error::SqliteFailure(err, _))
                    if err.code == rusqlite::ErrorCode::ConstraintViolation =>
                {
                    // Skip duplicate URLs
                    continue;
                }
                Err(e) => return Err(e.into()),
            }
        }

        Ok(imported_count)
    }
}

/// Import bookmarks from browser HTML export file (single-threaded)
pub fn import_bookmarks(db: &BukuDb, file_path: &str) -> crate::error::Result<usize> {
    let path = Path::new(file_path);
    let importer = HtmlImporter;
    importer.import(db, path)
}
