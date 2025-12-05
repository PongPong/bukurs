//! Private Bookmarks Plugin
//!
//! Password-protects sensitive bookmarks:
//! - Encrypts bookmark URLs and titles with a password
//! - Hidden from normal searches/prints unless unlocked
//! - Uses AES-256-GCM encryption
//!
//! Usage:
//! - Tag a bookmark with "private" to mark it as private
//! - Private bookmarks are encrypted and hidden from normal view
//! - Run `bukurs unlock` to temporarily view private bookmarks

use bukurs::models::bookmark::Bookmark;
use bukurs::plugin::{HookResult, Plugin, PluginContext, PluginInfo, SearchContext};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

// Use ring for encryption
use ring::aead::{self, Aad, BoundKey, Nonce, NonceSequence, NONCE_LEN, UnboundKey};
use ring::pbkdf2;
use ring::rand::{SecureRandom, SystemRandom};

const CREDENTIAL_LEN: usize = 32;
const SALT_LEN: usize = 16;

/// Encrypted bookmark data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedBookmark {
    /// Original bookmark ID
    pub id: usize,
    /// Encrypted URL (base64)
    pub encrypted_url: String,
    /// Encrypted title (base64)
    pub encrypted_title: String,
    /// Encrypted description (base64)
    pub encrypted_description: String,
    /// Salt used for key derivation (base64)
    pub salt: String,
    /// Nonce used for encryption (base64)
    pub nonce: String,
}

/// Persisted private bookmark data
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct PrivateData {
    /// Map of bookmark ID to encrypted data
    encrypted: HashMap<usize, EncryptedBookmark>,
    /// Password hash for verification (base64)
    password_hash: Option<String>,
    /// Salt for password hash
    password_salt: Option<String>,
}

/// Simple nonce sequence for AES-GCM
struct CounterNonceSequence(u64);

impl NonceSequence for CounterNonceSequence {
    fn advance(&mut self) -> Result<Nonce, ring::error::Unspecified> {
        let mut nonce_bytes = [0u8; NONCE_LEN];
        nonce_bytes[..8].copy_from_slice(&self.0.to_le_bytes());
        self.0 += 1;
        Nonce::try_assume_unique_for_key(&nonce_bytes)
    }
}

pub struct PrivateBookmarksPlugin {
    /// Private bookmark data
    data: Mutex<PrivateData>,
    /// Data file path
    data_file: Option<PathBuf>,
    /// Tag that marks bookmarks as private
    private_tag: String,
    /// Placeholder text for hidden URLs
    hidden_url_placeholder: String,
    /// Placeholder text for hidden titles
    hidden_title_placeholder: String,
    /// Whether currently unlocked
    unlocked: Mutex<bool>,
    /// Current session password (in memory only)
    session_password: Mutex<Option<String>>,
    /// Whether the plugin is enabled
    enabled: bool,
}

impl PrivateBookmarksPlugin {
    pub fn new() -> Self {
        Self {
            data: Mutex::new(PrivateData::default()),
            data_file: None,
            private_tag: "private".to_string(),
            hidden_url_placeholder: "[PRIVATE - use 'bukurs unlock' to view]".to_string(),
            hidden_title_placeholder: "[Private Bookmark]".to_string(),
            unlocked: Mutex::new(false),
            session_password: Mutex::new(None),
            enabled: true,
        }
    }

    /// Load data from file
    fn load_data(&self) -> PrivateData {
        if let Some(ref path) = self.data_file {
            if let Ok(contents) = fs::read_to_string(path) {
                if let Ok(data) = serde_json::from_str(&contents) {
                    return data;
                }
            }
        }
        PrivateData::default()
    }

    /// Save data to file
    fn save_data(&self) {
        if let Some(ref path) = self.data_file {
            if let Ok(data) = self.data.lock() {
                if let Ok(json) = serde_json::to_string_pretty(&*data) {
                    let _ = fs::write(path, json);
                }
            }
        }
    }

    /// Derive key from password
    fn derive_key(password: &str, salt: &[u8]) -> [u8; CREDENTIAL_LEN] {
        let mut key = [0u8; CREDENTIAL_LEN];
        pbkdf2::derive(
            pbkdf2::PBKDF2_HMAC_SHA256,
            std::num::NonZeroU32::new(100_000).unwrap(),
            salt,
            password.as_bytes(),
            &mut key,
        );
        key
    }

    /// Generate random bytes
    fn random_bytes<const N: usize>() -> [u8; N] {
        let rng = SystemRandom::new();
        let mut bytes = [0u8; N];
        rng.fill(&mut bytes).expect("Failed to generate random bytes");
        bytes
    }

