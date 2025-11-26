pub mod browser;
pub mod export;
pub mod import;

// Re-export main functions for convenience
pub use export::export_bookmarks;
pub use import::{import_bookmarks, import_bookmarks_parallel};
// Re-export browser detection and import functions (used by CLI)
pub use browser::{
    auto_import_all, auto_import_all_with_progress, import_from_selected_browsers,
    import_from_selected_browsers_with_progress, list_detected_browsers,
};
