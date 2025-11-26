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

/// Enum-based dispatch for commands (avoids Box<dyn BukuCommand>)
pub enum CommandEnum {
    Add(add::AddCommand),
    Update(update::UpdateCommand),
    Delete(delete::DeleteCommand),
    Print(print::PrintCommand),
    Search(search::SearchCommand),
    Tag(tag::TagCommand),
    Lock(lock_unlock::LockCommand),
    Unlock(lock_unlock::UnlockCommand),
    Import(import_export::ImportCommand),
    ImportBrowsers(import_export::ImportBrowsersCommand),
    Export(import_export::ExportCommand),
    Open(misc::OpenCommand),
    Shell(misc::ShellCommand),
    Edit(edit::EditCommand),
    Undo(misc::UndoCommand),
    No(misc::NoCommand),
}

impl CommandEnum {
    pub fn execute(&self, ctx: &AppContext) -> Result<()> {
        match self {
            Self::Add(cmd) => cmd.execute(ctx),
            Self::Update(cmd) => cmd.execute(ctx),
            Self::Delete(cmd) => cmd.execute(ctx),
            Self::Print(cmd) => cmd.execute(ctx),
            Self::Search(cmd) => cmd.execute(ctx),
            Self::Tag(cmd) => cmd.execute(ctx),
            Self::Lock(cmd) => cmd.execute(ctx),
            Self::Unlock(cmd) => cmd.execute(ctx),
            Self::Import(cmd) => cmd.execute(ctx),
            Self::ImportBrowsers(cmd) => cmd.execute(ctx),
            Self::Export(cmd) => cmd.execute(ctx),
            Self::Open(cmd) => cmd.execute(ctx),
            Self::Shell(cmd) => cmd.execute(ctx),
            Self::Edit(cmd) => cmd.execute(ctx),
            Self::Undo(cmd) => cmd.execute(ctx),
            Self::No(cmd) => cmd.execute(ctx),
        }
    }
}
