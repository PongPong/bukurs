use super::import::BookmarkImporter;
use crate::db::BukuDb;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

/// Detected browser type
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BrowserType {
    Chrome,
    Firefox,
    Edge,
    Safari,
}

impl BrowserType {
    /// Get a user-friendly display name for the browser
    pub fn display_name(&self) -> &str {
        match self {
            BrowserType::Chrome => "Chrome",
            BrowserType::Firefox => "Firefox",
            BrowserType::Edge => "Edge",
            BrowserType::Safari => "Safari",
        }
    }

    /// Parse browser type from string (case-insensitive)
    pub fn from_string(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "chrome" => Some(BrowserType::Chrome),
            "firefox" => Some(BrowserType::Firefox),
            "edge" => Some(BrowserType::Edge),
            "safari" => Some(BrowserType::Safari),
            _ => None,
        }
    }
}

/// Browser profile location
#[derive(Debug, Clone)]
pub struct BrowserProfile {
    pub browser: BrowserType,
    pub profile_name: String,
    pub path: PathBuf,
}

impl BrowserProfile {
    pub fn display_string(&self) -> String {
        format!("{} ({})", self.browser.display_name(), self.profile_name)
    }
}

/// Chrome bookmark structure (JSON)
#[derive(Debug, Deserialize, Serialize)]
struct ChromeBookmark {
    #[serde(rename = "type")]
    node_type: String,
    name: Option<String>,
    url: Option<String>,
    children: Option<Vec<ChromeBookmark>>,
}

#[derive(Debug, Deserialize)]
struct ChromeBookmarkFile {
    roots: ChromeRoots,
}

#[derive(Debug, Deserialize)]
struct ChromeRoots {
    bookmark_bar: ChromeBookmark,
    other: ChromeBookmark,
    synced: Option<ChromeBookmark>,
}

/// Detect installed browsers and their profile locations
pub fn detect_browsers() -> Vec<BrowserProfile> {
    let mut profiles = Vec::new();

    // Detect all Chrome profiles
    profiles.extend(detect_all_chrome_profiles());

    // Detect all Firefox profiles
    profiles.extend(detect_all_firefox_profiles());

    // Detect all Edge profiles
    profiles.extend(detect_all_edge_profiles());

    profiles
}

/// Detect all Chrome profile locations
fn detect_all_chrome_profiles() -> Vec<BrowserProfile> {
    let mut profiles = Vec::new();
    let home = match std::env::var("HOME") {
        Ok(h) => h,
        Err(_) => return profiles,
    };

    #[cfg(target_os = "macos")]
    let chrome_base = format!("{}/Library/Application Support/Google/Chrome", home);

    #[cfg(target_os = "linux")]
    let chrome_base = format!("{}/.config/google-chrome", home);

    #[cfg(target_os = "windows")]
    let chrome_base = format!("{}\\AppData\\Local\\Google\\Chrome\\User Data", home);

    let base_path = PathBuf::from(&chrome_base);
    if !base_path.exists() {
        return profiles;
    }

    // Common profile directories to check
    let profile_names = vec![
        "Default",
        "Profile 1",
        "Profile 2",
        "Profile 3",
        "Profile 4",
    ];

    for profile_name in profile_names {
        let bookmarks_path = base_path.join(profile_name).join("Bookmarks");
        if bookmarks_path.exists() {
            profiles.push(BrowserProfile {
                browser: BrowserType::Chrome,
                profile_name: profile_name.to_string(),
                path: bookmarks_path,
            });
        }
    }

    // Also check for Chromium on Linux
    #[cfg(target_os = "linux")]
    {
        let chromium_base = format!("{}/.config/chromium", home);
        let chromium_path = PathBuf::from(&chromium_base);
        if chromium_path.exists() {
            for profile_name in &profile_names {
                let bookmarks_path = chromium_path.join(profile_name).join("Bookmarks");
                if bookmarks_path.exists() {
                    profiles.push(BrowserProfile {
                        browser: BrowserType::Chrome,
                        profile_name: format!("Chromium {}", profile_name),
                        path: bookmarks_path,
                    });
                }
            }
        }
    }

    profiles
}

