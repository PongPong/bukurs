use crate::format::traits::BookmarkFormat;
use crate::models::bookmark::Bookmark;

pub struct YamlBookmark<'a>(pub &'a Bookmark);

impl BookmarkFormat for YamlBookmark<'_> {
    fn to_string(&self) -> String {
        serde_yaml::to_string(self.0).unwrap()
    }
}
