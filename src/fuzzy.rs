use crate::models::bookmark::Bookmark;
use skim::prelude::*;
use std::io::Cursor;

pub fn run_fuzzy_search(
    bookmarks: &[Bookmark],
    query: Option<&str>,
) -> Result<Option<Bookmark>, Box<dyn std::error::Error>> {
    let options = SkimOptionsBuilder::default()
        .height(Some("50%"))
        .multi(false)
        .query(query)
        .build()
        .unwrap();

    // Prepare input for skim
    // Format: "ID. Title - URL # Tags"
    let input = bookmarks
        .iter()
        .map(|b| format!("{}. {} - {} # {}", b.id, b.title, b.url, b.tags))
        .collect::<Vec<String>>()
        .join("\n");

    let item_reader = SkimItemReader::default();
    let items = item_reader.of_bufread(Cursor::new(input));

    let selected_items = Skim::run_with(&options, Some(items))
        .map(|out| out.selected_items)
        .unwrap_or_else(|| Vec::new());

    if selected_items.is_empty() {
        return Ok(None);
    }

    // Parse the selected item to get the ID
    let selected_text = selected_items[0].output();
    // Extract ID from "ID. Title..."
    if let Some(dot_pos) = selected_text.find('.') {
        if let Ok(id) = selected_text[..dot_pos].parse::<usize>() {
            // Find the bookmark with this ID
            return Ok(bookmarks.iter().find(|b| b.id == id).cloned());
        }
    }

    Ok(None)
}
