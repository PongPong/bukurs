use crate::format::traits::BookmarkFormat;
use crate::models::bookmark::Bookmark;

impl BookmarkFormat for Bookmark {
    fn to_string(&self) -> String {
        let id = self.id.to_string();

        let mut s = String::new();
        s.push_str(&format!("{}. {}\n", id, self.title,));
        let padding = id.len() + 3;
        // padding for alignment
        s.push_str(&format!("{:>padding$} {}\n", ">", self.url));
        s.push_str(&format!("{:>padding$} {}\n", "+", self.description));
        s
    }
}
