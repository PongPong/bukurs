use crate::db::BukuDb;
use crate::format::OutputFormat;
use crate::models::errors::AppError;
use crate::{browser, crypto, fetch, import_export, interactive, operations};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(author, version, about, long_about = None, disable_version_flag = true)]
pub struct Cli {
    /// Show the program version and exit
    #[arg(short = 'v', long = "version")]
    pub version: bool,

    /// Optional custom database file path
    #[arg(long)]
    pub db: Option<PathBuf>,

    /// Disable color output
    #[arg(long)]
    pub nc: bool,

    /// Show debug information
    #[arg(short = 'g', long = "debug")]
    pub debug: bool,

    #[arg(short = 'f', long)]
    pub format: Option<String>,

    /// Open selected bookmark in browser
    #[arg(short = 'o', long)]
    pub open: bool,

    /// Limit number of results shown (shows last N entries)
    #[arg(short = 'n', long)]
    pub limit: Option<usize>,

    /// Search keywords (when no subcommand is provided)
    #[arg(name = "KEYWORD")]
    pub keywords: Vec<String>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Add a new bookmark
    Add {
        /// URL to bookmark
        url: String,

        /// Comma-separated tags
        #[arg(short, long)]
        tag: Option<Vec<String>>,

        /// Bookmark title
        #[arg(long)]
        title: Option<String>,

        /// Notes or description
        #[arg(short, long)]
        comment: Option<String>,

        /// Add without connecting to web
        #[arg(long)]
        offline: bool,
    },

    /// Update an existing bookmark
    Update {
        /// Bookmark index to update
        id: usize,

        /// New URL
        #[arg(long)]
        url: Option<String>,

        /// New tags (comma-separated)
        #[arg(short, long)]
        tag: Option<Vec<String>>,

        /// New title
        #[arg(long)]
        title: Option<String>,

        /// New description
        #[arg(short, long)]
        comment: Option<String>,

        /// Disable web-fetch during auto-refresh
        #[arg(long)]
        immutable: Option<u8>,
    },

    /// Delete bookmark(s)
    Delete {
        /// Bookmark indices, ranges (e.g., 1-5), or * for all
        #[arg(num_args = 0..)]
        ids: Vec<String>,

        /// Skip confirmation prompt
        #[arg(short, long)]
        force: bool,

        /// Prevents reordering after deletion
        #[arg(long)]
        retain_order: bool,
    },

    /// Print/list bookmarks
    Print {
        /// Bookmark indices or ranges to print
        #[arg(num_args = 0..)]
        ids: Vec<String>,

        /// Bitwise column selection. Combine values to display multiple fields:
        ///    1  URL
        ///    2  Title
        ///    4  Tags
        ///    8  Description
        ///   16  ID
        /// Examples:
        ///    1         => URL only
        ///    5         => URL + Tags (1 | 4)
        ///    7         => URL + Title + Tags (1 | 2 | 4)
        #[arg(short, long)]
        columns: Option<u8>,

        /// JSON formatted output
        #[arg(short, long)]
        json: bool,
    },

    /// Search bookmarks
    Search {
        /// Search keywords
        keywords: Vec<String>,

        /// Match ALL keywords (default: ANY)
        #[arg(short = 'a', long)]
        all: bool,

        /// Match substrings
        #[arg(long)]
        deep: bool,

        /// Regex search
        #[arg(short, long)]
        regex: bool,

        /// Search for keywords in specific fields
        #[arg(long)]
        markers: bool,
    },

    /// Search bookmarks by tags
    Tag {
        /// Tag keywords to search
        #[arg(num_args = 0..)]
        tags: Vec<String>,
    },

    /// Encrypt database
    Lock {
        /// Number of hash iterations
        #[arg(default_value = "8")]
        iterations: u32,
    },

    /// Decrypt database
    Unlock {
        /// Number of hash iterations
        #[arg(default_value = "8")]
        iterations: u32,
    },

    /// Import bookmarks from file
    Import {
        /// File path to import from
        file: String,
    },

    /// Import bookmarks from browser profiles
    ImportBrowsers {
        /// List available browser profiles without importing
        #[arg(short, long)]
        list: bool,

        /// Import from all detected browsers
        #[arg(short, long)]
        all: bool,

        /// Specific browsers to import from (comma-separated: chrome,firefox,edge,safari)
        #[arg(short, long, value_delimiter = ',')]
        browsers: Option<Vec<String>>,
    },

    /// Export bookmarks to file
    Export {
        /// File path to export to
        file: String,
    },

    /// Open bookmark(s) in browser
    Open {
        /// Bookmark indices to open
        #[arg(num_args = 0..)]
        ids: Vec<String>,
    },