/// Detect all Firefox profile locations
fn detect_all_firefox_profiles() -> Vec<BrowserProfile> {
    let mut profiles = Vec::new();
    let home = match std::env::var("HOME") {
        Ok(h) => h,
        Err(_) => return profiles,
    };

    #[cfg(target_os = "macos")]
    let firefox_base = format!("{}/Library/Application Support/Firefox/Profiles", home);

    #[cfg(target_os = "linux")]
    let firefox_base = format!("{}/.mozilla/firefox", home);

    #[cfg(target_os = "windows")]
    let firefox_base = format!("{}\\AppData\\Roaming\\Mozilla\\Firefox\\Profiles", home);

    let base_path = PathBuf::from(firefox_base);
    if !base_path.exists() {
        return profiles;
    }

    // Find all profile directories with places.sqlite
    if let Ok(entries) = fs::read_dir(&base_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let places = path.join("places.sqlite");
                if places.exists() {
                    let profile_name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("Unknown")
                        .to_string();
                    profiles.push(BrowserProfile {
                        browser: BrowserType::Firefox,
                        profile_name,
                        path: places,
                    });
                }
            }
        }
    }

    profiles
}

/// Detect all Edge profile locations (uses Chrome format)
fn detect_all_edge_profiles() -> Vec<BrowserProfile> {
    let mut profiles = Vec::new();
    let home = match std::env::var("HOME") {
        Ok(h) => h,
        Err(_) => return profiles,
    };

    #[cfg(target_os = "macos")]
    let edge_base = format!("{}/Library/Application Support/Microsoft Edge", home);

    #[cfg(target_os = "linux")]
    let edge_base = format!("{}/.config/microsoft-edge", home);

    #[cfg(target_os = "windows")]
    let edge_base = format!("{}\\AppData\\Local\\Microsoft\\Edge\\User Data", home);

    let base_path = PathBuf::from(&edge_base);
    if !base_path.exists() {
        return profiles;
    }

    // Common profile directories to check
    let profile_names = vec![
        "Default",
        "Profile 1",
        "Profile 2",
        "Profile 3",
        "Profile 4",
    ];

    for profile_name in profile_names {
        let bookmarks_path = base_path.join(profile_name).join("Bookmarks");
        if bookmarks_path.exists() {
            profiles.push(BrowserProfile {
                browser: BrowserType::Edge,
                profile_name: profile_name.to_string(),
                path: bookmarks_path,
            });
        }
    }

    profiles
}

/// Chrome JSON bookmark importer
pub struct ChromeImporter;

impl super::import::BookmarkImporter for ChromeImporter {
    fn import(&self, db: &BukuDb, path: &Path) -> Result<usize, Box<dyn Error>> {
        let mut json_content = fs::read(path)?;
        let chrome_data: ChromeBookmarkFile = simd_json::serde::from_slice(&mut json_content)?;

        let mut imported_count = 0;

        // Import from bookmark bar
        imported_count +=
            import_chrome_folder(db, &chrome_data.roots.bookmark_bar, "bookmark_bar")?;

        // Import from other bookmarks
        imported_count += import_chrome_folder(db, &chrome_data.roots.other, "other")?;

        // Import from synced (if exists)
        if let Some(ref synced) = chrome_data.roots.synced {
            imported_count += import_chrome_folder(db, synced, "synced")?;
        }

        Ok(imported_count)
    }
}

fn import_chrome_folder(
    db: &BukuDb,
    folder: &ChromeBookmark,
    parent_tags: &str,
) -> Result<usize, Box<dyn Error>> {
    let mut count = 0;

    if let Some(ref children) = folder.children {
        for child in children {
            match child.node_type.as_str() {
                "url" => {
                    if let (Some(ref url), Some(ref name)) = (&child.url, &child.name) {
                        let tags = format!(",{},", parent_tags);
                        match db.add_rec(url, name, &tags, "") {
                            Ok(_) => count += 1,
                            Err(rusqlite::Error::SqliteFailure(err, _))
                                if err.code == rusqlite::ErrorCode::ConstraintViolation =>
                            {
                                // Skip duplicates
                                continue;
                            }
                            Err(e) => return Err(e.into()),
                        }
                    }
                }
                "folder" => {
                    if let Some(ref name) = child.name {
                        let new_tags = format!("{},{}", parent_tags, name);
                        count += import_chrome_folder(db, child, &new_tags)?;
                    }
                }
                _ => {}
            }
        }
    }

    Ok(count)
}

