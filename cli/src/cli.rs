use crate::format::OutputFormat;
use crate::interactive;
use bukurs::db::BukuDb;
use bukurs::models::errors::AppError;
use bukurs::{browser, crypto, fetch, import_export, operations};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::sync::OnceLock;

fn get_exe_name() -> &'static str {
    static EXE_NAME: OnceLock<String> = OnceLock::new();
    EXE_NAME.get_or_init(|| {
        std::env::args()
            .next()
            .as_ref()
            .map(std::path::Path::new)
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "bukurs".to_string())
    })
}

#[derive(Parser)]
#[command(author, version, about, long_about = None, disable_version_flag = true)]
pub struct Cli {
    /// Show the program version and exit
    #[arg(short = 'v', long = "version")]
    pub version: bool,

    /// Optional custom database file path
    #[arg(long)]
    pub db: Option<PathBuf>,

    /// Optional custom configuration file path
    #[arg(long)]
    pub config: Option<PathBuf>,

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
        /// Bookmark indices, ranges (e.g., 1-5), or * for all
        /// When no edit options are provided, refreshes metadata from web
        #[arg(num_args = 0..)]
        ids: Vec<String>,

        /// New URL
        #[arg(long)]
        url: Option<String>,

        /// Tag operations (supports: +add, -remove, ~old:new, or plain tag to add)
        /// Examples: +urgent, -archived, ~todo:done
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
        /// Bookmark ID to edit (if not provided, creates a new bookmark)
        id: Option<usize>,
    },

    /// Undo last operation(s)
    Undo {
        /// Number of operations to undo (default: 1)
        #[arg(default_value = "1")]
        count: usize,
    },
}

