use crate::format::traits::BookmarkFormat;
use bukurs::models::bookmark::Bookmark;

pub struct TomlBookmark<'a>(pub &'a Bookmark);

impl<'a> BookmarkFormat for TomlBookmark<'a> {
    fn to_string(&self) -> String {
        toml::to_string_pretty(self.0).unwrap()
    }
}
