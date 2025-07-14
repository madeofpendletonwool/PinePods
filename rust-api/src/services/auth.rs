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
    use rand::rngs::OsRng;
    
    let argon2 = Argon2::default();
    let salt = SaltString::generate(&mut OsRng);
    
    let password_hash = argon2.hash_password(password.as_bytes(), &salt)
        .map_err(|e| AppError::Auth(format!("Failed to hash password: {}", e)))?;
    
    Ok(password_hash.to_string())
}