/// Firefox SQLite bookmark importer
pub struct FirefoxImporter;

impl super::import::BookmarkImporter for FirefoxImporter {
    fn import(&self, db: &BukuDb, path: &Path) -> Result<usize, Box<dyn Error>> {
        let conn = rusqlite::Connection::open(path)?;

        let mut stmt = conn.prepare(
            "SELECT moz_places.url, moz_bookmarks.title
             FROM moz_bookmarks
             JOIN moz_places ON moz_bookmarks.fk = moz_places.id
             WHERE moz_bookmarks.type = 1 AND moz_places.url IS NOT NULL",
        )?;

        let bookmarks = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
        })?;

        let mut count = 0;
        for bookmark_result in bookmarks {
            let (url, title_opt) = bookmark_result?;
            let title = title_opt.unwrap_or_else(|| url.clone());

            match db.add_rec(&url, &title, ",firefox,", "") {
                Ok(_) => count += 1,
                Err(rusqlite::Error::SqliteFailure(err, _))
                    if err.code == rusqlite::ErrorCode::ConstraintViolation =>
                {
                    // Skip duplicates
                    continue;
                }
                Err(e) => return Err(e.into()),
            }
        }

        Ok(count)
    }
}

/// Import bookmarks directly from Chrome JSON file
pub fn import_from_chrome(db: &BukuDb, bookmarks_path: &Path) -> Result<usize, Box<dyn Error>> {
    let importer = ChromeImporter;
    importer.import(db, bookmarks_path)
}

/// Import bookmarks directly from Firefox SQLite database
pub fn import_from_firefox(db: &BukuDb, places_path: &Path) -> Result<usize, Box<dyn Error>> {
    let importer = FirefoxImporter;
    importer.import(db, places_path)
}

/// Auto-import from all detected browsers
pub fn auto_import_all(db: &BukuDb) -> Result<usize, Box<dyn Error>> {
    let profiles = detect_browsers();
    let mut total_count = 0;

    for profile in profiles {
        let count = match profile.browser {
            BrowserType::Chrome | BrowserType::Edge => import_from_chrome(db, &profile.path)?,
            BrowserType::Firefox => import_from_firefox(db, &profile.path)?,
            BrowserType::Safari => {
                // Safari uses plist format - not implemented yet
                0
            }
        };

        eprintln!(
            "✓ Imported {} bookmarks from {}",
            count,
            profile.display_string()
        );
        total_count += count;
    }

    Ok(total_count)
}

/// List all detected browser profiles
pub fn list_detected_browsers() -> Vec<BrowserProfile> {
    detect_browsers()
}

