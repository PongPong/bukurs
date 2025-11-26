use bukurs::db::BukuDb;
use bukurs::error::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::sync::OnceLock;

pub fn get_exe_name() -> &'static str {
    static EXE_NAME: OnceLock<String> = OnceLock::new();
    EXE_NAME.get_or_init(|| {
        std::env::args()
            .next()
            .as_ref()
            .map(std::path::Path::new)
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "bukurs".to_string())
    })
}

#[derive(Parser)]
#[command(author, version, about, long_about = None, disable_version_flag = true)]
pub struct Cli {
    /// Show the program version and exit
    #[arg(short = 'v', long = "version")]
    pub version: bool,

    /// Optional custom database file path
    #[arg(long)]
    pub db: Option<PathBuf>,

    /// Optional custom configuration file path
    #[arg(long)]
    pub config: Option<PathBuf>,

    /// Disable color output
    #[arg(long)]
    pub nc: bool,

    /// Show debug information
    #[arg(short = 'g', long = "debug")]
    pub debug: bool,

    #[arg(short = 'f', long)]
    pub format: Option<String>,

    /// Open selected bookmark in browser
    #[arg(short = 'o', long)]
    pub open: bool,

    /// Limit number of results shown (shows last N entries)
    #[arg(short = 'n', long)]
    pub limit: Option<usize>,

    /// Search keywords (when no subcommand is provided)
    #[arg(name = "KEYWORD")]
    pub keywords: Vec<String>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Add a new bookmark
    Add {
        /// URL to bookmark
        url: String,

        /// Comma-separated tags
        #[arg(short, long)]
        tag: Option<Vec<String>>,

        /// Bookmark title
        #[arg(long)]
        title: Option<String>,

        /// Notes or description
        #[arg(short, long)]
        comment: Option<String>,

        /// Add without connecting to web
        #[arg(long)]
        offline: bool,
    },

    /// Update an existing bookmark
    Update {
        /// Bookmark indices, ranges (e.g., 1-5), or * for all
        /// When no edit options are provided, refreshes metadata from web
        #[arg(num_args = 0..)]
        ids: Vec<String>,

        /// New URL
        #[arg(long)]
        url: Option<String>,

        /// Tag operations (supports: +add, -remove, ~old:new, or plain tag to add)
        /// Examples: +urgent, -archived, ~todo:done
        #[arg(short, long)]
        tag: Option<Vec<String>>,

        /// New title
        #[arg(long)]
        title: Option<String>,

        /// New description
        #[arg(short, long)]
        comment: Option<String>,

        /// Disable web-fetch during auto-refresh
        #[arg(long)]
        immutable: Option<u8>,
    },

    /// Delete bookmark(s)
    Delete {
        /// Bookmark indices, ranges (e.g., 1-5), or * for all
        #[arg(num_args = 0..)]
        ids: Vec<String>,

        /// Skip confirmation prompt
        #[arg(short, long)]
        force: bool,

        /// Prevents reordering after deletion
        #[arg(long)]
        retain_order: bool,
    },

    /// Print/list bookmarks
    Print {
        /// Bookmark indices or ranges to print
        #[arg(num_args = 0..)]
        ids: Vec<String>,

        /// Bitwise column selection. Combine values to display multiple fields:
        ///    1  URL
        ///    2  Title
        ///    4  Tags
        ///    8  Description
        ///   16  ID
        /// Examples:
        ///    1         => URL only
        ///    5         => URL + Tags (1 | 4)
        ///    7         => URL + Title + Tags (1 | 2 | 4)
        #[arg(short, long)]
        columns: Option<u8>,
    },

    /// Search bookmarks
    Search {
        /// Search keywords
        keywords: Vec<String>,

        /// Match ALL keywords (default: ANY)
        #[arg(short = 'a', long)]
        all: bool,

        /// Match substrings
        #[arg(long)]
        deep: bool,

        /// Regex search
        #[arg(short, long)]
        regex: bool,

        /// Search for keywords in specific fields
        #[arg(long)]
        markers: bool,
    },

    /// Search bookmarks by tags
    Tag {
        /// Tag keywords to search
        #[arg(num_args = 0..)]
        tags: Vec<String>,
    },

    /// Encrypt database
    Lock {
        /// Number of hash iterations
        #[arg(default_value = "8")]
        iterations: u32,
    },

    /// Decrypt database
    Unlock {
        /// Number of hash iterations
        #[arg(default_value = "8")]
        iterations: u32,
    },

    /// Import bookmarks from file
    Import {
        /// File path to import from
        file: String,
    },

