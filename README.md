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

## License

This project maintains compatibility with the original buku license.

## Credits

This is a Rust port of the excellent [buku](https://github.com/jarun/buku) bookmark manager by Arun Prakash Jana.
