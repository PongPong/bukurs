mod browser;
mod cli;
mod crypto;
mod db;
mod editor;
mod fetch;
mod format;
mod fuzzy;
mod import_export;
mod interactive;
mod models;
mod operations;
mod output;
mod utils;

use clap::Parser;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = cli::Cli::parse();

    // Initialize logger
    env_logger::init();

    if args.version {
        println!("buku {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let db_path = if let Some(path) = &args.db {
        path.clone()
    } else {
        utils::get_default_dbdir().join("bookmarks.db")
    };

    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let db = db::BukuDb::init(&db_path)?;

    cli::handle_args(args, &db, &db_path)?;

    Ok(())
}
