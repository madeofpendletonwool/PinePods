use argon2::{Argon2, PasswordHash, PasswordVerifier};
use crate::error::{AppError, AppResult};

/// Verify password using Argon2 - matches Python's passlib CryptContext with argon2
pub fn verify_password(password: &str, stored_hash: &str) -> AppResult<bool> {
    let argon2 = Argon2::default();
    
    let parsed_hash = PasswordHash::new(stored_hash)
        .map_err(|e| AppError::Auth(format!("Invalid password hash format: {}", e)))?;
    
    match argon2.verify_password(password.as_bytes(), &parsed_hash) {
        Ok(()) => Ok(true),
        Err(_) => Ok(false),
    }
}

/// Hash password using Argon2 - matches Python's passlib CryptContext
pub fn hash_password(password: &str) -> AppResult<String> {
    use argon2::{PasswordHasher, password_hash::SaltString};
    use rand::Rng;
    
    let argon2 = Argon2::default();
    let mut salt_bytes = [0u8; 32];
    rand::rng().fill(&mut salt_bytes);
    let salt = SaltString::encode_b64(&salt_bytes)
        .map_err(|e| AppError::Auth(format!("Failed to create salt: {}", e)))?;
    
    let password_hash = argon2.hash_password(password.as_bytes(), &salt)
        .map_err(|e| AppError::Auth(format!("Failed to hash password: {}", e)))?;
    
    Ok(password_hash.to_string())
}