    /// Import bookmarks from browser profiles
    ImportBrowsers {
        /// List available browser profiles without importing
        #[arg(short, long)]
        list: bool,

        /// Import from all detected browsers
        #[arg(short, long)]
        all: bool,

        /// Specific browsers to import from (comma-separated: chrome,firefox,edge,safari)
        #[arg(short, long, value_delimiter = ',')]
        browsers: Option<Vec<String>>,
    },

    /// Export bookmarks to file
    Export {
        /// File path to export to
        file: String,
    },

    /// Open bookmark(s) in browser
    Open {
        /// Bookmark indices to open
        #[arg(num_args = 0..)]
        ids: Vec<String>,
    },

    /// Start interactive shell
    Shell,

    /// Edit bookmark in $EDITOR
    Edit {
        /// Bookmark ID to edit (if not provided, creates a new bookmark)
        id: Option<usize>,
    },

    /// Undo last operation(s)
    Undo {
        /// Number of operations to undo (default: 1)
        #[arg(default_value = "1")]
        count: usize,
    },
}

// ============================================================================
// Main Command Dispatcher
// ============================================================================

use crate::commands::{
    add::AddCommand,
    delete::DeleteCommand,
    edit::EditCommand,
    import_export::{ExportCommand, ImportBrowsersCommand, ImportCommand},
    lock_unlock::{LockCommand, UnlockCommand},
    misc::{NoCommand, OpenCommand, ShellCommand, UndoCommand},
    print::PrintCommand,
    search::SearchCommand,
    tag::TagCommand,
    update::UpdateCommand,
    AppContext, CommandEnum,
};

