use crate::format::traits::BookmarkFormat;
use crate::models::bookmark::Bookmark;

impl BookmarkFormat for Bookmark {
    fn to_string(&self) -> String {
        format!(
            "{}. {}\n   > {}\n   + {}\n   # {}",
            self.id, self.title, self.url, self.description, self.tags
        )
    }
}
