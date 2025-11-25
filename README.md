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
cargo install --path .
```

Or build from source:

```bash
cargo build --release
```

The binary will be in `target/release/buku`.

## Usage

### Quick Start

```bash
# Start interactive mode (no arguments)
buku

# Add a bookmark
buku add https://example.com --tag rust,cli

# Search for bookmarks (positional keywords)
buku rust programming

# List all bookmarks
buku print
```

### Subcommands

```bash
buku add <URL>           # Add a new bookmark
buku update <ID>         # Update an existing bookmark
buku delete <ID>         # Delete a bookmark
buku print               # List all bookmarks
buku search <KEYWORDS>   # Search bookmarks
buku tag <TAGS>          # Search by tags
buku undo [COUNT]        # Undo last operation(s)
buku lock [ITERATIONS]   # Encrypt database
buku unlock [ITERATIONS] # Decrypt database
buku import <FILE>       # Import bookmarks
buku export <FILE>       # Export bookmarks
buku open <ID>           # Open bookmark in browser
buku interactive         # Start interactive mode
```

### Search Examples

#### Normal Search

Search using positional keywords (any match by default):

```bash
# Search for bookmarks containing "rust" OR "programming"
buku rust programming

# Same as above using explicit search subcommand
buku search rust programming

# Search for bookmarks containing ALL keywords
buku search rust programming --all

# Deep search (match substrings)
buku search rust --deep

# Regex search
buku search "rust|python" --regex
```

#### Searching for Subcommand Names

If you want to search for keywords that match subcommand names (like "add", "update", "delete"), you have two options:

**Option 1: Use the explicit `search` subcommand** (Recommended)
```bash
# Search for bookmarks containing "add"
buku search add

# Search for "update" and "delete"
buku search update delete
```

**Option 2: Use the `--` delimiter** (Unix convention)
```bash
# Search for "add" using -- delimiter
buku -- add

# Search for multiple keywords including subcommand names
buku -- add update delete
```

The `--` tells the parser that everything after it should be treated as arguments, not subcommands.

### Add Bookmarks

```bash
# Add with automatic metadata fetching
buku add https://example.com

# Add with custom title and tags
buku add https://example.com --title "Example Site" --tag rust,web

# Add with description
buku add https://example.com --comment "A great example site"

# Add without fetching metadata (offline)
buku add https://example.com --offline
```

### Update Bookmarks

```bash
# Update title
buku update 1 --title "New Title"

# Update URL and tags
buku update 1 --url https://newurl.com --tag rust,updated

# Update description
buku update 1 --comment "Updated description"

# Refresh metadata from web (no options = refresh)
buku update 1

# Refresh multiple bookmarks
buku update 1-10
buku update "*"  # Refresh all bookmarks
```

### Tag Operations

The `--tag` option supports powerful tag manipulation with prefix operators:

#### Add Tags (`+` prefix)
Add tags without removing existing ones:

```bash
# Add a single tag
buku update 1 --tag +urgent

# Add multiple tags
buku update 1 --tag +urgent,+todo

# Add tags to multiple bookmarks
buku update 1-5 --tag +reviewed
```

#### Remove Tags (`-` prefix)
Remove specific tags:

```bash
# Remove a single tag
buku update 1 --tag -archived

# Remove multiple tags
buku update 1 --tag -old,-deprecated

# Remove tags from multiple bookmarks
buku update 1-10 --tag -draft
```

#### Replace Tags (`~` prefix)
Replace one tag with another:

```bash
# Replace 'todo' with 'done'
buku update 1 --tag ~todo:done

# Replace 'draft' with 'published'
buku update 1 --tag ~draft:published
```

#### Combine Operations
You can combine different tag operations in a single command:

```bash
# Add 'urgent', remove 'archived', and replace 'todo' with 'done'
buku update 1 --tag +urgent,-archived,~todo:done

# Works with multiple bookmarks too
buku update 1-100 --tag +reviewed,-draft
```

#### Plain Tags (No Prefix)
Tags without a prefix are added by default:

```bash
# These are equivalent
buku update 1 --tag newtag
buku update 1 --tag +newtag
```

#### Batch Updates with Single Undo
When updating multiple bookmarks without tag operations, changes are batched for efficiency:

```bash
# Update 100 bookmarks - single undo reverts all
buku update 1-100 --title "Reviewed"

# Undo once to revert all 100 changes
buku undo
```

### Delete Bookmarks

```bash
# Delete bookmark by ID
buku delete 5

# Delete with preserved order
buku delete 5 --retain-order
```

### Undo Operations

Undo recent changes to your bookmarks:

```bash
# Undo the last operation
buku undo

# Undo the last 5 operations
buku undo 5
```

**Batch Undo Support**: When you update multiple bookmarks in a single command (without tag operations), all changes are grouped together. A single `undo` command will revert all of them:

```bash
# Update 100 bookmarks
buku update 1-100 --title "Reviewed"

# One undo reverts all 100 changes
buku undo
# Output: ✓ Undid batch UPDATE: 100 bookmark(s) reverted
```

Supported operations:
- **ADD**: Undoing an add removes the bookmark
- **UPDATE**: Undoing an update restores the previous values
- **DELETE**: Undoing a delete restores the bookmark

### Encryption

```bash
# Encrypt database with 8 iterations (default)
buku lock

# Encrypt with custom iterations
buku lock 16

# Decrypt database
buku unlock

# Decrypt with custom iterations
buku unlock 16
```

### Import/Export

```bash
# Export to HTML
buku export bookmarks.html

# Import from HTML
buku import bookmarks.html
```

### Interactive Mode

Launch interactive mode to browse and search bookmarks:

```bash
buku
# or explicitly
buku interactive
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
buku --db /path/to/custom.db print
```

## License

This project maintains compatibility with the original buku license.

## Credits

This is a Rust port of the excellent [buku](https://github.com/jarun/buku) bookmark manager by Arun Prakash Jana.
