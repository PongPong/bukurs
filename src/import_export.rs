use crate::db::BukuDb;
use crate::models::bookmark::Bookmark;
use std::error::Error;
use std::fs::File;
use std::io::Write;
use std::path::Path;

pub fn export_bookmarks(db: &BukuDb, file_path: &str) -> Result<(), Box<dyn Error>> {
    let path = Path::new(file_path);
    let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");

    let records = db.get_rec_all()?;

    match extension {
        "html" => export_html(&records, path),
        "md" => export_md(&records, path),
        "org" => export_org(&records, path),
        _ => Err(format!("Unsupported export format: {}", extension).into()),
    }
}

fn export_html(records: &Vec<Bookmark>, path: &Path) -> Result<(), Box<dyn Error>> {
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

fn export_md(records: &Vec<Bookmark>, path: &Path) -> Result<(), Box<dyn Error>> {
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

fn export_org(records: &Vec<Bookmark>, path: &Path) -> Result<(), Box<dyn Error>> {
    let mut file = File::create(path)?;
    for bookmark in records {
        // Format: * [[url][title]] :tags:
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

pub fn import_bookmarks(db: &BukuDb, file_path: &str) -> Result<(), Box<dyn Error>> {
    // Placeholder for import implementation
    // Parsing HTML bookmarks is non-trivial without a proper parser.
    // We can use `scraper` or regex for simple cases.
    println!("Importing from {} (not implemented yet)", file_path);
    Ok(())
}
