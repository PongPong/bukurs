use serde::{Deserialize, Serialize};

/// Represents a bookmark with all its metadata
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Bookmark {
    pub id: usize,
    pub url: String,
    pub title: String,
    pub tags: String,
    pub description: String,
}

impl Bookmark {
    /// Create a new Bookmark
    pub fn new(id: usize, url: String, title: String, tags: String, description: String) -> Self {
        Self {
            id,
            url,
            title,
            tags,
            description,
        }
    }
    
    /// Create from database tuple format
    pub fn from_tuple(id: usize, tuple: (String, String, String, String)) -> Self {
        Self {
            id,
            url: tuple.0,
            title: tuple.1,
            tags: tuple.2,
            description: tuple.3,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_bookmark_creation() {
        let bookmark = Bookmark::new(
            1,
            "https://example.com".to_string(),
            "Example".to_string(),
            ",rust,".to_string(),
            "A test bookmark".to_string(),
        );
        
        assert_eq!(bookmark.id, 1);
        assert_eq!(bookmark.url, "https://example.com");
        assert_eq!(bookmark.title, "Example");
    }
    
    #[test]
    fn test_bookmark_serialization() {
        let bookmark = Bookmark::new(
            1,
            "https://example.com".to_string(),
            "Example".to_string(),
            ",rust,".to_string(),
            "A test".to_string(),
        );
        
        let json = serde_json::to_string(&bookmark).unwrap();
        assert!(json.contains("\"id\":1"));
        assert!(json.contains("\"url\":\"https://example.com\""));
        
        let deserialized: Bookmark = serde_json::from_str(&json).unwrap();
        assert_eq!(bookmark, deserialized);
    }
}
