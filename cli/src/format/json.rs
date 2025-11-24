use crate::format::traits::BookmarkFormat;
use bukurs::models::bookmark::Bookmark;

pub struct JsonBookmark<'a>(pub &'a Bookmark);

impl<'a> BookmarkFormat for JsonBookmark<'a> {
    fn to_string(&self) -> String {
        serde_json::to_string_pretty(self.0).unwrap()
    }
}
