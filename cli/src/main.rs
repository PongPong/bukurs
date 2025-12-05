mod cli;
mod commands;
mod editor;
mod fetch_ui;
mod format;
mod interactive;
mod output;
mod plugins;
mod tag_ops;

use bukurs::plugin::PluginManager;
use bukurs::{config, db, error::Result, utils};
use clap::Parser;

fn main() -> Result<()> {
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

    // Initialize plugin system
    let plugins = if args.no_plugins {
        PluginManager::disabled()
    } else {
        let plugin_data_dir = utils::get_default_dbdir().join("plugins");
        std::fs::create_dir_all(&plugin_data_dir)?;
        let mut manager = PluginManager::new(db_path.clone(), plugin_data_dir);

        // Register built-in plugins
        if let Err(e) = plugins::register_all_plugins(&mut manager) {
            log::warn!("Failed to register some plugins: {}", e);
        }

        manager
    };

    cli::handle_args(args, &db, &db_path, &cfg, &plugins)?;

    Ok(())
}
