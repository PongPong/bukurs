use crate::{
    format::{
        json::JsonBookmark, plain::PlainBookmark, toml::TomlBookmark, toon::ToonBookmark,
        traits::BookmarkFormat, yaml::YamlBookmark,
    },
    output::colorize::{Colorize, ColorizeBookmark},
};

pub mod json;
pub mod plain;
pub mod toml;
pub mod toon;
pub mod traits;
pub mod yaml;

#[derive(Clone, Copy)]
pub enum OutputFormat {
    Json,
    Yaml,
    Toml,
    Toon,
    Colored,
}

impl OutputFormat {
    pub fn from_string(format: &str) -> Self {
        match format {
            "json" => OutputFormat::Json,
            "yaml" | "yml" => OutputFormat::Yaml,
            "toml" => OutputFormat::Toml,
            "toon" => OutputFormat::Toon,
            _ => OutputFormat::Colored,
        }
    }

    pub fn print_bookmarks(
        self,
        records: &Vec<bukurs::models::bookmark::Bookmark>,
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
                        println!("{}", PlainBookmark(b).to_string());
                    } else {
                        println!("{}", ColorizeBookmark(b).to_colored());
                    }
                }
            }
        }
    }
}
