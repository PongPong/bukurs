use std::path::PathBuf;


pub fn get_default_dbdir() -> PathBuf {
    if let Ok(path) = std::env::var("BUKU_DEFAULT_DBDIR") {
        return PathBuf::from(path);
    }

    if let Ok(path) = std::env::var("XDG_DATA_HOME") {
        return PathBuf::from(path).join("buku");
    }

    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".local/share/buku");
    }

    #[cfg(target_os = "windows")]
    if let Ok(appdata) = std::env::var("APPDATA") {
        return PathBuf::from(appdata).join("buku");
    }

    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}