    /// Start interactive shell
    Shell,

    /// Edit bookmark in $EDITOR
    Edit {
        /// Bookmark ID to edit
        id: usize,
    },

    /// Undo last operation
    Undo,
}

pub fn handle_args(
    cli: Cli,
    db: &BukuDb,
    db_path: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    match cli.command {
        Some(Commands::Add {
            url,
            tag,
            title,
            comment,
            offline,
        }) => {
            let tags = tag.unwrap_or_default();

            if !offline {
                eprintln!("Fetching metadata for: {}", url);
            }

            let fetch_result = if !offline {
                fetch::fetch_data(&url).unwrap_or(fetch::FetchResult {
                    url: url.clone(),
                    title: "".to_string(),
                    desc: "".to_string(),
                    keywords: "".to_string(),
                })
            } else {
                fetch::FetchResult {
                    url: url.clone(),
                    title: "".to_string(),
                    desc: "".to_string(),
                    keywords: "".to_string(),
                }
            };

            let final_title = if let Some(t) = title {
                t
            } else if !fetch_result.title.is_empty() {
                fetch_result.title
            } else {
                url.clone()
            };

            let desc = comment.unwrap_or(fetch_result.desc);

            let tags_str = if tags.is_empty() {
                format!(",{},", fetch_result.keywords)
            } else {
                format!(",{},", tags.join(","))
            };

            match db.add_rec(&fetch_result.url, &final_title, &tags_str, &desc) {
                Ok(id) => {
                    eprintln!("Added bookmark at index {}", id);
                }
                Err(e) => {
                    if let rusqlite::Error::SqliteFailure(err, _) = &e {
                        // 2067 = SQLITE_CONSTRAINT_UNIQUE
                        if err.extended_code == 2067 {
                            eprintln!("Error: Another bookmark with this URL already exists");
                            eprintln!("URL: {}", url);
                            return Err(AppError::DuplicateUrl.into());
                        }
                    }

                    // For all other DB errors, return a generic one
                    return Err(AppError::DbError.into());
                }
            }
        }

        Some(Commands::Update {
            id,
            url,
            tag,
            title,
            comment,
            immutable,
        }) => {
            let url_ref = url.as_deref();
            let title_str = title.as_deref();
            let tags = tag.map(|v| v.join(","));
            let tags_ref = tags.as_deref();
            let desc_ref = comment.as_deref();

            match db.update_rec(id, url_ref, title_str, tags_ref, desc_ref, immutable) {
                Ok(()) => {
                    eprintln!("Updated bookmark at index {}", id);
                }
                Err(e) => {
                    if let rusqlite::Error::SqliteFailure(err, _) = &e {
                        if err.extended_code == 2067 {
                            // UNIQUE constraint
                            eprintln!("Error: Another bookmark with this URL already exists");
                            if let Some(new_url) = url {
                                eprintln!("URL: {}", new_url);
                            }
                        }
                    }
                    return Err(e.into());
                }
            }
        }

        Some(Commands::Delete {
            ids,
            force,
            retain_order: _,
        }) => {
            // Prepare delete operation (business logic)
            let operation = operations::prepare_delete(&ids, db)?;

            // Handle empty results
            if operation.bookmarks.is_empty() {
                match operation.mode {
                    operations::SelectionMode::ByKeywords(_) => {
                        eprintln!("No bookmarks found matching the search criteria.");
                    }
                    _ => {
                        eprintln!("No bookmarks to delete.");
                    }
                }
                return Ok(());
            }

            // Display bookmarks to be deleted (UI concern)
            match &operation.mode {
                operations::SelectionMode::All => {
                    eprintln!("⚠️  DELETE ALL BOOKMARKS:");
                }
                operations::SelectionMode::ByKeywords(keywords) => {
                    eprintln!("Searching for bookmarks matching: {:?}", keywords);
                    eprintln!("Bookmarks matching search criteria:");
                }
                operations::SelectionMode::ByIds(_) => {
                    eprintln!("Bookmarks to be deleted:");
                }
            }

            for bookmark in &operation.bookmarks {
                eprintln!("  {}. {} - {}", bookmark.id, bookmark.title, bookmark.url);
            }

            // Ask for confirmation unless --force is used (UI concern)
            let confirmed = if force {
                true
            } else {
                use std::io::{self, Write};

                let prompt = match operation.mode {
                    operations::SelectionMode::All => {
                        format!(
                            "\n⚠️  DELETE ALL {} bookmark(s)? [y/N]: ",
                            operation.bookmarks.len()
                        )
                    }
                    _ => {
                        format!(
                            "\nDelete {} bookmark(s)? [y/N]: ",
                            operation.bookmarks.len()
                        )
                    }
                };

                print!("{}", prompt);
                io::stdout().flush()?;

                let mut response = String::new();
                io::stdin().read_line(&mut response)?;
                let response = response.trim().to_lowercase();
                response == "y" || response == "yes"
            };

            if confirmed {
                // Execute deletion (business logic)
                let count = operations::execute_delete(&operation, db)?;
                eprintln!("Deleted {} bookmark(s).", count);
            } else {
                eprintln!("Deletion cancelled.");
            }
        }

        Some(Commands::Print {
            ids,
            columns: _,
            json: _,
        }) => {
            // Use the prepare_print operation (reuses DeleteMode logic)
            let operation = operations::prepare_print(&ids, db)?;

            // Handle empty results
            if operation.bookmarks.is_empty() {
                match operation.mode {
                    operations::SelectionMode::ByKeywords(_) => {
                        eprintln!("No bookmarks found matching the search criteria.");
                    }
                    _ => {
                        eprintln!("No bookmarks to display.");
                    }
                }
                return Ok(());
            }

            // Apply limit if specified
            let mut records = operation.bookmarks;
            if let Some(limit) = cli.limit {
                let start = records.len().saturating_sub(limit);
                records = records.into_iter().skip(start).collect();
            }

            let format: OutputFormat = cli
                .format
                .as_deref()
                .and_then(|s| Some(OutputFormat::from_string(s)))
                .unwrap_or(OutputFormat::Colored);

            format.print_bookmarks(&records, cli.nc);
        }

        Some(Commands::Search {
            keywords,
            all,
            deep,
            regex,
            markers: _,
        }) => {
            let any = !all;
            eprintln!("Searching for: {:?}", keywords);
            let mut records = db.search(&keywords, any, deep, regex)?;

            // Apply limit if specified
            if let Some(limit) = cli.limit {
                let start = records.len().saturating_sub(limit);
                records = records.into_iter().skip(start).collect();
            }

            let format: OutputFormat = cli
                .format
                .as_deref() // Option<&str>
                .and_then(|s| Some(OutputFormat::from_string(s)))
                .unwrap_or(OutputFormat::Colored); // default

            format.print_bookmarks(&records, cli.nc);
        }

        Some(Commands::Tag { tags }) => {
            if tags.is_empty() {
                eprintln!("Listing all tags (not implemented yet)");
            } else {
                eprintln!("Searching tags: {:?}", tags);
                let mut records = db.search_tags(&tags)?;
                if records.is_empty() {
                    eprintln!("No bookmarks found with the specified tags.");
                    return Ok(());
                }

                // Apply limit if specified
                if let Some(limit) = cli.limit {
                    let start = records.len().saturating_sub(limit);
                    records = records.into_iter().skip(start).collect();
                }

                let format: OutputFormat = cli
                    .format
                    .as_deref() // Option<&str>
                    .and_then(|s| Some(OutputFormat::from_string(s)))
                    .unwrap_or(OutputFormat::Colored); // default
                format.print_bookmarks(&records, cli.nc);
            }
        }

        Some(Commands::Lock { iterations }) => {
            let password = rpassword::prompt_password("Enter password: ")?;
            let confirm = rpassword::prompt_password("Confirm password: ")?;
            if password != confirm {
                return Err("Passwords do not match".into());
            }

            let enc_path = db_path.with_extension("db.enc");
            println!(
                "Encrypting {} to {} with {} iterations...",
                db_path.display(),
                enc_path.display(),
                iterations
            );
            crypto::BukuCrypt::encrypt_file(iterations, db_path, &enc_path, &password)?;
            eprintln!("Encryption complete.");
        }

        Some(Commands::Unlock { iterations }) => {
            let password = rpassword::prompt_password("Enter password: ")?;
            let enc_path = if db_path.extension().map_or(false, |ext| ext == "enc") {
                db_path.to_path_buf()
            } else {
                db_path.with_extension("db.enc")
            };

            let out_path = if enc_path.extension().map_or(false, |ext| ext == "enc") {
                enc_path.with_extension("")
            } else {
                enc_path.with_extension("db")
            };

            println!(
                "Decrypting {} to {} with {} iterations...",
                enc_path.display(),
                out_path.display(),
                iterations
            );
            crypto::BukuCrypt::decrypt_file(iterations, &out_path, &enc_path, &password)?;
            eprintln!("Decryption complete.");
        }

        Some(Commands::Import { file }) => {
            let count = import_export::import_bookmarks(db, &file)?;
            eprintln!(
                "✓ Successfully imported {} bookmark(s) from {}",
                count, file
            );
        }

        Some(Commands::ImportBrowsers {
            list,
            all,
            browsers,
        }) => {
            if list {
                // List detected browsers
                let profiles = import_export::list_detected_browsers();
                if profiles.is_empty() {
                    eprintln!("No browser profiles detected.");
                } else {
                    eprintln!("Detected browser profiles:");
                    for profile in profiles {
                        eprintln!("  • {}", profile.display_string());
                    }
                }
            } else if all {
                // Import from all detected browsers
                eprintln!("Importing from all detected browsers...");
                match import_export::auto_import_all(db) {
                    Ok(count) => {
                        eprintln!("✓ Successfully imported {} total bookmark(s)", count);
                    }
                    Err(e) => {
                        eprintln!("Error during import: {}", e);
                        return Err(e);
                    }
                }
            } else if let Some(browser_list) = browsers {
                // Import from specific browsers
                eprintln!("Importing from selected browsers: {:?}", browser_list);
                match import_export::import_from_selected_browsers(db, &browser_list) {
                    Ok(count) => {
                        eprintln!("✓ Successfully imported {} total bookmark(s)", count);
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        return Err(e);
                    }
                }
            } else {
                eprintln!("Error: Please specify --list, --all, or --browsers");
                eprintln!("Examples:");
                eprintln!("  bukurs import-browsers --list");
                eprintln!("  bukurs import-browsers --all");
                eprintln!("  bukurs import-browsers --browsers chrome,firefox");
                return Err("No import option specified".into());
            }
        }

        Some(Commands::Export { file }) => {
            import_export::export_bookmarks(db, &file)?;
            eprintln!("Exported bookmarks to {}", file);
        }

        Some(Commands::Open { ids }) => {
            if ids.is_empty() {
                eprintln!("Opening random bookmark (not implemented yet)");
            } else {
                for arg in ids {
                    if let Ok(id) = arg.parse::<usize>() {
                        if let Some(rec) = db.get_rec_by_id(id)? {
                            eprintln!("Opening: {}", rec.url);
                            browser::open_url(&rec.url)?;
                        } else {
                            eprintln!("Index {} not found", id);
                        }
                    } else {
                        eprintln!("Invalid index: {}", arg);
                    }
                }
            }
        }

        Some(Commands::Shell) => {
            // let records = db.get_rec_all()?;
            interactive::run(db)?;
        }

        Some(Commands::Edit { id }) => {
            // Fetch the bookmark
            let bookmark = db
                .get_rec_by_id(id)?
                .ok_or_else(|| format!("Bookmark {} not found", id))?;

            eprintln!("Opening bookmark #{} in editor...", id);

            // Edit the bookmark
            match crate::editor::edit_bookmark(&bookmark) {
                Ok(edited) => {
                    // Update the database
                    match db.update_rec(
                        id,
                        Some(&edited.url),
                        Some(&edited.title),
                        Some(&edited.tags),
                        Some(&edited.description),
                        None,
                    ) {
                        Ok(()) => {
                            eprintln!("Bookmark {} updated successfully", id);
                        }
                        Err(e) => {
                            if let rusqlite::Error::SqliteFailure(err, _) = &e {
                                if err.extended_code == 2067 {
                                    // UNIQUE constraint failed
                                    eprintln!(
                                        "Error: Another bookmark with this URL already exists"
                                    );
                                    eprintln!("URL: {}", edited.url);
                                    return Err(AppError::DuplicateUrl.into());
                                }
                            }
                            return Err(AppError::DbError.into());
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Edit cancelled or failed: {}", e);
                }
            }
        }

        Some(Commands::Undo) => {
            if let Some(op) = db.undo_last()? {
                eprintln!("Undid last operation: {}", op);
            } else {
                eprintln!("Nothing to undo.");
            }
        }

        None => {
            // No subcommand provided, Search with keywords
            eprintln!("Searching for: {:?}", cli.keywords);
            // Fuzzy search with keywords
            let query = if !cli.keywords.is_empty() {
                Some(cli.keywords.join(" "))
            } else {
                None
            };

            let records = db.get_rec_all()?;
            if let Some(selected) = crate::fuzzy::run_fuzzy_search(&records, query)? {
                if cli.open {
                    eprintln!("Opening: {}", selected.url);
                    browser::open_url(&selected.url)?;
                } else {
                    let format: OutputFormat = cli
                        .format
                        .as_deref() // Option<&str>
                        .and_then(|s| Some(OutputFormat::from_string(s)))
                        .unwrap_or(OutputFormat::Colored); // default
                                                           // Display selected bookmark
                    let selected = vec![selected];
                    format.print_bookmarks(&selected, cli.nc);
                }
            }
        }
    }

    Ok(())
}