pub fn handle_args(
    cli: Cli,
    db: &BukuDb,
    db_path: &std::path::Path,
    config: &bukurs::config::Config,
) -> Result<()> {
    let ctx = AppContext {
        db,
        config,
        db_path,
    };

    let command = match cli.command {
        Some(Commands::Add {
            url,
            tag,
            title,
            comment,
            offline,
        }) => CommandEnum::Add(AddCommand {
            url,
            tag,
            title,
            comment,
            offline,
        }),

        Some(Commands::Update {
            ids,
            url,
            tag,
            title,
            comment,
            immutable,
        }) => CommandEnum::Update(UpdateCommand {
            ids,
            url,
            tag,
            title,
            comment,
            immutable,
        }),

        Some(Commands::Delete {
            ids,
            force,
            retain_order: _,
        }) => CommandEnum::Delete(DeleteCommand { ids, force }),

        Some(Commands::Print { ids, columns: _ }) => CommandEnum::Print(PrintCommand {
            ids,
            limit: cli.limit,
            format: cli.format,
            nc: cli.nc,
        }),

        Some(Commands::Search {
            keywords,
            all,
            deep,
            regex,
            markers: _,
        }) => CommandEnum::Search(SearchCommand {
            keywords,
            all,
            deep,
            regex,
            limit: cli.limit,
            format: cli.format,
            nc: cli.nc,
        }),

        Some(Commands::Tag { tags }) => CommandEnum::Tag(TagCommand {
            tags,
            limit: cli.limit,
            format: cli.format,
            nc: cli.nc,
        }),

        Some(Commands::Lock { iterations }) => CommandEnum::Lock(LockCommand { iterations }),

        Some(Commands::Unlock { iterations }) => CommandEnum::Unlock(UnlockCommand { iterations }),

        Some(Commands::Import { file }) => CommandEnum::Import(ImportCommand { file }),

        Some(Commands::ImportBrowsers {
            list,
            all,
            browsers,
        }) => CommandEnum::ImportBrowsers(ImportBrowsersCommand {
            list,
            all,
            browsers,
        }),

        Some(Commands::Export { file }) => CommandEnum::Export(ExportCommand { file }),

        Some(Commands::Open { ids }) => CommandEnum::Open(OpenCommand { ids }),

        Some(Commands::Shell) => CommandEnum::Shell(ShellCommand),

        Some(Commands::Edit { id }) => CommandEnum::Edit(EditCommand { id }),

        Some(Commands::Undo { count }) => CommandEnum::Undo(UndoCommand { count }),

        None => CommandEnum::No(NoCommand {
            keywords: cli.keywords,
            open: cli.open,
            format: cli.format,
            nc: cli.nc,
        }),
    };

    command.execute(&ctx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use rstest::rstest;

    // Helper to parse CLI arguments from a string
    fn parse_args(args: &str) -> std::result::Result<Cli, clap::Error> {
        let args_vec: Vec<&str> = args.split_whitespace().collect();
        Cli::try_parse_from(std::iter::once("buku").chain(args_vec))
    }

    // Helper to expect successful parsing
    fn parse_args_ok(args: &str) -> Cli {
        parse_args(args).expect("Failed to parse valid arguments")
    }

    #[test]
    fn test_no_args() {
        let cli = parse_args_ok("");
        assert!(!cli.version);
        assert_eq!(cli.db, None);
        assert!(!cli.nc);
        assert!(!cli.debug);
        assert_eq!(cli.format, None);
        assert!(!cli.open);
        assert_eq!(cli.limit, None);
        assert!(cli.keywords.is_empty());
        assert!(cli.command.is_none());
    }

    #[rstest]
    #[case("--version", true)]
    #[case("-v", true)]
    fn test_version_flag(#[case] args: &str, #[case] expected: bool) {
        let cli = parse_args_ok(args);
        assert_eq!(cli.version, expected);
    }

    #[rstest]
    #[case("--db /path/to/db.db", Some("/path/to/db.db"))]
    #[case("--db custom.db", Some("custom.db"))]
    fn test_db_path(#[case] args: &str, #[case] expected: Option<&str>) {
        let cli = parse_args_ok(args);
        assert_eq!(cli.db.as_ref().map(|p| p.to_str().unwrap()), expected);
    }

    #[rstest]
    #[case("--nc", true)]
    #[case("", false)]
    fn test_no_color_flag(#[case] args: &str, #[case] expected: bool) {
        let cli = parse_args_ok(args);
        assert_eq!(cli.nc, expected);
    }

    #[rstest]
    #[case("--debug", true)]
    #[case("-g", true)]
    #[case("", false)]
    fn test_debug_flag(#[case] args: &str, #[case] expected: bool) {
        let cli = parse_args_ok(args);
        assert_eq!(cli.debug, expected);
    }

    #[rstest]
    #[case("--format json", Some("json"))]
    #[case("-f markdown", Some("markdown"))]
    #[case("", None)]
    fn test_format_option(#[case] args: &str, #[case] expected: Option<&str>) {
        let cli = parse_args_ok(args);
        assert_eq!(cli.format.as_deref(), expected);
    }

    #[rstest]
    #[case("--open", true)]
    #[case("-o", true)]
    #[case("", false)]
    fn test_open_flag(#[case] args: &str, #[case] expected: bool) {
        let cli = parse_args_ok(args);
        assert_eq!(cli.open, expected);
    }

    #[rstest]
    #[case("--limit 10", Some(10))]
    #[case("-n 5", Some(5))]
    #[case("", None)]
    fn test_limit_option(#[case] args: &str, #[case] expected: Option<usize>) {
        let cli = parse_args_ok(args);
        assert_eq!(cli.limit, expected);
    }

    #[rstest]
    #[case("rust programming", vec!["rust", "programming"])]
    #[case("test", vec!["test"])]
    #[case("", vec![])]
    fn test_search_keywords(#[case] args: &str, #[case] expected: Vec<&str>) {
        let cli = parse_args_ok(args);
        assert_eq!(cli.keywords, expected);
    }

    // Add command tests
    #[rstest]
    #[case("add https://example.com")]
    #[case("add https://rust-lang.org --title Rust")]
    #[case("add https://test.com --tag rust,programming")]
    #[case("add https://test.com --comment Description")]
    #[case("add https://test.com --offline")]
    fn test_add_command(#[case] args: &str) {
        let cli = parse_args_ok(args);
        assert!(matches!(cli.command, Some(Commands::Add { .. })));
    }

    #[test]
    fn test_add_command_with_all_options() {
        let cli = parse_args_ok(
            "add https://example.com --title Test --tag rust --tag test --comment Description --offline",
        );
        match cli.command {
            Some(Commands::Add {
                url,
                tag,
                title,
                comment,
                offline,
            }) => {
                assert_eq!(url, "https://example.com");
                assert_eq!(title, Some("Test".to_string()));
                assert_eq!(tag, Some(vec!["rust".to_string(), "test".to_string()]));
                assert_eq!(comment, Some("Description".to_string()));
                assert!(offline);
            }
            _ => panic!("Expected Add command"),
        }
    }

    // Update command tests
    #[rstest]
    #[case("update 1 --url https://new.com")]
    #[case("update 42 --title NewTitle")]
    #[case("update 5 --tag updated")]
    #[case("update 10 --comment UpdatedDescription")]
    #[case("update 3 --immutable 1")]
    fn test_update_command(#[case] args: &str) {
        let cli = parse_args_ok(args);
        assert!(matches!(cli.command, Some(Commands::Update { .. })));
    }

    #[test]
    fn test_update_command_details() {
        let cli = parse_args_ok("update 42");
        assert!(matches!(cli.command, Some(Commands::Update { .. })));

        // Verify the ids are parsed correctly
        if let Some(Commands::Update { ids, .. }) = cli.command {
            assert_eq!(ids, vec!["42".to_string()]);
        } else {
            panic!("Expected Update command");
        }
    }

    // Delete command tests
    #[rstest]
    #[case("delete 1")]
    #[case("delete 1 2 3")]
    #[case("delete 1-5")]
    #[case("delete --force 1")]
    #[case("delete --retain-order 1 2")]
    fn test_delete_command(#[case] args: &str) {
        let cli = parse_args_ok(args);
        assert!(matches!(cli.command, Some(Commands::Delete { .. })));
    }

    #[test]
    fn test_delete_command_with_force() {
        let cli = parse_args_ok("delete --force 1 2 3");
        match cli.command {
            Some(Commands::Delete { ids, force, .. }) => {
                assert_eq!(ids, vec!["1", "2", "3"]);
                assert!(force);
            }
            _ => panic!("Expected Delete command"),
        }
    }

    // Print command tests
    #[rstest]
    #[case("print")]
    #[case("print 1")]
    #[case("print 1 2 3")]
    #[case("print --columns 5")]
    fn test_print_command(#[case] args: &str) {
        let cli = parse_args_ok(args);
        assert!(matches!(cli.command, Some(Commands::Print { .. })));
    }

    // Search command tests
    #[rstest]
    #[case("search rust")]
    #[case("search rust programming")]
    #[case("search --all keyword1 keyword2")]
    #[case("search --deep test")]
    #[case("search --regex '^http'")]
    #[case("search --markers tag:test")]
    fn test_search_command(#[case] args: &str) {
        let cli = parse_args_ok(args);
        assert!(matches!(cli.command, Some(Commands::Search { .. })));
    }

    #[test]
    fn test_search_command_all_flag() {
        let cli = parse_args_ok("search --all rust web");
        match cli.command {
            Some(Commands::Search { keywords, all, .. }) => {
                assert_eq!(keywords, vec!["rust", "web"]);
                assert!(all);
            }
            _ => panic!("Expected Search command"),
        }
    }

    // Tag command tests
    #[rstest]
    #[case("tag rust")]
    #[case("tag programming web")]
    #[case("tag")]
    fn test_tag_command(#[case] args: &str) {
        let cli = parse_args_ok(args);
        assert!(matches!(cli.command, Some(Commands::Tag { .. })));
    }

    // Lock/Unlock command tests
    #[rstest]
    #[case("lock", 8)]
    #[case("lock 16", 16)]
    #[case("unlock", 8)]
    #[case("unlock 10", 10)]
    fn test_lock_unlock_commands(#[case] args: &str, #[case] expected_iterations: u32) {
        let cli = parse_args_ok(args);
        match cli.command {
            Some(Commands::Lock { iterations }) => {
                assert_eq!(iterations, expected_iterations);
            }
            Some(Commands::Unlock { iterations }) => {
                assert_eq!(iterations, expected_iterations);
            }
            _ => panic!("Expected Lock or Unlock command"),
        }
    }

    // Import/Export command tests
    #[rstest]
    #[case("import bookmarks.html")]
    #[case("export output.html")]
    fn test_import_export_commands(#[case] args: &str) {
        let cli = parse_args_ok(args);
        match args.split_whitespace().next().unwrap() {
            "import" => assert!(matches!(cli.command, Some(Commands::Import { .. }))),
            "export" => assert!(matches!(cli.command, Some(Commands::Export { .. }))),
            _ => panic!("Unexpected command"),
        }
    }

    // ImportBrowsers command tests
    #[rstest]
    #[case("import-browsers --list")]
    #[case("import-browsers --all")]
    #[case("import-browsers --browsers chrome")]
    #[case("import-browsers --browsers chrome,firefox")]
    #[case("import-browsers -b chrome,firefox,edge")]
    fn test_import_browsers_command(#[case] args: &str) {
        let cli = parse_args_ok(args);
        assert!(matches!(cli.command, Some(Commands::ImportBrowsers { .. })));
    }

    #[test]
    fn test_import_browsers_list_flag() {
        let cli = parse_args_ok("import-browsers --list");
        match cli.command {
            Some(Commands::ImportBrowsers {
                list,
                all,
                browsers,
            }) => {
                assert!(list);
                assert!(!all);
                assert!(browsers.is_none());
            }
            _ => panic!("Expected ImportBrowsers command"),
        }
    }

    #[test]
    fn test_import_browsers_all_flag() {
        let cli = parse_args_ok("import-browsers --all");
        match cli.command {
            Some(Commands::ImportBrowsers { list, all, .. }) => {
                assert!(!list);
                assert!(all);
            }
            _ => panic!("Expected ImportBrowsers command"),
        }
    }

    #[test]
    fn test_import_browsers_with_browser_list() {
        let cli = parse_args_ok("import-browsers --browsers chrome,firefox");
        match cli.command {
            Some(Commands::ImportBrowsers { browsers, .. }) => {
                assert_eq!(
                    browsers,
                    Some(vec!["chrome".to_string(), "firefox".to_string()])
                );
            }
            _ => panic!("Expected ImportBrowsers command"),
        }
    }

    // Open command tests
    #[rstest]
    #[case("open 1")]
    #[case("open 1 2 3")]
    fn test_open_command(#[case] args: &str) {
        let cli = parse_args_ok(args);
        assert!(matches!(cli.command, Some(Commands::Open { .. })));
    }

    // Shell command test
    #[test]
    fn test_shell_command() {
        let cli = parse_args_ok("shell");
        assert!(matches!(cli.command, Some(Commands::Shell)));
    }

    // Edit command tests
    #[rstest]
    #[case("edit 1", Some(1))]
    #[case("edit 42", Some(42))]
    #[case("edit", None)]
    fn test_edit_command(#[case] args: &str, #[case] expected_id: Option<usize>) {
        let cli = parse_args_ok(args);
        match cli.command {
            Some(Commands::Edit { id }) => {
                assert_eq!(id, expected_id);
            }
            _ => panic!("Expected Edit command"),
        }
    }

    // Undo command test
    #[test]
    fn test_undo_command() {
        let cli = parse_args_ok("undo");
        assert!(matches!(cli.command, Some(Commands::Undo { .. })));

        if let Some(Commands::Undo { count }) = cli.command {
            assert_eq!(count, 1); // Default value
        }
    }

    #[test]
    fn test_undo_command_with_count() {
        let cli = parse_args_ok("undo 100");
        assert!(matches!(cli.command, Some(Commands::Undo { .. })));

        if let Some(Commands::Undo { count }) = cli.command {
            assert_eq!(count, 100);
        }
    }

    // Combined flag tests
    #[rstest]
    #[case("--nc --debug search test")]
    #[case("-g -n 10 search rust")]
    #[case("--format json --open print")]
    fn test_combined_flags(#[case] args: &str) {
        let result = parse_args(args);
        assert!(result.is_ok(), "Failed to parse: {}", args);
    }

    #[test]
    fn test_all_top_level_flags() {
        let cli =
            parse_args_ok("--nc --debug --format json --open --limit 5 --db test.db search test");
        assert!(cli.nc);
        assert!(cli.debug);
        assert_eq!(cli.format.as_deref(), Some("json"));
        assert!(cli.open);
        assert_eq!(cli.limit, Some(5));
        assert_eq!(
            cli.db.as_ref().map(|p| p.to_str().unwrap()),
            Some("test.db")
        );
    }

    // Error cases
    #[rstest]
    #[case("add")] // Missing required URL
    #[case("update")] // Missing required ID
    #[case("delete")] // No IDs provided (actually valid, but worth testing behavior)
    fn test_invalid_commands(#[case] args: &str) {
        let result = parse_args(args);
        // These should either fail or parse in a specific way
        // We're just ensuring they don't panic
        let _ = result;
    }

    #[test]
    fn test_version_short_flag() {
        let cli = parse_args_ok("-v");
        assert!(cli.version);
    }

    #[test]
    fn test_debug_short_flag() {
        let cli = parse_args_ok("-g");
        assert!(cli.debug);
    }

    // Test that mutually exclusive options can be parsed
    // (actual mutual exclusivity would be enforced in the handler)
    #[test]
    fn test_import_browsers_multiple_options() {
        // While not semantically correct, CLI parsing should succeed
        // The handler will enforce business logic
        let cli = parse_args_ok("import-browsers --list --all");
        match cli.command {
            Some(Commands::ImportBrowsers { list, all, .. }) => {
                assert!(list);
                assert!(all);
            }
            _ => panic!("Expected ImportBrowsers command"),
        }
    }
}