/// Import bookmarks from selected browsers
pub fn import_from_selected_browsers(
    db: &BukuDb,
    browser_names: &[String],
) -> Result<usize, Box<dyn Error>> {
    let all_profiles = detect_browsers();

    // Parse browser names
    let requested_browsers: Vec<BrowserType> = browser_names
        .iter()
        .filter_map(|name| BrowserType::from_string(name))
        .collect();

    if requested_browsers.is_empty() {
        return Err("No valid browsers specified".into());
    }

    // Filter profiles by requested browsers
    let selected_profiles: Vec<_> = all_profiles
        .into_iter()
        .filter(|profile| requested_browsers.contains(&profile.browser))
        .collect();

    if selected_profiles.is_empty() {
        return Err("No matching browser profiles found".into());
    }

    let mut total_count = 0;
    for profile in selected_profiles {
        let count = match profile.browser {
            BrowserType::Chrome | BrowserType::Edge => import_from_chrome(db, &profile.path)?,
            BrowserType::Firefox => import_from_firefox(db, &profile.path)?,
            BrowserType::Safari => {
                // Safari uses plist format - not implemented yet
                0
            }
        };

        eprintln!(
            "✓ Imported {} bookmarks from {}",
            count,
            profile.display_string()
        );
        total_count += count;
    }

    Ok(total_count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_browsers() {
        // Just verify the function doesn't panic
        let browsers = detect_browsers();
        // On a system with browsers installed, this should find at least one
        println!("Detected browsers: {:?}", browsers);
        for browser in browsers {
            println!("  - {}", browser.display_string());
        }
    }

    #[test]
    fn test_browser_type_from_str() {
        assert_eq!(BrowserType::from_string("chrome"), Some(BrowserType::Chrome));
        assert_eq!(BrowserType::from_string("Chrome"), Some(BrowserType::Chrome));
        assert_eq!(BrowserType::from_string("CHROME"), Some(BrowserType::Chrome));
        assert_eq!(BrowserType::from_string("firefox"), Some(BrowserType::Firefox));
        assert_eq!(BrowserType::from_string("edge"), Some(BrowserType::Edge));
        assert_eq!(BrowserType::from_string("safari"), Some(BrowserType::Safari));
        assert_eq!(BrowserType::from_string("invalid"), None);
    }

    #[test]
    fn test_browser_type_display_name() {
        assert_eq!(BrowserType::Chrome.display_name(), "Chrome");
        assert_eq!(BrowserType::Firefox.display_name(), "Firefox");
        assert_eq!(BrowserType::Edge.display_name(), "Edge");
        assert_eq!(BrowserType::Safari.display_name(), "Safari");
    }

    #[test]
    fn test_chrome_import_parsing() {
        use crate::db::BukuDb;
        use std::io::Write;
        use tempfile::NamedTempFile;

        // Create a temporary database
        let db_file = NamedTempFile::new().unwrap();
        let db = BukuDb::init(db_file.path()).unwrap();

        // Create a sample Chrome bookmark JSON file
        let mut bookmark_file = NamedTempFile::new().unwrap();
        let json_content = r#"{
            "checksum": "e68417696614de65818e666d48227636",
            "roots": {
                "bookmark_bar": {
                    "children": [
                        {
                            "date_added": "13245678900000000",
                            "id": "1",
                            "name": "Google",
                            "type": "url",
                            "url": "https://www.google.com/"
                        },
                        {
                            "children": [
                                {
                                    "date_added": "13245678900000000",
                                    "id": "3",
                                    "name": "Rust",
                                    "type": "url",
                                    "url": "https://www.rust-lang.org/"
                                }
                            ],
                            "date_added": "13245678900000000",
                            "date_modified": "13245678900000000",
                            "id": "2",
                            "name": "Dev",
                            "type": "folder"
                        }
                    ],
                    "date_added": "13245678900000000",
                    "date_modified": "13245678900000000",
                    "id": "1",
                    "name": "Bookmarks Bar",
                    "type": "folder"
                },
                "other": {
                    "children": [],
                    "date_added": "13245678900000000",
                    "date_modified": "13245678900000000",
                    "id": "2",
                    "name": "Other Bookmarks",
                    "type": "folder"
                },
                "synced": {
                    "children": [],
                    "date_added": "13245678900000000",
                    "date_modified": "13245678900000000",
                    "id": "3",
                    "name": "Mobile Bookmarks",
                    "type": "folder"
                }
            },
            "version": 1
        }"#;

        write!(bookmark_file, "{}", json_content).unwrap();

        // Test import
        let count = import_from_chrome(&db, bookmark_file.path()).unwrap();
        assert_eq!(count, 2);

        // Verify bookmarks in DB
        let bookmarks = db.search(&[], false, false, false).unwrap();
        assert_eq!(bookmarks.len(), 2);

        let google = bookmarks
            .iter()
            .find(|b| b.url == "https://www.google.com/")
            .unwrap();
        assert_eq!(google.title, "Google");
        assert!(google.tags.contains(",bookmark_bar,"));

        let rust = bookmarks
            .iter()
            .find(|b| b.url == "https://www.rust-lang.org/")
            .unwrap();
        assert_eq!(rust.title, "Rust");
        assert!(rust.tags.contains(",bookmark_bar,Dev,"));
    }
}
