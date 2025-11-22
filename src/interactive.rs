use crate::db::BukuDb;
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
        "s" => {
            if parts.len() > 1 {
                let keywords: Vec<String> = parts[1..].iter().map(|s| s.to_string()).collect();
                let records = db.search(&keywords, true, false, false)?;
                for bookmark in records {
                    println!(
                        "{}. {}\n   > {}\n   + {}\n   # {}",
                        bookmark.id,
                        bookmark.title,
                        bookmark.url,
                        bookmark.description,
                        bookmark.tags
                    );
                }
            } else {
                println!("Search requires keywords");
            }
        }
        "S" => {
            if parts.len() > 1 {
                let keywords: Vec<String> = parts[1..].iter().map(|s| s.to_string()).collect();
                let records = db.search(&keywords, false, false, false)?;
                for bookmark in records {
                    println!(
                        "{}. {}\n   > {}\n   + {}\n   # {}",
                        bookmark.id,
                        bookmark.title,
                        bookmark.url,
                        bookmark.description,
                        bookmark.tags
                    );
                }
            } else {
                println!("Search requires keywords");
            }
        }
        "p" => {
            // Print by index/range
            // TODO: Implement range parsing
            println!("Print by index/range not fully implemented yet");
        }
        _ => {
            // Check if it's an index or range
            if let Ok(id) = parts[0].parse::<usize>() {
                // Open bookmark? Or print?
                // Buku opens by default if index is typed.
                if let Some(rec) = db.get_rec_by_id(id)? {
                    println!("Opening: {}", rec.url);
                    open::that(&rec.url)?;
                } else {
                    println!("Index {} not found", id);
                }
            } else {
                println!("Unknown command: {}", parts[0]);
            }
        }
    }
    Ok(())
}
