use crate::db::BukuDb;
use crate::models::bookmark::Bookmark;
use crate::utils;

/// Selection modes supported by the application
#[derive(Debug, Clone, PartialEq)]
pub enum SelectionMode {
    /// Select all bookmarks
    All,
    /// Select bookmarks by specific IDs
    ByIds(Vec<usize>),
    /// Select bookmarks matching keywords
    ByKeywords(Vec<String>),
}

/// Represents a prepared bookmark selection with all necessary data
pub struct BookmarkSelection {
    /// The selection mode
    pub mode: SelectionMode,
    /// IDs that were selected
    pub selected_ids: Vec<usize>,
    /// Bookmarks that were selected
    pub bookmarks: Vec<Bookmark>,
}

/// Check if input looks like an ID or range (numeric), not a keyword
pub fn is_id_or_range(input: &str) -> bool {
    let input = utils::trim_both_simd(input);

    // Wildcard is considered ID-like
    if input == "*" {
        return true;
    }

    // Range format: "5-10"
    if utils::has_char(b'-', input) {
        let parts: Vec<&str> = input.split('-').collect();
        if parts.len() == 2 {
            return parts[0].parse::<usize>().is_ok() && parts[1].parse::<usize>().is_ok();
        }
        return false;
    }

    // Single ID: "5"
    input.parse::<usize>().is_ok()
}

/// Parse range syntax into individual IDs
/// Supports:
/// - "*" for all bookmarks
/// - Single IDs: "5"
/// - Ranges: "1-5"
/// - Multiple: "1 3 5-7"
pub fn parse_ranges(
    inputs: &[String],
    db: &BukuDb,
) -> Result<Vec<usize>, Box<dyn std::error::Error>> {
    let mut ids = Vec::new();

    // Get all bookmarks to find valid IDs
    let all_records = db.get_rec_all()?;
    if all_records.is_empty() {
        return Ok(ids);
    }

    let all_ids: Vec<usize> = all_records.iter().map(|b| b.id).collect();

    for input in inputs {
        let input = utils::trim_both_simd(input);

        if input == "*" {
            // Wildcard - return all IDs
            return Ok(all_ids);
        } else if utils::has_char(b'-', input) {
            // Range: "5-10"
            let parts: Vec<&str> = input.split('-').collect();
            if parts.len() == 2 {
                if let (Ok(start), Ok(end)) = (parts[0].parse::<usize>(), parts[1].parse::<usize>())
                {
                    for id in start..=end {
                        if all_ids.contains(&id) {
                            ids.push(id);
                        }
                    }
                } else {
                    eprintln!("Warning: Invalid range format: {}", input);
                }
            } else {
                eprintln!("Warning: Invalid range format: {}", input);
            }
        } else {
            // Single ID
            if let Ok(id) = input.parse::<usize>() {
                if all_ids.contains(&id) {
                    ids.push(id);
                }
            } else {
                eprintln!("Warning: Invalid ID: {}", input);
            }
        }
    }

    // Remove duplicates
    ids.sort();
    ids.dedup();

    Ok(ids)
}