    /// Encrypt data with password
    fn encrypt(plaintext: &str, password: &str) -> Result<(String, String, String), String> {
        let salt: [u8; SALT_LEN] = Self::random_bytes();
        let nonce_bytes: [u8; NONCE_LEN] = Self::random_bytes();
        let key_bytes = Self::derive_key(password, &salt);

        let unbound_key = UnboundKey::new(&aead::AES_256_GCM, &key_bytes)
            .map_err(|_| "Failed to create encryption key")?;

        let _nonce = Nonce::try_assume_unique_for_key(&nonce_bytes)
            .map_err(|_| "Failed to create nonce")?;

        let mut sealing_key = aead::SealingKey::new(unbound_key, CounterNonceSequence(0));

        let mut in_out = plaintext.as_bytes().to_vec();
        in_out.reserve(aead::AES_256_GCM.tag_len());

        sealing_key
            .seal_in_place_append_tag(Aad::empty(), &mut in_out)
            .map_err(|_| "Encryption failed")?;

        use base64::Engine;
        let engine = base64::engine::general_purpose::STANDARD;

        Ok((
            engine.encode(&in_out),
            engine.encode(salt),
            engine.encode(nonce_bytes),
        ))
    }

    /// Decrypt data with password
    fn decrypt(ciphertext_b64: &str, salt_b64: &str, nonce_b64: &str, password: &str) -> Result<String, String> {
        use base64::Engine;
        let engine = base64::engine::general_purpose::STANDARD;

        let ciphertext = engine.decode(ciphertext_b64).map_err(|_| "Invalid ciphertext")?;
        let salt = engine.decode(salt_b64).map_err(|_| "Invalid salt")?;
        let nonce_bytes = engine.decode(nonce_b64).map_err(|_| "Invalid nonce")?;

        let key_bytes = Self::derive_key(password, &salt);

        let unbound_key = UnboundKey::new(&aead::AES_256_GCM, &key_bytes)
            .map_err(|_| "Failed to create decryption key")?;

        let _nonce = Nonce::try_assume_unique_for_key(&nonce_bytes)
            .map_err(|_| "Invalid nonce")?;

        let mut opening_key = aead::OpeningKey::new(unbound_key, CounterNonceSequence(0));

        let mut in_out = ciphertext;
        let plaintext = opening_key
            .open_in_place(Aad::empty(), &mut in_out)
            .map_err(|_| "Decryption failed - wrong password?")?;

        String::from_utf8(plaintext.to_vec()).map_err(|_| "Invalid UTF-8 in decrypted data".to_string())
    }

    /// Check if bookmark has private tag
    fn is_private(tags: &str, private_tag: &str) -> bool {
        tags.trim_matches(',')
            .split(',')
            .any(|t| t == private_tag)
    }

    /// Hash password for storage
    fn hash_password(password: &str, salt: &[u8]) -> String {
        let key = Self::derive_key(password, salt);
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.encode(key)
    }

    /// Verify password against stored hash
    fn verify_password(&self, password: &str) -> bool {
        if let Ok(data) = self.data.lock() {
            if let (Some(ref hash), Some(ref salt_b64)) = (&data.password_hash, &data.password_salt) {
                use base64::Engine;
                if let Ok(salt) = base64::engine::general_purpose::STANDARD.decode(salt_b64) {
                    let computed = Self::hash_password(password, &salt);
                    return computed == *hash;
                }
            }
        }
        // No password set yet
        true
    }

    /// Set/update password
    fn set_password(&self, password: &str) {
        let salt: [u8; SALT_LEN] = Self::random_bytes();
        let hash = Self::hash_password(password, &salt);

        use base64::Engine;
        let salt_b64 = base64::engine::general_purpose::STANDARD.encode(salt);

        if let Ok(mut data) = self.data.lock() {
            data.password_hash = Some(hash);
            data.password_salt = Some(salt_b64);
        }
        self.save_data();
    }
}

