use crate::format::OutputFormat;
use bukurs::browser;
use bukurs::error::Result;
use bukurs::models::bookmark::Bookmark;

/// Helper function to handle fuzzy search selection and open/display the selected bookmark
///
/// This function is shared across multiple commands (NoCommand, SearchCommand, TagCommand)
/// to avoid code duplication for the common pattern of:
/// 1. Run fuzzy picker on bookmarks
/// 2. Either open the selected bookmark in browser or display it
pub fn handle_bookmark_selection(
    records: &[Bookmark],
    query: Option<String>,
    open: bool,
    format: Option<&str>,
    nc: bool,
) -> Result<()> {
    if let Some(selected) = bukurs::fuzzy::run_fuzzy_search(records, query)? {
        if open {
            eprintln!("Opening: {}", selected.url);
            browser::open_url(&selected.url)?;
        } else {
            let output_format: OutputFormat = format
                .map(OutputFormat::from_string)
                .unwrap_or(OutputFormat::Colored);
            let selected = vec![selected];
            output_format.print_bookmarks(&selected, nc);
        }
    }
    Ok(())
}