/// Resolve bookmarks by analyzing inputs and fetching matching bookmarks
/// This is interface-agnostic and doesn't prompt or print
/// Can be used for delete, print, or any other operation that needs to select bookmarks
pub fn resolve_bookmarks(
    inputs: &[String],
    db: &BukuDb,
) -> Result<BookmarkSelection, Box<dyn std::error::Error>> {
    // Determine selection mode and get IDs
    let (mode, selected_ids) = if inputs.is_empty() {
        // No args → select all bookmarks
        let all_records = db.get_rec_all()?;
        let all_ids: Vec<usize> = all_records.iter().map(|b| b.id).collect();
        (SelectionMode::All, all_ids)
    } else if inputs.iter().all(|s| is_id_or_range(s)) {
        // All inputs are IDs/ranges → select by IDs
        let ids = parse_ranges(inputs, db)?;
        (SelectionMode::ByIds(ids.clone()), ids)
    } else {
        // Inputs are keywords → search for matching bookmarks
        let all_bookmarks = db.get_rec_all()?;
        let matching: Vec<usize> = all_bookmarks
            .iter()
            .filter(|b| {
                inputs.iter().any(|keyword| {
                    let kw_lower = keyword.to_lowercase();
                    b.title.to_lowercase().contains(&kw_lower)
                        || b.description.to_lowercase().contains(&kw_lower)
                        || b.tags.to_lowercase().contains(&kw_lower)
                        || b.url.to_lowercase().contains(&kw_lower)
                })
            })
            .map(|b| b.id)
            .collect();

        (SelectionMode::ByKeywords(inputs.to_vec()), matching)
    };

    // Fetch the actual bookmark data
    let bookmarks: Vec<Bookmark> = selected_ids
        .iter()
        .filter_map(|id| db.get_rec_by_id(*id).ok().flatten())
        .collect();

    Ok(BookmarkSelection {
        mode,
        selected_ids,
        bookmarks,
    })
}

/// Prepare a delete operation (wrapper around resolve_bookmarks for backward compatibility)
pub fn prepare_delete(
    ids: &[String],
    db: &BukuDb,
) -> Result<BookmarkSelection, Box<dyn std::error::Error>> {
    resolve_bookmarks(ids, db)
}

/// Prepare a print operation (wrapper around resolve_bookmarks)
pub fn prepare_print(
    ids: &[String],
    db: &BukuDb,
) -> Result<BookmarkSelection, Box<dyn std::error::Error>> {
    resolve_bookmarks(ids, db)
}

/// Execute a delete operation
/// Returns the number of bookmarks deleted
pub fn execute_delete(
    operation: &BookmarkSelection,
    db: &BukuDb,
) -> Result<usize, Box<dyn std::error::Error>> {
    // For multiple bookmarks, use batch delete to enable batch undo
    if operation.selected_ids.len() > 1 {
        let count = db.delete_rec_batch(&operation.selected_ids)?;
        Ok(count)
    } else if operation.selected_ids.len() == 1 {
        // For single bookmark, use regular delete
        db.delete_rec(operation.selected_ids[0])?;
        Ok(1)
    } else {
        Ok(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_id_or_range_single_id() {
        assert!(is_id_or_range("5"));
        assert!(is_id_or_range("123"));
        assert!(is_id_or_range("  10  ")); // with whitespace
    }

    #[test]
    fn test_is_id_or_range_range() {
        assert!(is_id_or_range("1-5"));
        assert!(is_id_or_range("10-20"));
        assert!(is_id_or_range("  5-10  ")); // with whitespace
    }

    #[test]
    fn test_is_id_or_range_wildcard() {
        assert!(is_id_or_range("*"));
        assert!(is_id_or_range("  *  ")); // with whitespace
    }

    #[test]
    fn test_is_id_or_range_keywords() {
        assert!(!is_id_or_range("rust"));
        assert!(!is_id_or_range("programming"));
        assert!(!is_id_or_range("add"));
        assert!(!is_id_or_range("update"));
    }

    #[test]
    fn test_is_id_or_range_invalid_range() {
        assert!(!is_id_or_range("1-abc"));
        assert!(!is_id_or_range("abc-5"));
        assert!(!is_id_or_range("1-2-3"));
    }

    #[test]
    fn test_is_id_or_range_mixed() {
        // Keywords that happen to contain numbers
        assert!(!is_id_or_range("rust2024"));
        assert!(!is_id_or_range("c++"));
    }

    #[test]
    fn test_selection_mode_equality() {
        assert_eq!(SelectionMode::All, SelectionMode::All);
        assert_eq!(
            SelectionMode::ByIds(vec![1, 2]),
            SelectionMode::ByIds(vec![1, 2])
        );
        assert_ne!(SelectionMode::All, SelectionMode::ByIds(vec![1]));
    }
}
