use bukurs::db::BukuDb;
use bukurs::error::Result;
use bukurs::config::Config;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use crate::commands::{AppContext, BukuCommand};
use crate::commands::add::AddCommand;
use crate::commands::update::UpdateCommand;
use crate::commands::delete::DeleteCommand;
use crate::commands::search::SearchCommand;
use crate::commands::tag::TagCommand;
use crate::commands::misc::{NoCommand, OpenCommand, UndoCommand};
use crate::commands::print::PrintCommand;
use crate::commands::import_export::{ImportCommand, ExportCommand, ImportBrowsersCommand};
use crate::commands::lock_unlock::{LockCommand, UnlockCommand};

pub fn run_with_context(ctx: &AppContext) -> Result<()> {
    let mut rl =
        DefaultEditor::new().map_err(|e| bukurs::error::BukursError::Other(e.to_string()))?;

    println!("bukurs interactive mode - type '?' for help");

    loop {
        let readline = rl.readline("buku> ");
        match readline {
            Ok(line) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                rl.add_history_entry(line)
                    .map_err(|e| bukurs::error::BukursError::Other(e.to_string()))?;

                match line {
                    "q" | "quit" | "exit" => break,
                    "?" | "help" => print_help(),
                    _ => {
                        if let Err(e) = handle_command(ctx, line) {
                            eprintln!("Error: {}", e);
                        }
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("^C");
                break;
            }
            Err(ReadlineError::Eof) => {
                println!("^D");
                break;
            }
            Err(err) => {
                eprintln!("Error: {:?}", err);
                break;
            }
        }
    }
    Ok(())
}

// Legacy entry point - creates a default context
pub fn run(db: &BukuDb) -> Result<()> {
    let config = Config::default();
    let db_path = std::path::PathBuf::from("bookmarks.db");
    let ctx = AppContext {
        db,
        config: &config,
        db_path: &db_path,
    };
    run_with_context(&ctx)
}

fn print_help() {
    println!(
        "
INTERACTIVE MODE COMMANDS (All CLI features supported!):

SEARCH & BROWSE:
    s [keywords...]        Search bookmarks with ANY keyword (fuzzy picker)
    S [keywords...]        Search bookmarks with ALL keywords (fuzzy picker)
    t [tags...]            Search by tags (or fuzzy pick if no tags given)
    [number]               Open bookmark by ID in browser
    ls                     List all bookmarks (fuzzy picker)

ADD & MODIFY:
    a <url> [tags] [title] [comment]
                           Add new bookmark
                           Example: a https://rust-lang.org rust,programming \"Rust\" \"Official\"
    
    u <id> [options]       Update bookmark
                           Options: --url <url> -t tag1,tag2 --title \"Title\" -c \"Comment\"
                           Example: u 5 -t +urgent
                           Example: u 5 --url https://new-url.com
    
    e <id>                 Edit bookmark in $EDITOR

DELETE:
    d <id|range> [-f]      Delete bookmark(s)
                           Examples: d 5, d 1-10, d 5 -f (force, no confirm)

PRINT:
    p <id|range>           Print bookmarks
                           Examples: p 5, p 1-10, p *

IMPORT/EXPORT:
    import <file>          Import bookmarks from HTML/JSON file
    export <file>          Export bookmarks to HTML file
    import-browsers [-l] [-a]
                           Import from browsers (-l: list, -a: all)

OPEN:
    open <id>              Open bookmark in browser
    o <id>                 Alias for 'open'
    <id>                   Direct shorthand (just type the number)

DATABASE:
    lock [iter]            Encrypt database (default: 8 iterations)
    unlock [iter]          Decrypt database (default: 8 iterations)
    undo [count]           Undo last operation(s) (default: 1)
    
HELP & EXIT:
    ?  or help             Show this help
    q  or quit or exit     Exit interactive mode
    ^D or ^C               Exit interactive mode

EXAMPLES:
    s rust programming     # Search ANY keyword and fuzzy pick
    S rust error           # Search ALL keywords and fuzzy pick
    t                      # Fuzzy pick from all tags
    t rust                 # Search by tag and fuzzy pick
    ls                     # List all and fuzzy pick
    
    a https://rust-lang.org rust \"Rust\" \"Programming language\"
    u 5 -t +urgent,-todo   # Add 'urgent', remove 'todo' tag
    u 5 --title \"New Title\"
    e 5                    # Edit in $EDITOR
    d 10                   # Delete with confirmation
    d 1-5 -f               # Delete range without confirmation
    
    p 1-10                 # Print bookmarks 1-10
    p *                    # Print all bookmarks
    
    import bookmarks.html  # Import from file
    export backup.html     # Export to file
    import-browsers -l     # List browser profiles
    import-browsers -a     # Import from all browsers
    
    open 5                 # Open bookmark 5 in browser
    5                      # Shorthand for open
    
    lock                   # Encrypt database
    unlock                 # Decrypt database
    undo 3                 # Undo last 3 operations

TIP: All commands reuse the exact same code as CLI mode for consistency!
"
    );
}

fn handle_command(ctx: &AppContext, line: &str) -> Result<()> {
    // Parse the command line using shell-like parsing
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.is_empty() {
        return Ok(());
    }

    let cmd = parts[0];
    let args = &parts[1..];
    
    match cmd {
        // Search commands - reuse existing command structures
        "s" => {
            let keywords: Vec<String> = args.iter().map(|s| s.to_string()).collect();
            if keywords.is_empty() {
                println!("Usage: s keyword [...]");
                return Ok(());
            }
            let command = SearchCommand {
                keywords,
                all: false,  // ANY
                deep: false,
                regex: false,
                limit: None,
                format: None,
                nc: false,
                open: false,
            };
            command.execute(ctx)
        }
        "S" => {
            let keywords: Vec<String> = args.iter().map(|s| s.to_string()).collect();
            if keywords.is_empty() {
                println!("Usage: S keyword [...]");
                return Ok(());
            }
            let command = SearchCommand {
                keywords,
                all: true,  // ALL
                deep: false,
                regex: false,
                limit: None,
                format: None,
                nc: false,
                open: false,
            };
            command.execute(ctx)
        }
        "t" | "tag" => {
            let tags: Vec<String> = args.iter().map(|s| s.to_string()).collect();
            let command = TagCommand {
                tags,
                limit: None,
                format: None,
                nc: false,
                open: false,
            };
            command.execute(ctx)
        }
        "ls" | "list" => {
            let command = NoCommand {
                keywords: vec![],
                open: false,
                format: None,
                nc: false,
            };
            command.execute(ctx)
        }
        
        // Add - simple parsing
        "a" | "add" => {
            if args.is_empty() {
                println!("Usage: a <url> [tags] [title] [comment]");
                println!("Example: a https://rust-lang.org rust,programming \"Rust\" \"Rust official site\"");
                return Ok(());
            }
            
            let url = args[0].to_string();
            let tags = if args.len() > 1 {
                Some(vec![args[1].to_string()])
            } else {
                None
            };
            let title = if args.len() > 2 {
                Some(args[2].to_string())
            } else {
                None
            };
            let comment = if args.len() > 3 {
                Some(args[3].to_string())
            } else {
                None
            };
            
            let command = AddCommand {
                url,
                tag: tags,
                title,
                comment,
                offline: false,
            };
            command.execute(ctx)
        }
        
        // Update - simplified parsing
        "u" | "update" => {
            if args.is_empty() {
                println!("Usage: u <id> [--url <url>] [-t tag1,tag2] [--title <title>] [-c <comment>]");
                println!("Example: u 5 -t +urgent");
                println!("Example: u 5 --url https://new-url.com");
                println!("Note: For complex updates, use 'e <id>' to edit in $EDITOR");
                return Ok(());
            }
            
            let id_str = args[0].to_string();
            let ids = vec![id_str];
            
            // Simple argument parsing
            let mut url = None;
            let mut tag = None;
            let mut title = None;
            let mut comment = None;
            
            let mut i = 1;
            while i < args.len() {
                match args[i] {
                    "--url" if i + 1 < args.len() => {
                        url = Some(args[i + 1].to_string());
                        i += 2;
                    }
                    "-t" if i + 1 < args.len() => {
                        tag = Some(vec![args[i + 1].to_string()]);
                        i += 2;
                    }
                    "--title" if i + 1 < args.len() => {
                        title = Some(args[i + 1].to_string());
                        i += 2;
                    }
                    "-c" if i + 1 < args.len() => {
                        comment = Some(args[i + 1].to_string());
                        i += 2;
                    }
                    _ => {
                        println!("Unknown option: {}", args[i]);
                        i += 1;
                    }
                }
            }
            
            let command = UpdateCommand {
                ids,
                url,
                tag,
                title,
                comment,
                immutable: None,
            };
            command.execute(ctx)
        }
        
        // Delete
        "d" | "delete" | "del" => {
            let ids: Vec<String> = args.iter().map(|s| s.to_string()).collect();
            if ids.is_empty() {
                println!("Usage: d <id|range> [-f]");
                println!("Example: d 5");
                println!("Example: d 1-10 -f");
                return Ok(());
            }
            
            let force = ids.contains(&"-f".to_string());
            let ids: Vec<String> = ids.into_iter().filter(|s| s != "-f").collect();
            
            let command = DeleteCommand {
                ids,
                force,
            };
            command.execute(ctx)
        }
        
        // Edit
        "e" | "edit" => handle_edit_interactive(ctx, args),
        
        // Print
        "p" | "print" => {
            let ids: Vec<String> = args.iter().map(|s| s.to_string()).collect();
            if ids.is_empty() {
                println!("Usage: p <id|range>");
                println!("Example: p 5");
                println!("Example: p 1-10");
                println!("Example: p *");
                return Ok(());
            }
            
            let command = PrintCommand {
                ids,
                limit: None,
                format: None,
                nc: false,
            };
            command.execute(ctx)
        }
        
        // Import/Export
        "import" => {
            if args.is_empty() {
                println!("Usage: import <file>");
                println!("Example: import bookmarks.html");
                return Ok(());
            }
            
            let command = ImportCommand {
                file: args[0].to_string(),
            };
            command.execute(ctx)
        }
        
        "export" => {
            if args.is_empty() {
                println!("Usage: export <file>");
                println!("Example: export bookmarks.html");
                return Ok(());
            }
            
            let command = ExportCommand {
                file: args[0].to_string(),
            };
            command.execute(ctx)
        }
        
        "import-browsers" => {
            let list = args.contains(&"-l");
            let all = args.contains(&"-a");
            let browsers = None; // Simplified - could parse -b flag
            
            let command = ImportBrowsersCommand {
                list,
                all,
                browsers,
            };
            command.execute(ctx)
        }
        
        // Open
        "open" | "o" => {
            let ids: Vec<String> = args.iter().map(|s| s.to_string()).collect();
            if ids.is_empty() {
                println!("Usage: open <id>");
                println!("Example: open 5");
                return Ok(());
            }
            
            let command = OpenCommand { ids };
            command.execute(ctx)
        }
        
        // Lock
        "lock" => {
            let iterations = if args.is_empty() {
                8
            } else {
                args[0].parse::<u32>().unwrap_or(8)
            };
            
            let command = LockCommand { iterations };
            command.execute(ctx)
        }
        
        // Unlock
        "unlock" => {
            let iterations = if args.is_empty() {
                8
            } else {
                args[0].parse::<u32>().unwrap_or(8)
            };
            
            let command = UnlockCommand { iterations };
            command.execute(ctx)
        }
        
        // Undo
        "undo" => {
            let count = if args.is_empty() {
                1
            } else {
                args[0].parse::<usize>().unwrap_or(1)
            };
            
            let command = UndoCommand { count };
            command.execute(ctx)
        }
        
        // Try to parse as ID
        _ => handle_open_by_id(ctx.db, cmd),
    }
}

// Edit handler (still needs special handling for editor interaction)
fn handle_edit_interactive(ctx: &AppContext, args: &[&str]) -> Result<()> {
    if args.is_empty() {
        println!("Usage: e <bookmark_id>");
        println!("Example: e 5");
        return Ok(());
    }

    let bookmark_id = match args[0].parse::<usize>() {
        Ok(id) => id,
        Err(_) => {
            println!("Invalid bookmark ID: {}", args[0]);
            return Ok(());
        }
    };

    let bookmark = match ctx.db.get_rec_by_id(bookmark_id)? {
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
    match ctx.db.update_rec_partial(
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

// Open by ID (when command is just a number)
fn handle_open_by_id(db: &BukuDb, cmd: &str) -> Result<()> {
    if let Ok(id) = cmd.parse::<usize>() {
        if let Some(rec) = db.get_rec_by_id(id)? {
            println!("Opening: {}", rec.url);
            bukurs::browser::open_url(&rec.url)?;
        } else {
            println!("Bookmark {} not found", id);
        }
    } else {
        println!("Unknown command: {}. Type '?' for help", cmd);
    }
    Ok(())
}
