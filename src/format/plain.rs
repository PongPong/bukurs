use crate::format::traits::BookmarkFormat;
use crate::models::bookmark::Bookmark;
use crate::tags::parse_tags;

pub struct PlainBookmark<'a>(pub &'a Bookmark);

impl<'a> BookmarkFormat for PlainBookmark<'a> {
    fn to_string(&self) -> String {
        let mut s = String::new();
        let id = self.0.id.to_string();
        s.push_str(&format!("{}. {}\n", id, self.0.title,));
        let padding = id.len() + 3;
        // padding for alignment
        s.push_str(&format!("{:>padding$} {}\n", ">", self.0.url));

        // Only show description if non-empty
        if !self.0.description.trim().is_empty() {
            s.push_str(&format!("{:>padding$} {}\n", "+", self.0.description));
        }

        // Parse tags and only show if non-empty
        let tags = parse_tags(&self.0.tags);
        if !tags.is_empty() {
            let tags_str = tags.join(", ");
            s.push_str(&format!("{:>padding$} {}\n", "#", tags_str));
        }
        s
    }
}
