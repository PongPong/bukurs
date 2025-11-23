use aes::Aes256;
use cbc::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use rand::{thread_rng, RngCore};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;

type Aes256CbcEnc = cbc::Encryptor<Aes256>;
type Aes256CbcDec = cbc::Decryptor<Aes256>;

pub struct BukuCrypt;

impl BukuCrypt {
    const BLOCKSIZE: usize = 0x10000; // 64 KB
    const SALT_SIZE: usize = 0x20;
    const CHUNKSIZE: usize = 0x80000; // 512 KB

    pub fn encrypt_file(
        iterations: u32,
        dbfile: &Path,
        encfile: &Path,
        password: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let dbhash = Self::get_filehash(dbfile)?;
        let filesize = fs::metadata(dbfile)?.len();

        let mut salt = [0u8; Self::SALT_SIZE];
        thread_rng().fill_bytes(&mut salt);

        let mut iv = [0u8; 16];
        thread_rng().fill_bytes(&mut iv);

        let key = Self::derive_key(password, &salt, iterations);
        let encryptor = Aes256CbcEnc::new(&key.into(), &iv.into());

        let mut infp = File::open(dbfile)?;
        let mut outfp = File::create(encfile)?;

        outfp.write_all(&filesize.to_le_bytes())?;
        outfp.write_all(&salt)?;
        outfp.write_all(&iv)?;
        outfp.write_all(&dbhash)?;

        let mut buffer = vec![0u8; Self::CHUNKSIZE];
        let mut encryptor = encryptor;

        loop {
            let read_bytes = infp.read(&mut buffer)?;
            if read_bytes == 0 {
                break;
            }

            let chunk = &buffer[..read_bytes];
            // Padding
            let padding_len = 16 - (read_bytes % 16);
            let padding_len = if padding_len == 0 { 0 } else { padding_len };

            let mut padded_chunk = chunk.to_vec();
            if padding_len > 0 {
                padded_chunk.extend(std::iter::repeat_n(b' ', padding_len));
            }

            for block_chunk in padded_chunk.chunks_mut(16) {
                let block = cbc::cipher::generic_array::GenericArray::from_mut_slice(block_chunk);
                encryptor.encrypt_block_mut(block);
            }
            outfp.write_all(&padded_chunk)?;
        }

        Ok(())
    }

    pub fn decrypt_file(
        iterations: u32,
        dbfile: &Path,
        encfile: &Path,
        password: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut infp = File::open(encfile)?;

        let mut size_bytes = [0u8; 8];
        infp.read_exact(&mut size_bytes)?;
        let size = u64::from_le_bytes(size_bytes);

        let mut salt = [0u8; 32];
        infp.read_exact(&mut salt)?;

        let mut iv = [0u8; 16];
        infp.read_exact(&mut iv)?;

        let key = Self::derive_key(password, &salt, iterations);
        let decryptor = Aes256CbcDec::new(&key.into(), &iv.into());

        let mut enchash = [0u8; 32];
        infp.read_exact(&mut enchash)?;

        let mut outfp = File::create(dbfile)?;
        let mut buffer = vec![0u8; Self::CHUNKSIZE];
        let mut decryptor = decryptor;

        loop {
            let read_bytes = infp.read(&mut buffer)?;
            if read_bytes == 0 {
                break;
            }

            let chunk = &mut buffer[..read_bytes];
            // Decrypt in place
            // Since we read chunks, they should be multiples of 16 (except maybe if file is corrupted or end?)
            // The encrypted file is padded to 16 bytes.

            for block_chunk in chunk.chunks_mut(16) {
                let block = cbc::cipher::generic_array::GenericArray::from_mut_slice(block_chunk);
                decryptor.decrypt_block_mut(block);
            }

            outfp.write_all(chunk)?;
        }

        outfp.set_len(size)?;

        let dbhash = Self::get_filehash(dbfile)?;
        if dbhash != enchash {
            fs::remove_file(dbfile)?;
            return Err("Decryption failed: Hash mismatch".into());
        }

        Ok(())
    }

    fn get_filehash(filepath: &Path) -> Result<[u8; 32], std::io::Error> {
        let mut file = File::open(filepath)?;
        let mut hasher = Sha256::new();
        let mut buffer = [0; Self::BLOCKSIZE];

        loop {
            let count = file.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            hasher.update(&buffer[..count]);
        }

        Ok(hasher.finalize().into())
    }

    fn derive_key(password: &str, salt: &[u8], iterations: u32) -> [u8; 32] {
        // Python: key = ('%s%s' % (self.password, salt.decode('utf-8', 'replace'))).encode('utf-8')
        let salt_str = String::from_utf8_lossy(salt);
        let key_material = format!("{}{}", password, salt_str).into_bytes();

        let mut key = [0u8; 32];
        // Initial hash
        // Python loop:
        // for _ in range(self.iterations):
        //     key = self._sha256(key).digest()
        // Wait, the python code starts with the concatenated string as `key`.
        // Then hashes it `iterations` times.

        let mut current_hash = key_material;

        for _ in 0..iterations {
            let mut hasher = Sha256::new();
            hasher.update(&current_hash);
            current_hash = hasher.finalize().to_vec();
        }

        key.copy_from_slice(&current_hash);
        key
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_encrypt_decrypt() {
        let dbfile = Path::new("test_crypto.db");
        let encfile = Path::new("test_crypto.db.enc");
        let password = "password123";

        // Create dummy DB file
        let mut file = File::create(dbfile).unwrap();
        file.write_all(b"dummy data for encryption test").unwrap();

        // Encrypt
        BukuCrypt::encrypt_file(8, dbfile, encfile, password).unwrap();

        // Remove original DB
        fs::remove_file(dbfile).unwrap();

        // Decrypt
        BukuCrypt::decrypt_file(8, dbfile, encfile, password).unwrap();

        // Verify content
        let mut file = File::open(dbfile).unwrap();
        let mut content = Vec::new();
        file.read_to_end(&mut content).unwrap();

        assert_eq!(content, b"dummy data for encryption test");

        // Cleanup
        fs::remove_file(dbfile).unwrap();
        fs::remove_file(encfile).unwrap();
    }
}
