# bukurs

A Rust port of [buku](https://github.com/jarun/buku), the powerful command-line bookmark manager.

## Features

- 🔖 **Bookmark Management**: Add, update, delete, and organize bookmarks
- 🔍 **Powerful Search**: Full-text search with regex support
- 🔐 **Encryption**: Secure your bookmarks with AES-256 encryption
- 📥 **Import/Export**: Compatible with browser bookmark formats
- 💻 **Interactive Mode**: Browse and manage bookmarks interactively
- 🏷️ **Tag System**: Organize bookmarks with tags
- ⚡ **Fast**: Single binary with no runtime dependencies

## Installation

```bash
cargo install --path ./cli
```

Or build from source:

```bash
cargo build --release
```

The binary will be in `target/release/bukurs`.

## Usage

### Quick Start

```bash
# Start interactive mode (no arguments)
bukurs

# Add a bookmark
bukurs add https://example.com --tag rust,cli

# Search for bookmarks (positional keywords)
bukurs rust programming

# List all bookmarks
bukurs print
```

### Subcommands

```bash
bukurs add <URL>           # Add a new bookmark
bukurs update <ID>         # Update an existing bookmark
bukurs delete <ID>         # Delete a bookmark
bukurs print               # List all bookmarks
bukurs search <KEYWORDS>   # Search bookmarks
bukurs tag <TAGS>          # Search by tags
bukurs undo [COUNT]        # Undo last operation(s)
bukurs lock [ITERATIONS]   # Encrypt database
bukurs unlock [ITERATIONS] # Decrypt database
bukurs import <FILE>       # Import bookmarks
bukurs export <FILE>       # Export bookmarks
bukurs open <ID>           # Open bookmark in browser
bukurs interactive         # Start interactive mode
```

### Search Examples

#### Normal Search

Search using positional keywords (any match by default):

```bash
# Search for bookmarks containing "rust" OR "programming"
bukurs rust programming

# Same as above using explicit search subcommand
bukurs search rust programming

# Search for bookmarks containing ALL keywords
bukurs search rust programming --all

# Deep search (match substrings)
bukurs search rust --deep

# Regex search
bukurs search "rust|python" --regex
```

#### Searching for Subcommand Names

If you want to search for keywords that match subcommand names (like "add", "update", "delete"), you have two options:

**Option 1: Use the explicit `search` subcommand** (Recommended)
```bash
# Search for bookmarks containing "add"
bukurs search add

# Search for "update" and "delete"
bukurs search update delete
```

**Option 2: Use the `--` delimiter** (Unix convention)
```bash
# Search for "add" using -- delimiter
bukurs -- add

# Search for multiple keywords including subcommand names
bukurs -- add update delete
```

The `--` tells the parser that everything after it should be treated as arguments, not subcommands.

### Add Bookmarks

```bash
# Add with automatic metadata fetching
bukurs add https://example.com

# Add with custom title and tags
bukurs add https://example.com --title "Example Site" --tag rust,web

# Add with description
bukurs add https://example.com --comment "A great example site"

# Add without fetching metadata (offline)
bukurs add https://example.com --offline
```

### Update Bookmarks

```bash
# Update title
bukurs update 1 --title "New Title"

# Update URL and tags
bukurs update 1 --url https://newurl.com --tag rust,updated

# Update description
bukurs update 1 --comment "Updated description"

# Refresh metadata from web (no options = refresh)
bukurs update 1

# Refresh multiple bookmarks
bukurs update 1-10
bukurs update "*"  # Refresh all bookmarks
```

### Tag Operations

The `--tag` option supports powerful tag manipulation with prefix operators:

#### Add Tags (`+` prefix)
Add tags without removing existing ones:

```bash
# Add a single tag
bukurs update 1 --tag=+urgent

# Add multiple tags
bukurs update 1 --tag=+urgent,+todo

# Add tags to multiple bookmarks
bukurs update 1-5 --tag=+reviewed
```

#### Remove Tags (`-` prefix)
Remove specific tags:

```bash
# Remove a single tag
bukurs update 1 --tag=-archived

# Remove multiple tags
bukurs update 1 --tag=-old,-deprecated

# Remove tags from multiple bookmarks
bukurs update 1-10 --tag=-draft
```

#### Replace Tags (`~` prefix)
Replace one tag with another:

```bash
# Replace 'todo' with 'done'
bukurs update 1 --tag=~todo:done

# Replace 'draft' with 'published'
bukurs update 1 --tag=~draft:published
```

#### Combine Operations
You can combine different tag operations in a single command:

```bash
# Add 'urgent', remove 'archived', and replace 'todo' with 'done'
bukurs update 1 --tag=+urgent,-archived,~todo:done

# Works with multiple bookmarks too
bukurs update 1-100 --tag=+reviewed,-draft
```

#### Plain Tags (No Prefix)
Tags without a prefix are added by default:

```bash
# These are equivalent
bukurs update 1 --tag=newtag
bukurs update 1 --tag=+newtag
```

#### Batch Updates with Single Undo
When updating multiple bookmarks without tag operations, changes are batched for efficiency:

```bash
# Update 100 bookmarks - single undo reverts all
bukurs update 1-100 --title "Reviewed"

# Undo once to revert all 100 changes
bukurs undo
```

### Delete Bookmarks

```bash
# Delete bookmark by ID
bukurs delete 5

# Delete with preserved order
bukurs delete 5 --retain-order
```

### Undo Operations

Undo recent changes to your bookmarks:

```bash
# Undo the last operation
bukurs undo

# Undo the last 5 operations
bukurs undo 5
```

**Batch Undo Support**: When you update multiple bookmarks in a single command (without tag operations), all changes are grouped together. A single `undo` command will revert all of them:

```bash
# Update 100 bookmarks
bukurs update 1-100 --title "Reviewed"

# One undo reverts all 100 changes
bukurs undo
# Output: ✓ Undid batch UPDATE: 100 bookmark(s) reverted
```

Supported operations:
- **ADD**: Undoing an add removes the bookmark
- **UPDATE**: Undoing an update restores the previous values
- **DELETE**: Undoing a delete restores the bookmark

### Encryption

```bash
# Encrypt database with 8 iterations (default)
bukurs lock

# Encrypt with custom iterations
bukurs lock 16

# Decrypt database
bukurs unlock

# Decrypt with custom iterations
bukurs unlock 16
```

### Import/Export

```bash
# Export to HTML
bukurs export bookmarks.html

# Import from HTML
bukurs import bookmarks.html
```

### Interactive Mode

Launch interactive mode to browse and search bookmarks:

```bash
bukurs
# or explicitly
bukurs interactive
```

Interactive commands:
- `?` or `help` - Show help
- `s keyword ...` - Search with ANY keyword
- `S keyword ...` - Search with ALL keywords
- `p id|range` - Print bookmarks
- `q`, `quit`, `exit`, or `^D` - Quit

### Global Options

```bash
--db <PATH>      # Use custom database location
--nc             # Disable color output
--debug          # Show debug information
--version        # Show version
```

## Database Location

By default, bookmarks are stored in:
- **Linux/macOS**: `~/.local/share/buku/bookmarks.db`
- **Windows**: `%APPDATA%\buku\bookmarks.db`

You can specify a custom location with `--db`:

```bash
bukurs --db /path/to/custom.db print
```

## Plugins

bukurs includes a powerful plugin system that extends functionality automatically. Plugins run in the background - you just use bukurs normally and they enhance your workflow.

### What Plugins Do

| When you... | What plugins add |
|-------------|------------------|
| `bukurs add https://github.com/foo/bar` | Auto-adds `github,code` tags, fetches title, adds `unread` tag, checks for duplicates |
| `bukurs add https://docs.rs/serde` | Auto-adds `rust,docs` tags |
| `bukurs open 1` | Warns if link is dead (404), marks as read |
| `bukurs add --tag private https://secret.com` | Hides from searches unless unlocked |

### Built-in Plugins

| Plugin | Default | Description |
|--------|---------|-------------|
| **auto_tagger** | ✅ On | Auto-tags based on URL domain (github, youtube, stackoverflow, etc.) |
| **title_fetcher** | ✅ On | Fetches page title if not provided |
| **tag_suggester** | ✅ On | Suggests tags from title/URL keywords (rust, python, tutorial, docs) |
| **reading_list** | ✅ On | Adds `unread` tag to new bookmarks, tracks read status |
| **duplicate_checker** | ✅ On | Warns about similar URLs |
| **url_validator** | ✅ On | Validates URLs before adding |
| **dead_link_checker** | ✅ On | Warns on `open` if URL returns 404 |
| **statistics** | ✅ On | Tracks bookmark statistics |
| **backup** | ✅ On | Auto-exports to JSON every 10 changes |
| **csv_format** | ✅ On | CSV output format support |
| **private_bookmarks** | ✅ On | Password-protects bookmarks tagged `private` |
| **webhook** | Off | Sends HTTP notifications (needs config) |
| **archive** | Off | Saves to archive.org on add |
| **expiry** | Off | Auto-tags bookmarks as `expired` after N days |
| **rss_feed** | Off | Generates RSS/Atom feed of bookmarks |

### Disabling Plugins

```bash
# Run without any plugins
bukurs --no-plugins add https://example.com
```

### Creating a Plugin

Plugins are single Rust files in `cli/src/plugins/`. They're automatically discovered at build time.

#### 1. Create the plugin file

Create `cli/src/plugins/my_plugin.rs`:

```rust
use bukurs::models::bookmark::Bookmark;
use bukurs::plugin::{Plugin, PluginInfo, HookResult, PluginContext};

pub struct MyPlugin {
    enabled: bool,
}

impl MyPlugin {
    pub fn new() -> Self {
        Self { enabled: true }
    }
}

impl Plugin for MyPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "my-plugin".to_string(),
            version: "1.0.0".to_string(),
            description: "Does something cool".to_string(),
            author: "your-name".to_string(),
        }
    }

    // Called when plugin loads - read config here
    fn on_load(&mut self, ctx: &PluginContext) -> HookResult {
        if let Some(enabled) = ctx.config.get("enabled") {
            self.enabled = enabled != "false";
        }
        HookResult::Continue
    }

    // Called before a bookmark is added - can modify the bookmark
    fn on_pre_add(&self, _ctx: &PluginContext, bookmark: &mut Bookmark) -> HookResult {
        if !self.enabled {
            return HookResult::Continue;
        }

        // Example: Add a custom tag
        if bookmark.url.contains("example.com") {
            bookmark.tags = format!("{},example", bookmark.tags.trim_matches(','));
        }

        HookResult::Continue
    }

    // Called after a bookmark is added
    fn on_post_add(&self, _ctx: &PluginContext, bookmark: &Bookmark) -> HookResult {
        log::info!("Bookmark added: {}", bookmark.title);
        HookResult::Continue
    }
}

// Required for auto-discovery
pub fn create_plugin() -> Box<dyn Plugin> {
    Box::new(MyPlugin::new())
}
```

#### 2. Build

```bash
cargo build --release
```

That's it! The plugin is automatically registered.

#### Available Hooks

| Hook | When it runs | Can modify |
|------|--------------|------------|
| `on_load` | Plugin initialization | Plugin config |
| `on_unload` | Plugin shutdown | - |
| `on_pre_add` | Before adding bookmark | Bookmark |
| `on_post_add` | After adding bookmark | - |
| `on_pre_update` | Before updating | New bookmark |
| `on_post_update` | After updating | - |
| `on_pre_delete` | Before deleting | - |
| `on_post_delete` | After deleting | - |
| `on_pre_search` | Before search | Search query |
| `on_post_search` | After search | Results |
| `on_pre_open` | Before opening URL | - |
| `on_pre_import` | Before import | Bookmark list |
| `on_post_import` | After import | - |
| `on_pre_export` | Before export | Bookmark list |

#### Hook Return Values

```rust
HookResult::Continue  // Continue with operation
HookResult::Skip      // Skip this operation (pre-hooks only)
HookResult::Error(msg) // Stop and return error
```

#### Plugin Context

Plugins receive a `PluginContext` with:

```rust
ctx.db_path    // Path to database file
ctx.data_dir   // Plugin's data directory for persistence
ctx.config     // HashMap<String, String> of plugin config
```

## License

This project maintains compatibility with the original buku license.

## Credits

This is a Rust port of the excellent [buku](https://github.com/jarun/buku) bookmark manager by Arun Prakash Jana.
