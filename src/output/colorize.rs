use crate::models::bookmark::Bookmark;
use owo_colors::OwoColorize;

pub trait Colorize {
    fn to_colored(&self) -> String;
}

pub struct ColorizeBookmark<'a>(pub &'a Bookmark);

impl<'a> Colorize for ColorizeBookmark<'a> {
    fn to_colored(&self) -> String {
        let mut s = String::new();
        let id = self.0.id.to_string();
        s.push_str(&format!("{}. {}\n", id.blue(), self.0.title.bold().green(),));
        let padding = id.len() + 3;
        // padding for alignment
        s.push_str(&format!(
            "{:>padding$} {}\n",
            ">".red(),
            self.0.url.yellow()
        ));
        s.push_str(&format!("{:>padding$} {}\n", "+".red(), self.0.description));
        s
    }
}
