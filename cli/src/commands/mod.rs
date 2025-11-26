use bukurs::config::Config;
use bukurs::db::BukuDb;
use bukurs::error::Result;
use std::path::Path;

pub struct AppContext<'a> {
    pub db: &'a BukuDb,
    pub config: &'a Config,
    pub db_path: &'a Path,
}

pub mod add;
pub mod delete;
pub mod edit;
pub mod import_export;
pub mod lock_unlock;
pub mod misc;
pub mod print;
pub mod search;
pub mod tag;
pub mod update;

pub trait BukuCommand {
    fn execute(&self, ctx: &AppContext) -> Result<()>;
}
