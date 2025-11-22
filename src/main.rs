mod db;
mod crypto;
mod fetch;
mod utils;
mod interactive;
mod import_export;
mod browser;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(author, version, about, long_about = None, disable_version_flag = true)]
struct Cli {
    /// Show the program version and exit
    #[arg(short = 'v', long = "version")]
    version: bool,

    /// Search keywords
    #[arg(name = "KEYWORD")]
    keywords: Vec<String>,

    /// Optional custom database file path
    #[arg(long)]
    db: Option<PathBuf>,

    /// Bookmark URL with comma-separated tags
    #[arg(short, long, value_names = &["URL", "tags"])]
    add: Option<Vec<String>>,

    /// Update fields of an existing bookmark
    #[arg(short, long, num_args = 0..=1)]
    update: Option<Vec<String>>,

    /// Edit and add a new bookmark in editor
    #[arg(short, long, num_args = 0..=1)]
    write: Option<Vec<String>>,

    /// Remove bookmarks from DB
    #[arg(short, long, num_args = 0..=1)]
    delete: Option<Vec<String>>,

    /// Prevents reordering after deleting a bookmark
    #[arg(long)]
    retain_order: bool,

    /// Bookmark link (for edit)
    #[arg(long)]
    url: Option<String>,

    /// Comma-separated tags (for edit)
    #[arg(long, num_args = 0..=1)]
    tag: Option<Vec<String>>,

    /// Bookmark title (for edit)
    #[arg(long, num_args = 0..=1)]
    title: Option<Vec<String>>,

    /// Notes or description of the bookmark
    #[arg(short, long, num_args = 0..=1)]
    comment: Option<Vec<String>>,

    /// Disable web-fetch during auto-refresh
    #[arg(long)]
    immutable: Option<u8>,

    /// Swap two records at specified indices
    #[arg(long, num_args = 2)]
    swap: Option<Vec<usize>>,

    /// Find records with ANY matching keyword
    #[arg(short = 's', long = "sany")]
    sany: Option<Vec<String>>,

    /// Find records matching ALL the keywords
    #[arg(short = 'S', long = "sall")]
    sall: Option<Vec<String>>,

    /// Match substrings
    #[arg(long)]
    deep: bool,

    /// Search for keywords in specific fields
    #[arg(long)]
    markers: bool,

    /// Run a regex search
    #[arg(short = 'r', long = "sreg")]
    sreg: Option<String>,

    /// Search bookmarks by tags
    #[arg(short = 't', long = "stag", num_args = 0..)]
    stag: Option<Vec<String>>,

    /// Omit records matching specified keywords
    #[arg(short = 'x', long = "exclude")]
    exclude: Option<Vec<String>>,

    /// Output random bookmarks out of the selection
    #[arg(long)]
    random: Option<usize>,

    /// Comma-separated list of fields to order the output by
    #[arg(long)]
    order: Option<Vec<String>>,

    /// Encrypt DB in N iterations
    #[arg(short = 'l', long = "lock")]
    lock: Option<u32>,

    /// Decrypt DB in N iterations
    #[arg(short = 'k', long = "unlock")]
    unlock: Option<u32>,

    /// Auto-import bookmarks from web browsers
    #[arg(long)]
    ai: bool,

    /// Export bookmarks
    #[arg(short, long)]
    export: Option<String>,

    /// Import bookmarks from file
    #[arg(short, long)]
    import: Option<String>,

    /// Show record details by indices, ranges
    #[arg(short, long, num_args = 0..)]
    print: Option<Vec<String>>,

    /// Limit fields in -p or JSON search output
    #[arg(short, long)]
    format: Option<u8>,

    /// JSON formatted output
    #[arg(short, long, num_args = 0..=1)]
    json: Option<Vec<String>>,

    /// Set output colors
    #[arg(long)]
    colors: Option<String>,

    /// Disable color output
    #[arg(long)]
    nc: bool,

    /// Show N results per page
    #[arg(short = 'n', long)]
    count: Option<usize>,

    /// Do not show the subprompt, run and exit
    #[arg(long)]
    np: bool,

    /// Browse bookmarks by indices and ranges
    #[arg(short, long, num_args = 0..)]
    open: Option<Vec<String>>,

    /// Browse all search results immediately
    #[arg(long)]
    oa: bool,

    /// If scheme is missing from uri, assume S
    #[arg(long)]
    default_scheme: Option<String>,

    /// Replace old tag with new tag everywhere
    #[arg(long, num_args = 1..=2)]
    replace: Option<Vec<String>>,

    /// When fetching an URL, use the resulting URL from following redirects
    #[arg(long)]
    url_redirect: bool,

    /// Add a tag in specified pattern on redirect
    #[arg(long, num_args = 0..=1)]
    tag_redirect: Option<Vec<String>>,

    /// Add a tag in specified pattern on error
    #[arg(long, num_args = 0..=1)]
    tag_error: Option<Vec<String>>,

