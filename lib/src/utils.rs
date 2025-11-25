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

pub fn get_config_dir() -> PathBuf {
    if let Ok(path) = std::env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(path).join("bukurs");
    }

    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".config/bukurs");
    }

    #[cfg(target_os = "windows")]
    if let Ok(appdata) = std::env::var("APPDATA") {
        return PathBuf::from(appdata).join("bukurs");
    }

    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// the builtin trim_start functions are not SIMD optimized, so we implement our own
/// to trim the start using SIMD optimization
/// unlike the builtin one, only ascii spaces and tabs are trimmed, other unicode whitespace are
/// preserved
#[inline]
pub fn trim_start_simd(s: &str) -> &str {
    let bytes = s.as_bytes();

    if bytes.is_empty() || (bytes[0] != b' ' && bytes[0] != b'\t') {
        return s;
    }

    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        if b != b' ' && b != b'\t' {
            return &s[i..];
        }
        i += 1;
    }

    ""
}

/// the builtin trim_end functions are not SIMD optimized, so we implement our own
/// to trim the end using SIMD optimization
/// unlike the builtin one, only ascii spaces and tabs are trimmed, other unicode whitespace are preserved
#[inline]
pub fn trim_end_simd(s: &str) -> &str {
    let bytes = s.as_bytes();

    if bytes.is_empty() {
        return s;
    }

    let mut end = bytes.len();
    while end > 0 {
        let b = bytes[end - 1];
        if b != b' ' && b != b'\t' {
            break;
        }
        end -= 1;
    }

    &s[..end]
}

/// the builtin trim functions are not SIMD optimized, so we implement our own
/// to trim both ends using the SIMD optimized functions above
/// unlike the builtin one, only ascii spaces and tabs are trimmed, other unicode whitespace are preserved
#[inline]
pub fn trim_both_simd(s: &str) -> &str {
    trim_end_simd(trim_start_simd(s))
}
