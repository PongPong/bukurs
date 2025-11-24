use crate::format::traits::BookmarkFormat;
use bukurs::models::bookmark::Bookmark;

pub struct ToonBookmark<'a>(pub &'a Bookmark);

impl<'a> BookmarkFormat for ToonBookmark<'a> {
    fn to_string(&self) -> String {
        format!(
            "📘 {}\n🔗 {}\n📝 {}\n",
            self.0.title, self.0.url, self.0.description
        )
    }
}