    /// Delete/do not add on error
    #[arg(long)]
    del_error: Option<Vec<String>>,

    /// Export records affected by the above options
    #[arg(long)]
    export_on: bool,

    /// Update DB indices to match specified order
    #[arg(long)]
    reorder: Option<Vec<String>>,

    /// Browse a cached page from Wayback Machine
    #[arg(long)]
    cached: Option<String>,

    /// Add a bookmark without connecting to web
    #[arg(long)]
    offline: bool,

    /// Show similar tags when adding bookmarks
    #[arg(long)]
    suggest: bool,

    /// Reduce verbosity
    #[arg(long)]
    tacit: bool,

    /// Do not wait for input
    #[arg(long)]
    nostdin: bool,

    /// Max network connections in full refresh
    #[arg(long)]
    threads: Option<usize>,

    /// Check latest upstream version available
    #[arg(short = 'V')]
    upstream: bool,

    /// Show debug information
    #[arg(short = 'g', long = "debug")]
    debug: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Initialize logger
    env_logger::init();

    if cli.version {
        println!("buku {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let db_path = if let Some(path) = cli.db {
        path
    } else {
        utils::get_default_dbdir().join("bookmarks.db")
    };

    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let db = db::BukuDb::init(&db_path)?;

    // Check if any command flag is present
    let has_args = cli.add.is_some() || cli.update.is_some() || cli.delete.is_some() || 
                   !cli.keywords.is_empty() || cli.sany.is_some() || cli.sall.is_some() || 
                   cli.sreg.is_some() || cli.stag.is_some() || cli.print.is_some();

    if !has_args && !cli.np {
        // Interactive mode
        // First list all bookmarks (optional, but buku does it)
        let records = db.get_rec_all()?;
        for (id, url, title, tags, desc) in records {
            println!("{}. {}\n   > {}\n   + {}\n   # {}", id, title, url, desc, tags);
        }
        interactive::run(&db)?;
    } else if let Some(add_args) = cli.add {
        // Handle add
        let url = add_args.get(0).ok_or("Missing URL")?;
        let mut tags = add_args.get(1..).unwrap_or(&[]).to_vec();
        
        // Merge with --tag flag
        if let Some(extra_tags) = cli.tag {
            tags.extend(extra_tags);
        }

        println!("Fetching metadata for: {}", url);
        let fetch_result = fetch::fetch_data(url).unwrap_or(fetch::FetchResult {
            url: url.clone(),
            title: "".to_string(),
            desc: "".to_string(),
            keywords: "".to_string(),
        });

        let title = if let Some(t) = cli.title {
            t.join(" ")
        } else if !fetch_result.title.is_empty() {
            fetch_result.title
        } else {
            url.clone()
        };

        let desc = if let Some(d) = cli.comment {
            d.join(" ")
        } else {
            fetch_result.desc
        };

        let tags_str = if tags.is_empty() {
            format!(",{},", fetch_result.keywords)
        } else {
            format!(",{},", tags.join(","))
        };

        let id = db.add_rec(&fetch_result.url, &title, &tags_str, &desc)?;
        println!("Added bookmark at index {}", id);
    } else if let Some(print_args) = cli.print {
        // Handle print/list
        let records = db.get_rec_all()?;
        for (id, url, title, tags, desc) in records {
            println!("{}. {}\n   > {}\n   + {}\n   # {}", id, title, url, desc, tags);
        }
    } else if let Some(update_args) = cli.update {
        // Handle update
        // If update_args is empty, it might be a full refresh or update last result?
        // For now, assume index is provided if args are present.
        // But update can take indices OR be used with search.
        // This logic is complex in original buku.
        // Let's implement basic update by index for now.
        
        if let Some(index_str) = update_args.get(0) {
            if let Ok(id) = index_str.parse::<usize>() {
                let url = cli.url.as_deref();
                let title = cli.title.as_deref().map(|v| v.join(" "));
                let tags = cli.tag.as_deref().map(|v| v.join(","));
                let desc = cli.comment.as_deref().map(|v| v.join(" "));
                let immutable = cli.immutable;

                db.update_rec(id, url, title.as_deref(), tags.as_deref(), desc.as_deref(), immutable)?;
                println!("Updated bookmark at index {}", id);
            } else {
                println!("Invalid index: {}", index_str);
            }
        } else {
            println!("Update requires an index (for now)");
        }
    } else if let Some(delete_args) = cli.delete {
        // Handle delete
        if let Some(index_str) = delete_args.get(0) {
             if let Ok(id) = index_str.parse::<usize>() {
                 db.delete_rec(id)?;
                 println!("Deleted bookmark at index {}", id);
             } else {
                 println!("Invalid index: {}", index_str);
             }
        } else {
            println!("Delete requires an index");
        }
    } else if !cli.keywords.is_empty() || cli.sany.is_some() || cli.sall.is_some() || cli.sreg.is_some() {
        // Handle search
        let keywords = if let Some(k) = &cli.sany {
            k
        } else if let Some(k) = &cli.sall {
            k
        } else if let Some(k) = &cli.sreg {
            // Regex search takes a single string usually, but our struct has Option<String>
            // We need to pass it as a slice.
            // Wait, sreg is Option<String>.
            // We can construct a vec.
            // But we need to handle the case where keywords are positional args too.
            // If positional args are present, they are ANY search by default.
            &cli.keywords
        } else {
            &cli.keywords
        };
        
        let any = cli.sany.is_some() || (!cli.keywords.is_empty() && cli.sall.is_none());
        let regex = cli.sreg.is_some();
        
        // If sreg is present, use that as keyword
        let search_terms = if let Some(re) = &cli.sreg {
            vec![re.clone()]
        } else {
            keywords.clone()
        };

        println!("Searching for: {:?}", search_terms);
        let records = db.search(&search_terms, any, cli.deep, regex)?;
        for (id, url, title, tags, desc) in records {
            println!("{}. {}\n   > {}\n   + {}\n   # {}", id, title, url, desc, tags);
        }
    } else if let Some(iterations) = cli.lock {
        // Handle lock
        let password = rpassword::prompt_password("Enter password: ")?;
        let confirm = rpassword::prompt_password("Confirm password: ")?;
        if password != confirm {
            return Err("Passwords do not match".into());
        }
        
        let enc_path = db_path.with_extension("db.enc");
        println!("Encrypting {} to {} with {} iterations...", db_path.display(), enc_path.display(), iterations);
        crypto::BukuCrypt::encrypt_file(iterations, &db_path, &enc_path, &password)?;
        println!("Encryption complete.");
    } else if let Some(iterations) = cli.unlock {
        // Handle unlock
        let password = rpassword::prompt_password("Enter password: ")?;
        let enc_path = if db_path.extension().map_or(false, |ext| ext == "enc") {
            db_path.clone()
        } else {
            db_path.with_extension("db.enc")
        };
        
        // If unlocking, we probably want to decrypt TO the db path.
        // But if db_path is the .enc file, we need to know where to decrypt to.
        // Buku usually assumes db.enc -> db.
        let out_path = if enc_path.extension().map_or(false, |ext| ext == "enc") {
            enc_path.with_extension("")
        } else {
            enc_path.with_extension("db")
        };

        println!("Decrypting {} to {} with {} iterations...", enc_path.display(), out_path.display(), iterations);
        crypto::BukuCrypt::decrypt_file(iterations, &out_path, &enc_path, &password)?;
        println!("Decryption complete.");
    } else if let Some(export_path) = cli.export {
        // Handle export
        import_export::export_bookmarks(&db, &export_path)?;
        println!("Exported bookmarks to {}", export_path);
    } else if let Some(import_path) = cli.import {
        // Handle import
        import_export::import_bookmarks(&db, &import_path)?;
        println!("Imported bookmarks from {}", import_path);
    } else if cli.ai {
        // Handle auto-import
        browser::auto_import()?;
    } else if let Some(open_args) = cli.open {
        // Handle open
        if open_args.is_empty() {
            // Open random?
            println!("Opening random bookmark (not implemented yet)");
        } else {
            // Open by index/range
            for arg in open_args {
                if let Ok(id) = arg.parse::<usize>() {
                    if let Some(rec) = db.get_rec_by_id(id)? {
                        println!("Opening: {}", rec.0);
                        browser::open_url(&rec.0)?;
                    } else {
                        println!("Index {} not found", id);
                    }
                } else {
                    println!("Invalid index: {}", arg);
                }
            }
        }
    } else if let Some(stag_args) = cli.stag {
        // Handle tag search
        // If args are empty, list all tags?
        if stag_args.is_empty() {
             println!("Listing all tags (not implemented yet)");
             // TODO: Implement get_all_tags
        } else {
             // Search by tags
             // Logic similar to search but specifically for tags field
             // And handling + for AND, , for OR
             println!("Searching tags: {:?}", stag_args);
             // For now, reuse search with tag marker '#' if we had it, or just search tags field.
             // But search method searches ALL fields.
             // We need a specific search_tags method or update search to support field filtering.
             // Let's update search to support field filtering or just use search for now as a fallback.
             // Actually, let's implement search_tags in db.rs
             let records = db.search_tags(&stag_args)?;
             for (id, url, title, tags, desc) in records {
                println!("{}. {}\n   > {}\n   + {}\n   # {}", id, title, url, desc, tags);
            }
        }
    } else {
        // Default to list if no args? Or show help?
        // Original buku shows help or list depending on context, but let's default to list for now if nothing else matches
        let records = db.get_rec_all()?;
        for (id, url, title, tags, desc) in records {
            println!("{}. {}\n   > {}\n   + {}\n   # {}", id, title, url, desc, tags);
        }
    }

    Ok(())
}