pub fn handle_args(
    cli: Cli,
    db: &BukuDb,
    db_path: &std::path::Path,
    config: &bukurs::config::Config,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::fetch_ui::fetch_with_spinner;

    match cli.command {
        Some(Commands::Add {
            url,
            tag,
            title,
            comment,
            offline,
        }) => {
            let tags = tag.unwrap_or_default();
            for t in &tags {
                if t.contains(' ') {
                    return Err(format!(
                        "Tag '{}' contains spaces. Tags cannot contain spaces.",
                        t
                    )
                    .into());
                }
            }

            let fetch_result = if !offline {
                match fetch_with_spinner(&url, &config.user_agent) {
                    Ok(result) => result,
                    Err(e) => {
                        eprintln!("Warning: Failed to fetch metadata: {}", e);
                        eprintln!("Continuing with manual entry...");
                        fetch::FetchResult {
                            url: url.clone(),
                            title: "".to_string(),
                            desc: "".to_string(),
                            keywords: "".to_string(),
                        }
                    }
                }
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
                            return Err(AppError::DuplicateUrl(url.clone()).into());
                        }
                    }

                    // For all other DB errors, return a generic one
                    return Err(AppError::DbError.into());
                }
            }
        }

        Some(Commands::Update {
            ids,
            url,
            tag,
            title,
            comment,
            immutable,
        }) => {
            use crate::tag_ops::{apply_tag_operations, parse_tag_operations};

            // Check if any edit options are provided
            let has_edit_options = url.is_some()
                || tag.is_some()
                || title.is_some()
                || comment.is_some()
                || immutable.is_some();

            if ids.is_empty() {
                eprintln!("Usage: {} update <ID|RANGE|*> [OPTIONS]", get_exe_name());
                eprintln!("Examples:");
                eprintln!(
                    "  {} update 5                  # Refresh metadata for bookmark 5",
                    get_exe_name()
                );
                eprintln!(
                    "  {} update 1-10               # Refresh metadata for bookmarks 1-10",
                    get_exe_name()
                );
                eprintln!(
                    "  {} update \"*\"                # Refresh all bookmarks",
                    get_exe_name()
                );
                eprintln!(
                    "  {} update 5 --tag +urgent    # Add 'urgent' tag",
                    get_exe_name()
                );
                eprintln!(
                    "  {} update 5 --tag -archived  # Remove 'archived' tag",
                    get_exe_name()
                );
                eprintln!(
                    "  {} update 5 --tag ~todo:done # Replace 'todo' with 'done'",
                    get_exe_name()
                );
                return Err("No bookmark IDs specified".into());
            }

            if has_edit_options {
                // Field update mode: update specific fields for bookmark(s)
                // Parse IDs/ranges
                let operation = operations::prepare_print(&ids, db)?;
                let bookmarks = operation.bookmarks;

                if bookmarks.is_empty() {
                    eprintln!("No bookmarks found");
                    return Ok(());
                }

                let url_ref = url.as_deref();
                let title_str = title.as_deref();
                let desc_ref = comment.as_deref();

                // Parse tag operations if provided
                let tag_operations = tag.as_ref().map(|tags| parse_tag_operations(tags));

                if bookmarks.len() > 1 {
                    // Batch update mode: use update_rec_batch for atomicity and single undo

                    // For batch mode with tag operations, we need to compute per-bookmark tags
                    // but we can only pass a single tags value to update_rec_batch
                    // So we'll handle tag operations differently for batch vs single

                    if tag_operations.is_some() {
                        // When tag operations are present, we need per-bookmark computation
                        // Fall back to individual updates but wrapped in explicit batch logging
                        let mut success_count = 0;
                        let mut failed_count = 0;

                        for bookmark in &bookmarks {
                            // Apply tag operations to existing tags
                            let final_tags = if let Some(ref ops) = tag_operations {
                                let new_tags = apply_tag_operations(&bookmark.tags, ops);
                                Some(new_tags)
                            } else {
                                None
                            };

                            let tags_ref = final_tags.as_deref();

                            match db.update_rec(
                                bookmark.id,
                                url_ref,
                                title_str,
                                tags_ref,
                                desc_ref,
                                immutable,
                            ) {
                                Ok(()) => {
                                    success_count += 1;
                                    eprintln!("✓ Updated bookmark {}", bookmark.id);
                                }
                                Err(e) => {
                                    failed_count += 1;
                                    if let rusqlite::Error::SqliteFailure(err, _) = &e {
                                        if err.extended_code == 2067 {
                                            eprintln!(
                                                "✗ Bookmark {}: URL already exists",
                                                bookmark.id
                                            );
                                        } else {
                                            eprintln!("✗ Bookmark {}: {}", bookmark.id, e);
                                        }
                                    } else {
                                        eprintln!("✗ Bookmark {}: {}", bookmark.id, e);
                                    }
                                }
                            }
                        }

                        eprintln!();
                        if success_count > 0 {
                            eprintln!("✓ Successfully updated {} bookmark(s)", success_count);
                        }
                        if failed_count > 0 {
                            eprintln!("✗ Failed to update {} bookmark(s)", failed_count);
                        }
                    } else {
                        // No tag operations - can use efficient batch update
                        match db.update_rec_batch(
                            &bookmarks, url_ref, title_str, None, desc_ref, immutable,
                        ) {
                            Ok((success_count, _failed_count)) => {
                                eprintln!();
                                eprintln!(
                                    "✓ Successfully updated {} bookmark(s) in batch",
                                    success_count
                                );
                            }
                            Err(e) => {
                                eprintln!("✗ Batch update failed: {}", e);
                                eprintln!("All changes have been rolled back.");
                            }
                        }
                    }
                } else {
                    // Single bookmark update - use original logic
                    let bookmark = &bookmarks[0];

                    // Apply tag operations to existing tags
                    let final_tags = if let Some(ref ops) = tag_operations {
                        let new_tags = apply_tag_operations(&bookmark.tags, ops);
                        Some(new_tags)
                    } else {
                        None
                    };

                    let tags_ref = final_tags.as_deref();

                    match db.update_rec(
                        bookmark.id,
                        url_ref,
                        title_str,
                        tags_ref,
                        desc_ref,
                        immutable,
                    ) {
                        Ok(()) => {
                            eprintln!("✓ Updated bookmark {}", bookmark.id);
                        }
                        Err(e) => {
                            if let rusqlite::Error::SqliteFailure(err, _) = &e {
                                if err.extended_code == 2067 {
                                    eprintln!("✗ Bookmark {}: URL already exists", bookmark.id);
                                } else {
                                    eprintln!("✗ Bookmark {}: {}", bookmark.id, e);
                                }
                            } else {
                                eprintln!("✗ Bookmark {}: {}", bookmark.id, e);
                            }
                        }
                    }
                }
            } else {
                // New behavior: refresh metadata from web
                use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

                // Parse IDs/ranges
                let operation = operations::prepare_print(&ids, db)?;
                let bookmarks = operation.bookmarks;

                if bookmarks.is_empty() {
                    eprintln!("No bookmarks found");
                    return Ok(());
                }

                eprintln!("Refreshing metadata for {} bookmark(s)...", bookmarks.len());

                // Create multi-progress for overall + per-URL progress
                let multi = MultiProgress::new();

                // Overall progress bar
                let pb = multi.add(ProgressBar::new(bookmarks.len() as u64));
                pb.set_style(
                    ProgressStyle::default_bar()
                        .template("{msg} [{bar:40.cyan/blue}] {pos}/{len}")
                        .unwrap()
                        .progress_chars("=>-"),
                );
                pb.set_message("Overall progress");

                let mut success_count = 0;
                let mut failed_count = 0;
                let mut failed_ids: Vec<usize> = Vec::new();

                for bookmark in &bookmarks {
                    // Fetch metadata using helper
                    match fetch_with_spinner(&bookmark.url, &config.user_agent) {
                        Ok(fetch_result) => {
                            // Update bookmark with fetched metadata
                            let new_title = if !fetch_result.title.is_empty() {
                                Some(fetch_result.title.as_str())
                            } else {
                                None
                            };

                            let new_desc = if !fetch_result.desc.is_empty() {
                                Some(fetch_result.desc.as_str())
                            } else {
                                None
                            };

                            // Keep existing tags, only update title and description
                            match db.update_rec(
                                bookmark.id,
                                None, // Don't change URL
                                new_title,
                                None, // Don't change tags
                                new_desc,
                                None, // Don't change immutable flag
                            ) {
                                Ok(()) => success_count += 1,
                                Err(_) => {
                                    failed_count += 1;
                                    failed_ids.push(bookmark.id);
                                }
                            }
                        }
                        Err(_) => {
                            failed_count += 1;
                            failed_ids.push(bookmark.id);
                        }
                    }
                    pb.inc(1);
                }

                pb.finish_and_clear();

                // Display summary
                if success_count > 0 {
                    eprintln!("✓ Successfully refreshed {} bookmark(s)", success_count);
                }
                if failed_count > 0 {
                    eprintln!("✗ Failed to refresh {} bookmark(s)", failed_count);
                    eprintln!(
                        "   Failed IDs: {}",
                        failed_ids
                            .iter()
                            .map(|id| id.to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                    eprintln!(
                        "   To retry: {} update {}",
                        get_exe_name(),
                        failed_ids
                            .iter()
                            .map(|id| id.to_string())
                            .collect::<Vec<_>>()
                            .join(" ")
                    );
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

        Some(Commands::Print { ids, columns: _ }) => {
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
                eprintln!("  {} import-browsers --list", get_exe_name());
                eprintln!("  {} import-browsers --all", get_exe_name());
                eprintln!(
                    "  {} import-browsers --browsers chrome,firefox",
                    get_exe_name()
                );
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
            match id {
                Some(bookmark_id) => {
                    // Edit existing bookmark
                    let bookmark = db
                        .get_rec_by_id(bookmark_id)?
                        .ok_or_else(|| format!("Bookmark {} not found", bookmark_id))?;

                    eprintln!("Opening bookmark #{} in editor...", bookmark_id);

                    match crate::editor::edit_bookmark(&bookmark) {
                        Ok(edited) => {
                            // Update the database
                            match db.update_rec(
                                bookmark_id,
                                Some(&edited.url),
                                Some(&edited.title),
                                Some(&edited.tags),
                                Some(&edited.description),
                                None,
                            ) {
                                Ok(()) => {
                                    eprintln!("Bookmark {} updated successfully", bookmark_id);
                                }
                                Err(e) => {
                                    if let rusqlite::Error::SqliteFailure(err, _) = &e {
                                        if err.extended_code == 2067 {
                                            // UNIQUE constraint failed
                                            return Err(
                                                AppError::DuplicateUrl(edited.url.clone()).into()
                                            );
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
                None => {
                    // Create new bookmark from template
                    eprintln!("Opening editor to create new bookmark...");

                    match crate::editor::edit_new_bookmark() {
                        Ok(new_bookmark) => {
                            // Add to database
                            match db.add_rec(
                                &new_bookmark.url,
                                &new_bookmark.title,
                                &new_bookmark.tags,
                                &new_bookmark.description,
                            ) {
                                Ok(id) => {
                                    eprintln!("✓ Created new bookmark at index {}", id);
                                }
                                Err(e) => {
                                    if let rusqlite::Error::SqliteFailure(err, _) = &e {
                                        if err.extended_code == 2067 {
                                            return Err(AppError::DuplicateUrl(
                                                new_bookmark.url.clone(),
                                            )
                                            .into());
                                        }
                                    }
                                    return Err(AppError::DbError.into());
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Creation cancelled or failed: {}", e);
                        }
                    }
                }
            }
        }

        Some(Commands::Undo { count }) => {
            if count == 0 {
                eprintln!("Error: Count must be at least 1");
                return Err("Invalid count".into());
            }

            let mut undone_count = 0;
            let mut operations = Vec::new();

            for i in 0..count {
                match db.undo_last()? {
                    Some((op_type, affected)) => {
                        undone_count += 1;
                        operations.push((op_type, affected));
                    }
                    None => {
                        if i == 0 {
                            eprintln!("Nothing to undo.");
                        } else {
                            eprintln!(
                                "No more operations to undo (undid {} operation(s)).",
                                undone_count
                            );
                        }
                        break;
                    }
                }
            }

            if undone_count > 0 {
                if undone_count == 1 {
                    let (op_type, affected) = &operations[0];
                    if *affected > 1 {
                        eprintln!(
                            "✓ Undid batch {}: {} bookmark(s) reverted",
                            op_type, affected
                        );
                    } else {
                        eprintln!("✓ Undid last operation: {}", op_type);
                    }
                } else {
                    eprintln!("✓ Undid {} operations:", undone_count);
                    for (i, (op_type, affected)) in operations.iter().enumerate() {
                        if *affected > 1 {
                            eprintln!("  {}. {} (batch: {} bookmarks)", i + 1, op_type, affected);
                        } else {
                            eprintln!("  {}. {}", i + 1, op_type);
                        }
                    }
                }
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
            if let Some(selected) = bukurs::fuzzy::run_fuzzy_search(&records, query)? {
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

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use rstest::rstest;

    // Helper to parse CLI arguments from a string
    fn parse_args(args: &str) -> Result<Cli, clap::Error> {
        let args_vec: Vec<&str> = args.split_whitespace().collect();
        Cli::try_parse_from(std::iter::once("buku").chain(args_vec))
    }

    // Helper to expect successful parsing
    fn parse_args_ok(args: &str) -> Cli {
        parse_args(args).expect("Failed to parse valid arguments")
    }

    #[test]
    fn test_no_args() {
        let cli = parse_args_ok("");
        assert!(!cli.version);
        assert_eq!(cli.db, None);
        assert!(!cli.nc);
        assert!(!cli.debug);
        assert_eq!(cli.format, None);
        assert!(!cli.open);
        assert_eq!(cli.limit, None);
        assert!(cli.keywords.is_empty());
        assert!(cli.command.is_none());
    }

    #[rstest]
    #[case("--version", true)]
    #[case("-v", true)]
    fn test_version_flag(#[case] args: &str, #[case] expected: bool) {
        let cli = parse_args_ok(args);
        assert_eq!(cli.version, expected);
    }

    #[rstest]
    #[case("--db /path/to/db.db", Some("/path/to/db.db"))]
    #[case("--db custom.db", Some("custom.db"))]
    fn test_db_path(#[case] args: &str, #[case] expected: Option<&str>) {
        let cli = parse_args_ok(args);
        assert_eq!(cli.db.as_ref().map(|p| p.to_str().unwrap()), expected);
    }

    #[rstest]
    #[case("--nc", true)]
    #[case("", false)]
    fn test_no_color_flag(#[case] args: &str, #[case] expected: bool) {
        let cli = parse_args_ok(args);
        assert_eq!(cli.nc, expected);
    }

    #[rstest]
    #[case("--debug", true)]
    #[case("-g", true)]
    #[case("", false)]
    fn test_debug_flag(#[case] args: &str, #[case] expected: bool) {
        let cli = parse_args_ok(args);
        assert_eq!(cli.debug, expected);
    }

    #[rstest]
    #[case("--format json", Some("json"))]
    #[case("-f markdown", Some("markdown"))]
    #[case("", None)]
    fn test_format_option(#[case] args: &str, #[case] expected: Option<&str>) {
        let cli = parse_args_ok(args);
        assert_eq!(cli.format.as_deref(), expected);
    }

    #[rstest]
    #[case("--open", true)]
    #[case("-o", true)]
    #[case("", false)]
    fn test_open_flag(#[case] args: &str, #[case] expected: bool) {
        let cli = parse_args_ok(args);
        assert_eq!(cli.open, expected);
    }

    #[rstest]
    #[case("--limit 10", Some(10))]
    #[case("-n 5", Some(5))]
    #[case("", None)]
    fn test_limit_option(#[case] args: &str, #[case] expected: Option<usize>) {
        let cli = parse_args_ok(args);
        assert_eq!(cli.limit, expected);
    }

    #[rstest]
    #[case("rust programming", vec!["rust", "programming"])]
    #[case("test", vec!["test"])]
    #[case("", vec![])]
    fn test_search_keywords(#[case] args: &str, #[case] expected: Vec<&str>) {
        let cli = parse_args_ok(args);
        assert_eq!(cli.keywords, expected);
    }

    // Add command tests
    #[rstest]
    #[case("add https://example.com")]
    #[case("add https://rust-lang.org --title Rust")]
    #[case("add https://test.com --tag rust,programming")]
    #[case("add https://test.com --comment Description")]
    #[case("add https://test.com --offline")]
    fn test_add_command(#[case] args: &str) {
        let cli = parse_args_ok(args);
        assert!(matches!(cli.command, Some(Commands::Add { .. })));
    }

    #[test]
    fn test_add_command_with_all_options() {
        let cli = parse_args_ok(
            "add https://example.com --title Test --tag rust --tag test --comment Description --offline",
        );
        match cli.command {
            Some(Commands::Add {
                url,
                tag,
                title,
                comment,
                offline,
            }) => {
                assert_eq!(url, "https://example.com");
                assert_eq!(title, Some("Test".to_string()));
                assert_eq!(tag, Some(vec!["rust".to_string(), "test".to_string()]));
                assert_eq!(comment, Some("Description".to_string()));
                assert!(offline);
            }
            _ => panic!("Expected Add command"),
        }
    }

    // Update command tests
    #[rstest]
    #[case("update 1 --url https://new.com")]
    #[case("update 42 --title NewTitle")]
    #[case("update 5 --tag updated")]
    #[case("update 10 --comment UpdatedDescription")]
    #[case("update 3 --immutable 1")]
    fn test_update_command(#[case] args: &str) {
        let cli = parse_args_ok(args);
        assert!(matches!(cli.command, Some(Commands::Update { .. })));
    }

    #[test]
    fn test_update_command_details() {
        let cli = parse_args_ok("update 42");
        assert!(matches!(cli.command, Some(Commands::Update { .. })));

        // Verify the ids are parsed correctly
        if let Some(Commands::Update { ids, .. }) = cli.command {
            assert_eq!(ids, vec!["42".to_string()]);
        } else {
            panic!("Expected Update command");
        }
    }

    // Delete command tests
    #[rstest]
    #[case("delete 1")]
    #[case("delete 1 2 3")]
    #[case("delete 1-5")]
    #[case("delete --force 1")]
    #[case("delete --retain-order 1 2")]
    fn test_delete_command(#[case] args: &str) {
        let cli = parse_args_ok(args);
        assert!(matches!(cli.command, Some(Commands::Delete { .. })));
    }

    #[test]
    fn test_delete_command_with_force() {
        let cli = parse_args_ok("delete --force 1 2 3");
        match cli.command {
            Some(Commands::Delete { ids, force, .. }) => {
                assert_eq!(ids, vec!["1", "2", "3"]);
                assert!(force);
            }
            _ => panic!("Expected Delete command"),
        }
    }

    // Print command tests
    #[rstest]
    #[case("print")]
    #[case("print 1")]
    #[case("print 1 2 3")]
    #[case("print --columns 5")]
    fn test_print_command(#[case] args: &str) {
        let cli = parse_args_ok(args);
        assert!(matches!(cli.command, Some(Commands::Print { .. })));
    }

    // Search command tests
    #[rstest]
    #[case("search rust")]
    #[case("search rust programming")]
    #[case("search --all keyword1 keyword2")]
    #[case("search --deep test")]
    #[case("search --regex '^http'")]
    #[case("search --markers tag:test")]
    fn test_search_command(#[case] args: &str) {
        let cli = parse_args_ok(args);
        assert!(matches!(cli.command, Some(Commands::Search { .. })));
    }

    #[test]
    fn test_search_command_all_flag() {
        let cli = parse_args_ok("search --all rust web");
        match cli.command {
            Some(Commands::Search { keywords, all, .. }) => {
                assert_eq!(keywords, vec!["rust", "web"]);
                assert!(all);
            }
            _ => panic!("Expected Search command"),
        }
    }

    // Tag command tests
    #[rstest]
    #[case("tag rust")]
    #[case("tag programming web")]
    #[case("tag")]
    fn test_tag_command(#[case] args: &str) {
        let cli = parse_args_ok(args);
        assert!(matches!(cli.command, Some(Commands::Tag { .. })));
    }

    // Lock/Unlock command tests
    #[rstest]
    #[case("lock", 8)]
    #[case("lock 16", 16)]
    #[case("unlock", 8)]
    #[case("unlock 10", 10)]
    fn test_lock_unlock_commands(#[case] args: &str, #[case] expected_iterations: u32) {
        let cli = parse_args_ok(args);
        match cli.command {
            Some(Commands::Lock { iterations }) => {
                assert_eq!(iterations, expected_iterations);
            }
            Some(Commands::Unlock { iterations }) => {
                assert_eq!(iterations, expected_iterations);
            }
            _ => panic!("Expected Lock or Unlock command"),
        }
    }

    // Import/Export command tests
    #[rstest]
    #[case("import bookmarks.html")]
    #[case("export output.html")]
    fn test_import_export_commands(#[case] args: &str) {
        let cli = parse_args_ok(args);
        match args.split_whitespace().next().unwrap() {
            "import" => assert!(matches!(cli.command, Some(Commands::Import { .. }))),
            "export" => assert!(matches!(cli.command, Some(Commands::Export { .. }))),
            _ => panic!("Unexpected command"),
        }
    }

    // ImportBrowsers command tests
    #[rstest]
    #[case("import-browsers --list")]
    #[case("import-browsers --all")]
    #[case("import-browsers --browsers chrome")]
    #[case("import-browsers --browsers chrome,firefox")]
    #[case("import-browsers -b chrome,firefox,edge")]
    fn test_import_browsers_command(#[case] args: &str) {
        let cli = parse_args_ok(args);
        assert!(matches!(cli.command, Some(Commands::ImportBrowsers { .. })));
    }

    #[test]
    fn test_import_browsers_list_flag() {
        let cli = parse_args_ok("import-browsers --list");
        match cli.command {
            Some(Commands::ImportBrowsers {
                list,
                all,
                browsers,
            }) => {
                assert!(list);
                assert!(!all);
                assert!(browsers.is_none());
            }
            _ => panic!("Expected ImportBrowsers command"),
        }
    }

    #[test]
    fn test_import_browsers_all_flag() {
        let cli = parse_args_ok("import-browsers --all");
        match cli.command {
            Some(Commands::ImportBrowsers { list, all, .. }) => {
                assert!(!list);
                assert!(all);
            }
            _ => panic!("Expected ImportBrowsers command"),
        }
    }

    #[test]
    fn test_import_browsers_with_browser_list() {
        let cli = parse_args_ok("import-browsers --browsers chrome,firefox");
        match cli.command {
            Some(Commands::ImportBrowsers { browsers, .. }) => {
                assert_eq!(
                    browsers,
                    Some(vec!["chrome".to_string(), "firefox".to_string()])
                );
            }
            _ => panic!("Expected ImportBrowsers command"),
        }
    }

    // Open command tests
    #[rstest]
    #[case("open 1")]
    #[case("open 1 2 3")]
    fn test_open_command(#[case] args: &str) {
        let cli = parse_args_ok(args);
        assert!(matches!(cli.command, Some(Commands::Open { .. })));
    }

    // Shell command test
    #[test]
    fn test_shell_command() {
        let cli = parse_args_ok("shell");
        assert!(matches!(cli.command, Some(Commands::Shell)));
    }

    // Edit command tests
    #[rstest]
    #[case("edit 1", Some(1))]
    #[case("edit 42", Some(42))]
    #[case("edit", None)]
    fn test_edit_command(#[case] args: &str, #[case] expected_id: Option<usize>) {
        let cli = parse_args_ok(args);
        match cli.command {
            Some(Commands::Edit { id }) => {
                assert_eq!(id, expected_id);
            }
            _ => panic!("Expected Edit command"),
        }
    }

    // Undo command test
    #[test]
    fn test_undo_command() {
        let cli = parse_args_ok("undo");
        assert!(matches!(cli.command, Some(Commands::Undo { .. })));

        if let Some(Commands::Undo { count }) = cli.command {
            assert_eq!(count, 1); // Default value
        }
    }

    #[test]
    fn test_undo_command_with_count() {
        let cli = parse_args_ok("undo 100");
        assert!(matches!(cli.command, Some(Commands::Undo { .. })));

        if let Some(Commands::Undo { count }) = cli.command {
            assert_eq!(count, 100);
        }
    }

    // Combined flag tests
    #[rstest]
    #[case("--nc --debug search test")]
    #[case("-g -n 10 search rust")]
    #[case("--format json --open print")]
    fn test_combined_flags(#[case] args: &str) {
        let result = parse_args(args);
        assert!(result.is_ok(), "Failed to parse: {}", args);
    }

    #[test]
    fn test_all_top_level_flags() {
        let cli =
            parse_args_ok("--nc --debug --format json --open --limit 5 --db test.db search test");
        assert!(cli.nc);
        assert!(cli.debug);
        assert_eq!(cli.format.as_deref(), Some("json"));
        assert!(cli.open);
        assert_eq!(cli.limit, Some(5));
        assert_eq!(
            cli.db.as_ref().map(|p| p.to_str().unwrap()),
            Some("test.db")
        );
    }

    // Error cases
    #[rstest]
    #[case("add")] // Missing required URL
    #[case("update")] // Missing required ID
    #[case("delete")] // No IDs provided (actually valid, but worth testing behavior)
    fn test_invalid_commands(#[case] args: &str) {
        let result = parse_args(args);
        // These should either fail or parse in a specific way
        // We're just ensuring they don't panic
        let _ = result;
    }

    #[test]
    fn test_version_short_flag() {
        let cli = parse_args_ok("-v");
        assert!(cli.version);
    }

    #[test]
    fn test_debug_short_flag() {
        let cli = parse_args_ok("-g");
        assert!(cli.debug);
    }

    // Test that mutually exclusive options can be parsed
    // (actual mutual exclusivity would be enforced in the handler)
    #[test]
    fn test_import_browsers_multiple_options() {
        // While not semantically correct, CLI parsing should succeed
        // The handler will enforce business logic
        let cli = parse_args_ok("import-browsers --list --all");
        match cli.command {
            Some(Commands::ImportBrowsers { list, all, .. }) => {
                assert!(list);
                assert!(all);
            }
            _ => panic!("Expected ImportBrowsers command"),
        }
    }
}
