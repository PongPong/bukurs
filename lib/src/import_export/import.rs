use crate::db::BukuDb;
use std::error::Error;
use std::path::Path;

/// Trait for importing bookmarks from different formats
pub trait BookmarkImporter {
    fn import(&self, db: &BukuDb, path: &Path) -> Result<usize, Box<dyn Error>>;
}

/// HTML/Netscape Bookmark File importer
pub struct HtmlImporter;

impl BookmarkImporter for HtmlImporter {
    fn import(&self, db: &BukuDb, path: &Path) -> Result<usize, Box<dyn Error>> {
        let html = std::fs::read_to_string(path)?;
        let dom = tl::parse(&html, tl::ParserOptions::default())?;
        let parser = dom.parser();

        let mut imported_count = 0;
        let mut folder_stack: Vec<String> = Vec::new();

        // Parse HTML nodes
        for node in dom.nodes() {
            if let Some(tag) = node.as_tag() {
                let tag_name = tag.name().as_utf8_str();

                match tag_name.as_ref() {
                    // H3 tags represent folder names
                    "H3" | "h3" => {
                        if let Some(folder_name) =
                            tag.inner_text(parser).as_ref().trim().to_string().into()
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

                            let title = tag.inner_text(parser).as_ref().trim().to_string();

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

                            // Try to add bookmark, skip if duplicate URL
                            match db.add_rec(&url, &title, &tags, "") {
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
                    }
                    // /DL closes a folder level
                    "/DL" | "/dl" => {
                        folder_stack.pop();
                    }
                    _ => {}
                }
            }
        }

        Ok(imported_count)
    }
}

/// Import bookmarks from browser HTML export file
/// Supports Chrome and Firefox Netscape Bookmark File format
pub fn import_bookmarks(db: &BukuDb, file_path: &str) -> Result<usize, Box<dyn Error>> {
    let path = Path::new(file_path);
    let importer = HtmlImporter;
    importer.import(db, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn setup_test_db() -> (BukuDb, tempfile::TempDir) {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = BukuDb::init(&db_path).unwrap();
        (db, temp_dir)
    }

    fn create_temp_html(content: &str) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(content.as_bytes()).unwrap();
        file.flush().unwrap();
        file
    }

    #[rstest]
    #[case(
        r#"<!DOCTYPE NETSCAPE-Bookmark-file-1>
<DL><p>
    <DT><A HREF="https://example.com">Example</A>
</DL><p>"#,
        1,
        "https://example.com",
        "Example"
    )]
    #[case(
        r#"<!DOCTYPE NETSCAPE-Bookmark-file-1>
<DL><p>
    <DT><A HREF="https://rust-lang.org">Rust</A>
    <DT><A HREF="https://github.com">GitHub</A>
