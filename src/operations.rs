use crate::db::BukuDb;
use crate::models::bookmark::Bookmark;

/// Deletion modes supported by the application
#[derive(Debug, Clone, PartialEq)]
pub enum DeleteMode {
    /// Delete all bookmarks
    All,
    /// Delete bookmarks by specific IDs
    ByIds(Vec<usize>),
    /// Delete bookmarks matching keywords
    ByKeywords(Vec<String>),
}

/// Represents a prepared delete operation with all necessary data
pub struct DeleteOperation {
    /// The deletion mode
    pub mode: DeleteMode,
    /// IDs that will be deleted
    pub ids_to_delete: Vec<usize>,
    /// Bookmarks that will be deleted
    pub bookmarks: Vec<Bookmark>,
}

/// Check if input looks like an ID or range (numeric), not a keyword
pub fn is_id_or_range(input: &str) -> bool {
    let input = input.trim();

    // Wildcard is considered ID-like
    if input == "*" {
        return true;
    }

    // Range format: "5-10"
    if input.contains('-') {
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
        let input = input.trim();

        if input == "*" {
            // Wildcard - return all IDs
            return Ok(all_ids);
        } else if input.contains('-') {
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

/// Prepare a delete operation by analyzing inputs and fetching affected bookmarks
/// This is interface-agnostic and doesn't prompt or print
pub fn prepare_delete(
    inputs: &[String],
    db: &BukuDb,
) -> Result<DeleteOperation, Box<dyn std::error::Error>> {
    // Determine deletion mode and get IDs
    let (mode, ids_to_delete) = if inputs.is_empty() {
        // No args → delete all bookmarks
        let all_records = db.get_rec_all()?;
        let all_ids: Vec<usize> = all_records.iter().map(|b| b.id).collect();
        (DeleteMode::All, all_ids)
    } else if inputs.iter().all(|s| is_id_or_range(s)) {
        // All inputs look like IDs/ranges → ID-based deletion
        let ids = parse_ranges(inputs, db)?;
        (DeleteMode::ByIds(ids.clone()), ids)
    } else {
        // Otherwise → keyword-based deletion
        let records = db.search(inputs, true, false, false)?;
        let found_ids: Vec<usize> = records.iter().map(|b| b.id).collect();
        (DeleteMode::ByKeywords(inputs.to_vec()), found_ids)
    };

    // Fetch bookmark details for the IDs
    let mut bookmarks = Vec::new();
    for id in &ids_to_delete {
        if let Some(bookmark) = db.get_rec_by_id(*id)? {
            bookmarks.push(bookmark);
        }
    }

    Ok(DeleteOperation {
        mode,
        ids_to_delete,
        bookmarks,
    })
}

/// Execute a delete operation
/// Returns the number of bookmarks deleted
pub fn execute_delete(
    operation: &DeleteOperation,
    db: &BukuDb,
) -> Result<usize, Box<dyn std::error::Error>> {
    // Delete in reverse order to maintain indices
    let mut sorted_ids = operation.ids_to_delete.clone();
    sorted_ids.sort_by(|a, b| b.cmp(a)); // Sort descending

    for id in &sorted_ids {
        db.delete_rec(*id)?;
    }

    Ok(sorted_ids.len())
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
    fn test_delete_mode_equality() {
        assert_eq!(DeleteMode::All, DeleteMode::All);
        assert_eq!(DeleteMode::ByIds(vec![1, 2]), DeleteMode::ByIds(vec![1, 2]));
        assert_ne!(DeleteMode::All, DeleteMode::ByIds(vec![1]));
    }
}
