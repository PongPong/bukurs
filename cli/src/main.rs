mod cli;
mod editor;
mod format;
mod interactive;
mod output;

use bukurs::{config, db, utils};
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

    // Load configuration
    let cfg = if let Some(config_path) = &args.config {
        config::Config::load_from_path(config_path)?
    } else {
        config::Config::load()
    };

    cli::handle_args(args, &db, &db_path, &cfg)?;

    Ok(())
}