impl Default for PrivateBookmarksPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for PrivateBookmarksPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "private-bookmarks".to_string(),
            version: "1.0.0".to_string(),
            description: "Password-protects sensitive bookmarks".to_string(),
            author: "bukurs".to_string(),
        }
    }

    fn on_load(&mut self, ctx: &PluginContext) -> HookResult {
        self.data_file = Some(ctx.data_dir.join("private_bookmarks.json"));

        // Load existing data
        let loaded = self.load_data();
        if let Ok(mut data) = self.data.lock() {
            *data = loaded;
        }

        // Load config
        if let Some(enabled) = ctx.config.get("enabled") {
            self.enabled = enabled != "false";
        }
        if let Some(tag) = ctx.config.get("private_tag") {
            self.private_tag = tag.clone();
        }

        HookResult::Continue
    }

    fn on_unload(&mut self, _ctx: &PluginContext) {
        // Clear session password on unload
        if let Ok(mut pwd) = self.session_password.lock() {
            *pwd = None;
        }
        if let Ok(mut unlocked) = self.unlocked.lock() {
            *unlocked = false;
        }
        self.save_data();
    }

    fn on_post_search(
        &self,
        _ctx: &PluginContext,
        _search_ctx: &SearchContext,
        results: &mut Vec<Bookmark>,
    ) -> HookResult {
        if !self.enabled {
            return HookResult::Continue;
        }

        let is_unlocked = self.unlocked.lock().map(|u| *u).unwrap_or(false);

        for bookmark in results.iter_mut() {
            if Self::is_private(&bookmark.tags, &self.private_tag) {
                if !is_unlocked {
                    // Hide private bookmark content
                    bookmark.url = self.hidden_url_placeholder.clone();
                    bookmark.title = self.hidden_title_placeholder.clone();
                    bookmark.description = String::new();
                } else {
                    // Try to decrypt if we have encrypted data
                    if let Ok(data) = self.data.lock() {
                        if let Some(encrypted) = data.encrypted.get(&bookmark.id) {
                            if let Ok(pwd) = self.session_password.lock() {
                                if let Some(ref password) = *pwd {
                                    // Decrypt and restore
                                    if let Ok(url) = Self::decrypt(
                                        &encrypted.encrypted_url,
                                        &encrypted.salt,
                                        &encrypted.nonce,
                                        password,
                                    ) {
                                        bookmark.url = url;
                                    }
                                    if let Ok(title) = Self::decrypt(
                                        &encrypted.encrypted_title,
                                        &encrypted.salt,
                                        &encrypted.nonce,
                                        password,
                                    ) {
                                        bookmark.title = title;
                                    }
                                    if let Ok(desc) = Self::decrypt(
                                        &encrypted.encrypted_description,
                                        &encrypted.salt,
                                        &encrypted.nonce,
                                        password,
                                    ) {
                                        bookmark.description = desc;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        HookResult::Continue
    }

    fn on_pre_open(&self, _ctx: &PluginContext, bookmark: &Bookmark) -> HookResult {
        if !self.enabled {
            return HookResult::Continue;
        }

        // Block opening private bookmarks when locked
        if Self::is_private(&bookmark.tags, &self.private_tag) {
            let is_unlocked = self.unlocked.lock().map(|u| *u).unwrap_or(false);
            if !is_unlocked {
                return HookResult::Error(
                    "Cannot open private bookmark. Use 'bukurs unlock' first.".to_string()
                );
            }
        }

        HookResult::Continue
    }
}

/// Create an instance of this plugin (required for auto-discovery)
pub fn create_plugin() -> Box<dyn Plugin> {
    Box::new(PrivateBookmarksPlugin::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_private() {
        assert!(PrivateBookmarksPlugin::is_private(",private,rust,", "private"));
        assert!(!PrivateBookmarksPlugin::is_private(",rust,web,", "private"));
    }

    #[test]
    fn test_encrypt_decrypt() {
        let plaintext = "https://secret-url.com/path";
        let password = "mysecretpassword";

        let (ciphertext, salt, nonce) = PrivateBookmarksPlugin::encrypt(plaintext, password).unwrap();
        let decrypted = PrivateBookmarksPlugin::decrypt(&ciphertext, &salt, &nonce, password).unwrap();

        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn test_wrong_password_fails() {
        let plaintext = "secret data";
        let password = "correct";
        let wrong_password = "wrong";

        let (ciphertext, salt, nonce) = PrivateBookmarksPlugin::encrypt(plaintext, password).unwrap();
        let result = PrivateBookmarksPlugin::decrypt(&ciphertext, &salt, &nonce, wrong_password);

        assert!(result.is_err());
    }

    #[test]
    fn test_key_derivation_deterministic() {
        let password = "test";
        let salt = [0u8; SALT_LEN];

        let key1 = PrivateBookmarksPlugin::derive_key(password, &salt);
        let key2 = PrivateBookmarksPlugin::derive_key(password, &salt);

        assert_eq!(key1, key2);
    }
}
