use std::sync::{Arc, OnceLock};

use crate::models::bookmark::Bookmark;
use nucleo_picker::{Picker, Render};

/// Wrapper for rendering bookmarks in the picker
/// Stores only the ID and display string, bookmark can be looked up after selection
struct BookmarkItem {
    id: usize,
    display: String,
}

static EMPTY_STRING: OnceLock<Arc<String>> = OnceLock::new();

fn empty_string() -> Arc<String> {
    EMPTY_STRING.get_or_init(|| Arc::new(String::new())).clone()
}

impl BookmarkItem {
    fn new(bookmark: &Bookmark, max_id_width: usize) -> Self {
        let tags = if bookmark.tags.is_empty() {
            empty_string()
        } else {
            Arc::new(format!(" #{}", bookmark.tags))
        };

        // Format with fixed-width ID section to ensure visibility
        // [ID] always takes the same space, making it act like a pinned column
        // Bold cyan ID | Title and tags | URL
        let id_section = format!(
            "\x1b[1;36m[{:>width$}]\x1b[0m",
            bookmark.id,
            width = max_id_width
        );

        // Truncate URL if it's too long to ensure ID stays visible
        let max_url_len = 80;
        let url_display = if bookmark.url.len() > max_url_len {
            &bookmark.url[..max_url_len]
        } else {
            &bookmark.url
        };

        let display = format!(
            "{} {}{} | {}",
            id_section, bookmark.title, tags, url_display
        );

        Self {
            id: bookmark.id,
            display,
        }
    }
}

/// Renderer for bookmark items
struct BookmarkRenderer;

impl Render<BookmarkItem> for BookmarkRenderer {
    type Str<'a> = &'a str;

    fn render<'a>(&self, item: &'a BookmarkItem) -> Self::Str<'a> {
        &item.display
    }
}

pub fn run_fuzzy_search(
    bookmarks: &[Bookmark],
    _query: Option<String>,
) -> crate::error::Result<Option<Bookmark>> {
    if bookmarks.is_empty() {
        return Ok(None);
    }

    // Calculate the width needed for the largest ID
    let max_id_width = bookmarks
        .iter()
        .map(|b| b.id.to_string().len())
        .max()
        .unwrap_or(1);

    // Create picker
    let mut picker = Picker::new(BookmarkRenderer);

    // Inject all bookmarks (only store ID and display string)
    let injector = picker.injector();
    for bookmark in bookmarks {
        let item = BookmarkItem::new(bookmark, max_id_width);
        injector.push(item);
    }

    // Run picker and get selection
    match picker.pick() {
        Ok(Some(item)) => {
            // Look up the full bookmark by ID to avoid cloning all bookmarks upfront
            Ok(bookmarks.iter().find(|b| b.id == item.id).cloned())
        }
        Ok(None) => Ok(None),
        Err(e) => Err(crate::error::BukursError::FuzzySearch(e.to_string())),
    }
}

/// Wrapper for rendering tags in the picker
struct TagItem {
    tag: String,
}

/// Renderer for tag items
struct TagRenderer;

impl Render<TagItem> for TagRenderer {
    type Str<'a> = &'a str;

    fn render<'a>(&self, item: &'a TagItem) -> Self::Str<'a> {
        &item.tag
    }
}

/// Run fuzzy search on tags and return the selected tag
pub fn run_fuzzy_tag_search(tags: &[String]) -> crate::error::Result<Option<String>> {
    if tags.is_empty() {
        return Ok(None);
    }

    // Create picker
    let mut picker = Picker::new(TagRenderer);

    // Inject all tags
    let injector = picker.injector();
    for tag in tags {
        injector.push(TagItem { tag: tag.clone() });
    }

    // Run picker and get selection
    match picker.pick() {
        Ok(Some(item)) => Ok(Some(item.tag.clone())),
        Ok(None) => Ok(None),
        Err(e) => Err(crate::error::BukursError::FuzzySearch(e.to_string())),
    }
}