</DL><p>"#,
        2,
        "https://rust-lang.org",
        "Rust"
    )]
    fn test_import_basic_bookmarks(
        #[case] html: &str,
        #[case] expected_count: usize,
        #[case] expected_url: &str,
        #[case] expected_title: &str,
    ) {
        let (db, _temp_dir) = setup_test_db();
        let temp_file = create_temp_html(html);

        let count = import_bookmarks(&db, temp_file.path().to_str().unwrap()).unwrap();
        assert_eq!(count, expected_count);

        let bookmark = db.get_rec_by_id(1).unwrap().unwrap();
        assert_eq!(bookmark.url, expected_url);
        assert_eq!(bookmark.title, expected_title);
    }

    #[test]
    fn test_import_with_folders() {
        let html = r#"<!DOCTYPE NETSCAPE-Bookmark-file-1>
<DL><p>
    <DT><H3>Work</H3>
    <DL><p>
        <DT><A HREF="https://work.example.com">Work Site</A>
    </DL><p>
    <DT><H3>Personal</H3>
    <DL><p>
        <DT><A HREF="https://personal.example.com">Personal Site</A>
    </DL><p>
</DL><p>"#;

        let (db, _temp_dir) = setup_test_db();
        let temp_file = create_temp_html(html);

        let count = import_bookmarks(&db, temp_file.path().to_str().unwrap()).unwrap();
        assert_eq!(count, 2);

        let bookmark1 = db.get_rec_by_id(1).unwrap().unwrap();
        assert!(bookmark1.tags.contains("Work"));

        let bookmark2 = db.get_rec_by_id(2).unwrap().unwrap();
        assert!(bookmark2.tags.contains("Personal"));
    }

    #[test]
    fn test_import_nested_folders() {
        let html = r#"<!DOCTYPE NETSCAPE-Bookmark-file-1>
<DL><p>
    <DT><H3>Programming</H3>
    <DL><p>
        <DT><H3>Rust</H3>
        <DL><p>
            <DT><A HREF="https://rust-lang.org">Rust Lang</A>
        </DL><p>
    </DL><p>
</DL><p>"#;

        let (db, _temp_dir) = setup_test_db();
        let temp_file = create_temp_html(html);

        let count = import_bookmarks(&db, temp_file.path().to_str().unwrap()).unwrap();
        assert_eq!(count, 1);

        let bookmark = db.get_rec_by_id(1).unwrap().unwrap();
        assert!(bookmark.tags.contains("Programming"));
        assert!(bookmark.tags.contains("Rust"));
    }

    #[test]
    fn test_import_with_tags_attribute() {
        let html = r#"<!DOCTYPE NETSCAPE-Bookmark-file-1>
<DL><p>
    <DT><A HREF="https://example.com" TAGS="rust,programming">Example</A>
</DL><p>"#;

        let (db, _temp_dir) = setup_test_db();
        let temp_file = create_temp_html(html);

        let count = import_bookmarks(&db, temp_file.path().to_str().unwrap()).unwrap();
        assert_eq!(count, 1);

        let bookmark = db.get_rec_by_id(1).unwrap().unwrap();
        assert!(bookmark.tags.contains("rust"));
        assert!(bookmark.tags.contains("programming"));
    }

    #[test]
    fn test_import_skip_duplicates() {
        let html = r#"<!DOCTYPE NETSCAPE-Bookmark-file-1>
<DL><p>
    <DT><A HREF="https://example.com">Example 1</A>
    <DT><A HREF="https://example.com">Example 2</A>
</DL><p>"#;

        let (db, _temp_dir) = setup_test_db();
        let temp_file = create_temp_html(html);

        let count = import_bookmarks(&db, temp_file.path().to_str().unwrap()).unwrap();
        assert_eq!(count, 1, "Should only import first occurrence");
    }

    #[rstest]
    #[case(r#"<DT><A HREF="place:sort=8&maxResults=10">Recent</A>"#)]
    #[case(r#"<DT><A HREF="javascript:alert('hi')">JS Link</A>"#)]
    #[case(r#"<DT><A HREF="">Empty</A>"#)]
    fn test_import_skip_special_urls(#[case] html_fragment: &str) {
        let html = format!(
            r#"<!DOCTYPE NETSCAPE-Bookmark-file-1>
<DL><p>
    {}
</DL><p>"#,
            html_fragment
        );

        let (db, _temp_dir) = setup_test_db();
        let temp_file = create_temp_html(&html);

        let count = import_bookmarks(&db, temp_file.path().to_str().unwrap()).unwrap();
        assert_eq!(count, 0, "Should skip special URLs");
    }

    #[test]
    fn test_import_chrome_format() {
        let html = r#"<!DOCTYPE NETSCAPE-Bookmark-file-1>
<!-- This is an automatically generated file.
     It will be read and overwritten.
     DO NOT EDIT! -->
<META HTTP-EQUIV="Content-Type" CONTENT="text/html; charset=UTF-8">
<TITLE>Bookmarks</TITLE>
<H1>Bookmarks</H1>
<DL><p>
    <DT><H3 ADD_DATE="1234567890" LAST_MODIFIED="1234567891" PERSONAL_TOOLBAR_FOLDER="true">Bookmarks bar</H3>
    <DL><p>
        <DT><A HREF="https://github.com" ADD_DATE="1234567890" ICON="data:image/png;base64,iVBOR...">GitHub</A>
    </DL><p>
</DL><p>"#;

        let (db, _temp_dir) = setup_test_db();
        let temp_file = create_temp_html(html);

        let count = import_bookmarks(&db, temp_file.path().to_str().unwrap()).unwrap();
        assert_eq!(count, 1);

        let bookmark = db.get_rec_by_id(1).unwrap().unwrap();
        assert_eq!(bookmark.url, "https://github.com");
        assert_eq!(bookmark.title, "GitHub");
    }

    #[test]
    fn test_import_firefox_format() {
        let html = r#"<!DOCTYPE NETSCAPE-Bookmark-file-1>
<META HTTP-EQUIV="Content-Type" CONTENT="text/html; charset=UTF-8">
<TITLE>Bookmarks</TITLE>
<H1>Bookmarks Menu</H1>
<DL><p>
    <DT><H3 ADD_DATE="1234567890" LAST_MODIFIED="1234567891">Mozilla Firefox</H3>
    <DL><p>
        <DT><A HREF="https://www.mozilla.org/firefox/central/" ADD_DATE="1234567890" LAST_MODIFIED="1234567891">Getting Started</A>
    </DL><p>
</DL><p>"#;

        let (db, _temp_dir) = setup_test_db();
        let temp_file = create_temp_html(html);

        let count = import_bookmarks(&db, temp_file.path().to_str().unwrap()).unwrap();
        assert_eq!(count, 1);

        let bookmark = db.get_rec_by_id(1).unwrap().unwrap();
        assert!(bookmark.url.contains("mozilla.org"));
    }

    #[test]
    fn test_import_empty_file() {
        let html = r#"<!DOCTYPE NETSCAPE-Bookmark-file-1>
<DL><p>
</DL><p>"#;

        let (db, _temp_dir) = setup_test_db();
        let temp_file = create_temp_html(html);

        let count = import_bookmarks(&db, temp_file.path().to_str().unwrap()).unwrap();
        assert_eq!(count, 0);
    }
}
