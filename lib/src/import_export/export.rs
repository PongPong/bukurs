use crate::db::BukuDb;
use crate::models::bookmark::Bookmark;
use std::error::Error;
use std::fs::File;
use std::io::Write;
use std::path::Path;

/// Trait for exporting bookmarks to different formats
pub trait BookmarkExporter {
    fn export(&self, bookmarks: &[Bookmark], path: &Path) -> Result<(), Box<dyn Error>>;
}

/// HTML/Netscape Bookmark File exporter
pub struct HtmlExporter;

impl BookmarkExporter for HtmlExporter {
    fn export(&self, records: &[Bookmark], path: &Path) -> Result<(), Box<dyn Error>> {
        let mut file = File::create(path)?;
        writeln!(file, "<!DOCTYPE NETSCAPE-Bookmark-file-1>")?;
        writeln!(file, "<!-- This is an automatically generated file.")?;
        writeln!(file, "     It will be read and overwritten.")?;
        writeln!(file, "     DO NOT EDIT! -->")?;
        writeln!(
            file,
            "<META HTTP-EQUIV=\"Content-Type\" CONTENT=\"text/html; charset=UTF-8\">"
        )?;
        writeln!(file, "<TITLE>Bookmarks</TITLE>")?;
        writeln!(file, "<H1>Bookmarks</H1>")?;
        writeln!(file, "<DL><p>")?;

        for bookmark in records {
            writeln!(
                file,
                "    <DT><A HREF=\"{}\" TAGS=\"{}\" ADD_DATE=\"0\">{}</A>",
                bookmark.url, bookmark.tags, bookmark.title
            )?;
            if !bookmark.description.is_empty() {
                writeln!(file, "    <DD>{}", bookmark.description)?;
            }
        }

        writeln!(file, "</DL><p>")?;
        Ok(())
    }
}

/// Markdown exporter
pub struct MarkdownExporter;

impl BookmarkExporter for MarkdownExporter {
    fn export(&self, records: &[Bookmark], path: &Path) -> Result<(), Box<dyn Error>> {
        let mut file = File::create(path)?;
        for bookmark in records {
            writeln!(
                file,
                "[{}]({}) <!-- {} -->",
                bookmark.title, bookmark.url, bookmark.tags
            )?;
        }
        Ok(())
    }
}

/// Org-mode exporter
pub struct OrgExporter;

impl BookmarkExporter for OrgExporter {
    fn export(&self, records: &[Bookmark], path: &Path) -> Result<(), Box<dyn Error>> {
        let mut file = File::create(path)?;
        for bookmark in records {
            let org_tags = if bookmark.tags.is_empty() {
                "".to_string()
            } else {
                format!(" :{}", bookmark.tags.replace(",", ":"))
            };
            writeln!(
                file,
                "* [[{}][{}]] {}:",
                bookmark.url, bookmark.title, org_tags
            )?;
        }
        Ok(())
    }
}

/// Export bookmarks to a file in the specified format
pub fn export_bookmarks(db: &BukuDb, file_path: &str) -> Result<(), Box<dyn Error>> {
    let path = Path::new(file_path);
    let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");

    let records = db.get_rec_all()?;

    let exporter: Box<dyn BookmarkExporter> = match extension {
        "html" => Box::new(HtmlExporter),
        "md" => Box::new(MarkdownExporter),
        "org" => Box::new(OrgExporter),
        _ => return Err(format!("Unsupported export format: {}", extension).into()),
    };

    exporter.export(&records, path)
}
