use bukurs::db::BukuDb;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use std::error::Error;

pub fn run(db: &BukuDb) -> Result<(), Box<dyn Error>> {
    let mut rl = DefaultEditor::new()?;

    loop {
        let readline = rl.readline("buku (? for help) ");
        match readline {
            Ok(line) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                rl.add_history_entry(line)?;

                match line {
                    "q" | "quit" | "exit" => break,
                    "?" | "help" => print_help(),
                    _ => handle_command(db, line)?,
                }
            }
            Err(ReadlineError::Interrupted) => {
                break;
            }
            Err(ReadlineError::Eof) => {
                break;
            }
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    }
    Ok(())
}

fn print_help() {
    println!(
        "
PROMPT KEYS:
    1-N                    browse search result indices and/or ranges
    s keyword [...]        search for records with ANY keyword
    S keyword [...]        search for records with ALL keywords
    p id|range [...]       print bookmarks by indices and/or ranges
    e ID                   edit bookmark in $EDITOR
    q, ^D                  quit
    ?                      show this help
"
    );
}

fn handle_command(db: &BukuDb, line: &str) -> Result<(), Box<dyn Error>> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.is_empty() {
        return Ok(());
    }

    match parts[0] {
        "s" => handle_search(db, &parts, true),
        "S" => handle_search(db, &parts, false),
        "e" | "edit" => handle_edit(db, &parts),
        "p" => handle_print(&parts),
        _ => handle_open_by_id(db, &parts),
    }
}

fn handle_search(db: &BukuDb, parts: &[&str], any: bool) -> Result<(), Box<dyn Error>> {
    if parts.len() > 1 {
        let keywords: Vec<String> = parts[1..].iter().map(|s| s.to_string()).collect();
        let records = db.search(&keywords, any, false, false)?;

        for bookmark in records {
            println!(
                "{}. {}\n   > {}\n   + {}\n   # {}",
                bookmark.id, bookmark.title, bookmark.url, bookmark.description, bookmark.tags
            );
        }
    } else {
        println!("Search requires keywords");
    }
    Ok(())
}

fn handle_edit(db: &BukuDb, parts: &[&str]) -> Result<(), Box<dyn Error>> {
    if parts.len() < 2 {
        println!("Usage: e <bookmark_id>");
        println!("Example: e 5");
        return Ok(());
    }

    let bookmark_id = match parts[1].parse::<usize>() {
        Ok(id) => id,
        Err(_) => {
            println!("Invalid bookmark ID: {}", parts[1]);
            return Ok(());
        }
    };

    let bookmark = match db.get_rec_by_id(bookmark_id)? {
        Some(b) => b,
        None => {
            println!("Bookmark {} not found", bookmark_id);
            return Ok(());
        }
    };

    println!("Opening bookmark #{} in editor...", bookmark_id);

    let edited = match crate::editor::edit_bookmark(&bookmark) {
        Ok(e) => e,
        Err(e) => {
            println!("Edit cancelled or failed: {}", e);
            return Ok(());
        }
    };

    // Update the database
    match db.update_rec_partial(
        bookmark_id,
        Some(&edited.url),
        Some(&edited.title),
        Some(&edited.tags),
        Some(&edited.description),
        None,
    ) {
        Ok(()) => {
            println!("✓ Bookmark {} updated successfully", bookmark_id);
        }
        Err(e) => {
            if let rusqlite::Error::SqliteFailure(err, _) = &e {
                // SQLITE_CONSTRAINT_UNIQUE = 2067
                if err.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE {
                    eprintln!("✗ Error: URL '{}' already exists", edited.url);
                    return Ok(());
                }
            }
            println!("Error updating bookmark: {}", e);
        }
    }

    Ok(())
}

fn handle_print(_parts: &[&str]) -> Result<(), Box<dyn Error>> {
    // TODO: Implement range parsing
    println!("Print by index/range not fully implemented yet");
    Ok(())
}

fn handle_open_by_id(db: &BukuDb, parts: &[&str]) -> Result<(), Box<dyn Error>> {
    // Check if it's an index
    if let Ok(id) = parts[0].parse::<usize>() {
        if let Some(rec) = db.get_rec_by_id(id)? {
            println!("Opening: {}", rec.url);
            open::that(&rec.url)?;
        } else {
            println!("Index {} not found", id);
        }
    } else {
        println!("Unknown command: {}", parts[0]);
    }
    Ok(())
}
