use super::{AppContext, BukuCommand};
use bukurs::crypto;
use serde::{Deserialize, Serialize};
use std::error::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockCommand {
    pub iterations: u32,
}

impl BukuCommand for LockCommand {
    fn execute(&self, ctx: &AppContext) -> Result<(), Box<dyn Error>> {
        let password = rpassword::prompt_password("Enter password: ")?;
        let confirm = rpassword::prompt_password("Confirm password: ")?;
        if password != confirm {
            return Err("Passwords do not match".into());
        }

        let enc_path = ctx.db_path.with_extension("db.enc");
        println!(
            "Encrypting {} to {} with {} iterations...",
            ctx.db_path.display(),
            enc_path.display(),
            self.iterations
        );
        crypto::BukuCrypt::encrypt_file(self.iterations, ctx.db_path, &enc_path, &password)?;
        eprintln!("Encryption complete.");
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnlockCommand {
    pub iterations: u32,
}

impl BukuCommand for UnlockCommand {
    fn execute(&self, ctx: &AppContext) -> Result<(), Box<dyn Error>> {
        let password = rpassword::prompt_password("Enter password: ")?;
        let enc_path = if ctx.db_path.extension().is_some_and(|ext| ext == "enc") {
            ctx.db_path.to_path_buf()
        } else {
            ctx.db_path.with_extension("db.enc")
        };

        let out_path = if enc_path.extension().is_some_and(|ext| ext == "enc") {
            enc_path.with_extension("")
        } else {
            enc_path.with_extension("db")
        };

        println!(
            "Decrypting {} to {} with {} iterations...",
            enc_path.display(),
            out_path.display(),
            self.iterations
        );
        crypto::BukuCrypt::decrypt_file(self.iterations, &out_path, &enc_path, &password)?;
        eprintln!("Decryption complete.");
        Ok(())
    }
}
