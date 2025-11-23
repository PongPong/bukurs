use crate::{
    format::{
        json::JsonBookmark, //
        toml::TomlBookmark,
        toon::ToonBookmark,
        traits::BookmarkFormat,
        yaml::YamlBookmark,
    },
    output::colorize::{Colorize, ColorizeBookmark},
};

pub mod json;
pub mod plain;
pub mod toml;
pub mod toon;
pub mod traits;
pub mod yaml;

#[derive(Debug, Clone, Copy)]
pub enum OutputFormat {
    Json,
    Yaml,
    Toml,
    Toon,
    Colored,
}

impl OutputFormat {
    pub fn from_str(format: &str) -> Option<OutputFormat> {
        match format.to_lowercase().as_str() {
            "json" => Some(OutputFormat::Json),
            "yaml" | "yml" => Some(OutputFormat::Yaml),
            "toml" => Some(OutputFormat::Toml),
            "toon" => Some(OutputFormat::Toon),
            "colored" | "color" | "plain" => Some(OutputFormat::Colored),
            _ => None,
        }
    }

    pub fn print_bookmarks(
        self: Self,
        records: &Vec<crate::models::bookmark::Bookmark>,
        no_color: bool,
    ) {
        match self {
            OutputFormat::Json => {
                for b in records {
                    println!("{}", JsonBookmark(b).to_string());
                }
            }
            OutputFormat::Yaml => {
                for b in records {
                    println!("{}", YamlBookmark(b).to_string());
                }
            }
            OutputFormat::Toml => {
                for b in records {
                    println!("{}", TomlBookmark(b).to_string());
                }
            }
            OutputFormat::Toon => {
                for b in records {
                    println!("{}", ToonBookmark(b).to_string());
                }
            }
            OutputFormat::Colored => {
                for b in records {
                    if no_color {
                        // Plain text output
                        println!("{}", b.to_string());
                    } else {
                        println!("{}", ColorizeBookmark(b).to_colored());
                    }
                }
            }
        }
    }
}
