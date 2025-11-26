use crate::models::bookmark::Bookmark;
use skim::prelude::*;
use std::io::Cursor;

pub fn run_fuzzy_search(
    bookmarks: &[Bookmark],
    query: Option<String>,
) -> Result<Option<Bookmark>, Box<dyn std::error::Error>> {
    let options = SkimOptionsBuilder::default()
        .height("50%".to_string())
        .multi(false)
        .query(query)
        .keep_right(false)
        .delimiter("\t".to_string())
        .with_nth(vec!["1".to_string(), "2".to_string()])
        .build()
        .unwrap();

    // Prepare input for skim with tab-separated columns
    // Column 1: [ID] (dynamic width based on max ID)
    // Column 2: Title #Tags URL
    // Column 3: Full line for parsing (hidden from display)

    // Calculate the width needed for the largest ID
    let max_id_width = bookmarks
        .iter()
        .map(|b| b.id.to_string().len())
        .max()
        .unwrap_or(1);

    let input = bookmarks
        .iter()
        .map(|b| {
            let tags = if b.tags.is_empty() {
                String::new()
            } else {
                format!(" #{}", b.tags)
            };
            // Tab-separated: ID column (bold cyan), then content column
            // Use ANSI codes to make ID stand out
            format!(
                "[{:>width$}]\t{}{} | {}\t{}",
                b.id, b.title, tags, b.url, b.id, width = max_id_width
            )
        })
        .collect::<Vec<String>>()
        .join("\n");

    let item_reader = SkimItemReader::default();
    let items = item_reader.of_bufread(Cursor::new(input));

    let selected_items = Skim::run_with(&options, Some(items))
        .map(|out| out.selected_items)
        .unwrap_or_default();

    if selected_items.is_empty() {
        return Ok(None);
    }

    // Parse the selected item to get the ID
    let selected_text = selected_items[0].output();
    // Extract ID from tab-separated format (last field contains the ID)
    if let Some(id_str) = selected_text.split('\t').nth(2) {
        if let Ok(id) = id_str.trim().parse::<usize>() {
            // Find the bookmark with this ID
            return Ok(bookmarks.iter().find(|b| b.id == id).cloned());
        }
    }

    // Fallback: try to extract from "[ID] Title..." format
    if let Some(start) = selected_text.find('[') {
        if let Some(end) = selected_text.find(']') {
            if let Ok(id) = selected_text[start + 1..end].trim().parse::<usize>() {
                return Ok(bookmarks.iter().find(|b| b.id == id).cloned());
            }
        }
    }

    Ok(None)
}
