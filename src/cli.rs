use crate::db::BukuDb;
use crate::format::json::JsonBookmark;
use crate::format::toml::TomlBookmark;
use crate::format::toon::ToonBookmark;
use crate::format::traits::BookmarkFormat;
use crate::format::yaml::YamlBookmark;
use crate::format::OutputFormat;
use crate::output::colorize::{Colorize, ColorizeBookmark};
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

        /// Limit fields in output
        #[arg(short, long)]
        format: Option<String>,

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

    /// Start interactive mode
    Interactive,
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
                println!("Fetching metadata for: {}", url);
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

            let id = db.add_rec(&fetch_result.url, &final_title, &tags_str, &desc)?;
            println!("Added bookmark at index {}", id);
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

            db.update_rec(id, url_ref, title_str, tags_ref, desc_ref, immutable)?;
            println!("Updated bookmark at index {}", id);
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
                    operations::DeleteMode::ByKeywords(_) => {
                        println!("No bookmarks found matching the search criteria.");
                    }
                    _ => {
                        println!("No bookmarks to delete.");
                    }
                }
                return Ok(());
            }

            // Display bookmarks to be deleted (UI concern)
            match &operation.mode {
                operations::DeleteMode::All => {
                    println!("⚠️  DELETE ALL BOOKMARKS:");
                }
                operations::DeleteMode::ByKeywords(keywords) => {
                    println!("Searching for bookmarks matching: {:?}", keywords);
                    println!("Bookmarks matching search criteria:");
                }
                operations::DeleteMode::ByIds(_) => {
                    println!("Bookmarks to be deleted:");
                }
            }

            for bookmark in &operation.bookmarks {
                println!("  {}. {} - {}", bookmark.id, bookmark.title, bookmark.url);
            }

            // Ask for confirmation unless --force is used (UI concern)
            let confirmed = if force {
                true
            } else {
                use std::io::{self, Write};

                let prompt = match operation.mode {
                    operations::DeleteMode::All => {
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
                println!("Deleted {} bookmark(s).", count);
            } else {
                println!("Deletion cancelled.");
            }
        }

        Some(Commands::Print {
            ids: _,
            format,
            columns: _,
            json: _,
        }) => {
            let records = db.get_rec_all()?;
            let format: OutputFormat = format
                .as_deref() // Option<&str>
                .and_then(|s| OutputFormat::from_str(s)) // Option<OutputFormat>
                .unwrap_or(OutputFormat::Colored); // default

            format.print_bookmarks(&records);
        }

        Some(Commands::Search {
            keywords,
            all,
            deep,
            regex,
            markers: _,
        }) => {
            let any = !all;
            println!("Searching for: {:?}", keywords);
            let records = db.search(&keywords, any, deep, regex)?;
            for bookmark in records {
                println!(
                    "{}. {}\n   > {}\n   + {}\n   # {}",
                    bookmark.id, bookmark.title, bookmark.url, bookmark.description, bookmark.tags
                );
            }
        }

        Some(Commands::Tag { tags }) => {
            if tags.is_empty() {
                println!("Listing all tags (not implemented yet)");
            } else {
                println!("Searching tags: {:?}", tags);
                let records = db.search_tags(&tags)?;
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
            println!("Encryption complete.");
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
            println!("Decryption complete.");
        }

        Some(Commands::Import { file }) => {
            import_export::import_bookmarks(db, &file)?;
            println!("Imported bookmarks from {}", file);
        }

        Some(Commands::Export { file }) => {
            import_export::export_bookmarks(db, &file)?;
            println!("Exported bookmarks to {}", file);
        }

        Some(Commands::Open { ids }) => {
            if ids.is_empty() {
                println!("Opening random bookmark (not implemented yet)");
            } else {
                for arg in ids {
                    if let Ok(id) = arg.parse::<usize>() {
                        if let Some(rec) = db.get_rec_by_id(id)? {
                            println!("Opening: {}", rec.url);
                            browser::open_url(&rec.url)?;
                        } else {
                            println!("Index {} not found", id);
                        }
                    } else {
                        println!("Invalid index: {}", arg);
                    }
                }
            }
        }

        Some(Commands::Interactive) => {
            // let records = db.get_rec_all()?;
            interactive::run(db)?;
        }

        None => {
            // No subcommand provided
            if !cli.keywords.is_empty() {
                // Search with keywords
                println!("Searching for: {:?}", cli.keywords);
                let records = db.search(&cli.keywords, true, false, false)?;
                let format: OutputFormat = cli
                    .format
                    .as_deref() // Option<&str>
                    .and_then(|s| OutputFormat::from_str(s)) // Option<OutputFormat>
                    .unwrap_or(OutputFormat::Colored); // default
                format.print_bookmarks(&records);
            } else {
                // Interactive mode
                let records = db.get_rec_all()?;
                let format: OutputFormat = cli
                    .format
                    .as_deref() // Option<&str>
                    .and_then(|s| OutputFormat::from_str(s)) // Option<OutputFormat>
                    .unwrap_or(OutputFormat::Colored); // default
                format.print_bookmarks(&records);
                interactive::run(db)?;
            }
        }
    }

    Ok(())
}
