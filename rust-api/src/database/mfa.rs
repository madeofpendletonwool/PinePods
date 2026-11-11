//! TOTP multi-factor secrets.

use super::*;

// Global temporary MFA secrets storage (matches Python temp_mfa_secrets)
lazy_static! {
    static ref TEMP_MFA_SECRETS: Arc<Mutex<HashMap<String, String>>> = Arc::new(Mutex::new(HashMap::new()));
}

// Build the TOTP verifier/provisioner used for all PinePods MFA: SHA1, 6 digits,
// 30s step, +/-1 step skew, issuer "Pinepods". `secret_b32` is the stored unpadded
// RFC4648 base32 secret.
pub fn build_totp(secret_b32: &str, account_name: &str) -> AppResult<totp_rs::Totp> {
    let secret = totp_rs::Secret::try_from_base32(secret_b32)
        .map_err(|e| AppError::internal(&format!("Invalid MFA secret format: {}", e)))?;
    totp_rs::Builder::new()
        .with_algorithm(totp_rs::Algorithm::SHA1)
        .with_digits(6)
        .with_skew(1)
        .with_step_duration(30)
        .with_secret(secret)
        .with_issuer(Some("Pinepods"))
        .with_account_name(account_name)
        .build()
        .map_err(|e| AppError::internal(&format!("TOTP creation failed: {}", e)))
}

impl DatabasePool {
    // Generate MFA secret with QR code - matches Python generate_mfa_secret function exactly
    pub async fn generate_mfa_secret(&self, user_id: i32) -> AppResult<(String, String)> {
        use qrcode::QrCode;

        // Generate random base32 secret (matches Python random_base32())
        let secret = {
            let secret_bytes: [u8; 20] = rand::random(); // 160 bits = 32 base32 chars
            totp_rs::Secret::from(secret_bytes).to_base32()
        };
        
        // Get user email for provisioning URI
        let email = dialect_match!(self, {
            pg(pool) => {
                let row = q::query(r#"SELECT email FROM "Users" WHERE userid = $1"#)
                    .bind(user_id)
                    .fetch_one(pool)
                    .await?;
                row.try_get::<String, _>("email")?
            },
            mysql(pool) => {
                let row = sqlx::query("SELECT Email FROM Users WHERE UserID = ?")
                    .bind(user_id)
                    .fetch_one(pool)
                    .await?;
                row.try_get::<String, _>("Email")?
            },
        });
        
        // Create TOTP instance and provisioning URI
        let totp = build_totp(&secret, &email)?;
        
        let provisioning_uri = totp.to_url()
            .map_err(|e| AppError::internal(&format!("TOTP URL generation failed: {}", e)))?;
        
        // Generate QR code as SVG (matches Python qrcode generation)
        let qr_code = QrCode::new(&provisioning_uri)
            .map_err(|e| AppError::internal(&format!("QR code generation failed: {}", e)))?;
        
        let qr_code_svg = qr_code
            .render::<qrcode::render::svg::Color>()
            .min_dimensions(200, 200)
            .dark_color(qrcode::render::svg::Color("#000000"))
            .light_color(qrcode::render::svg::Color("#ffffff"))
            .build();
        
        // Store temporarily in Redis (matches Python temp_mfa_secrets)
        let temp_key = format!("temp_mfa_{}", user_id);
        self.store_temp_mfa_secret(&temp_key, &secret).await?;
        
        Ok((secret, qr_code_svg))
    }

    // Store temporary MFA secret in memory (matches Python temp_mfa_secrets)
    pub(super) async fn store_temp_mfa_secret(&self, key: &str, secret: &str) -> AppResult<()> {
        let mut secrets = TEMP_MFA_SECRETS.lock().map_err(|e| AppError::internal(&format!("Failed to lock temp MFA secrets: {}", e)))?;
        secrets.insert(key.to_string(), secret.to_string());
        Ok(())
    }

    // Verify temporary MFA code - matches Python verify_temp_mfa function exactly  
    pub async fn verify_temp_mfa(&self, user_id: i32, mfa_code: &str) -> AppResult<bool> {
        // Get temporary secret (in production this would be from Redis)
        let temp_key = format!("temp_mfa_{}", user_id);
        let secret = match self.get_temp_mfa_secret(&temp_key).await? {
            Some(secret) => secret,
            None => return Ok(false),
        };
        
        // Create TOTP instance and verify code
        let totp = build_totp(&secret, "verification")?;
        
        let verified = totp.check_current(mfa_code).is_some();
        
        if verified {
            // Save to permanent storage and remove from temp
            self.save_mfa_secret(user_id, &secret).await?;
            self.remove_temp_mfa_secret(&temp_key).await?;
        }
        
        Ok(verified)
    }

    // Get temporary MFA secret from memory (matches Python temp_mfa_secrets lookup)
    pub(super) async fn get_temp_mfa_secret(&self, key: &str) -> AppResult<Option<String>> {
        let secrets = TEMP_MFA_SECRETS.lock().map_err(|e| AppError::internal(&format!("Failed to lock temp MFA secrets: {}", e)))?;
        Ok(secrets.get(key).cloned())
    }

    // Remove temporary MFA secret from memory (matches Python temp_mfa_secrets cleanup)
    pub(super) async fn remove_temp_mfa_secret(&self, key: &str) -> AppResult<()> {
        let mut secrets = TEMP_MFA_SECRETS.lock().map_err(|e| AppError::internal(&format!("Failed to lock temp MFA secrets: {}", e)))?;
        secrets.remove(key);
        Ok(())
    }

    // Check if MFA is enabled - matches Python check_mfa_enabled function exactly
    pub async fn check_mfa_enabled(&self, user_id: i32) -> AppResult<bool> {
        dialect_match!(self, {
            pg(pool) => {
                let row = q::query(r#"SELECT mfa_secret FROM "Users" WHERE userid = $1"#)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = row {
                    let secret: Option<String> = row.try_get("mfa_secret")?;
                    Ok(secret.is_some())
                } else {
                    Ok(false)
                }
            },
            mysql(pool) => {
                let row = sqlx::query("SELECT MFA_Secret FROM Users WHERE UserID = ?")
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = row {
                    let secret: Option<String> = row.try_get("MFA_Secret")?;
                    Ok(secret.is_some())
                } else {
                    Ok(false)
                }
            },
        })
    }

    // Save MFA secret - matches Python save_mfa_secret function exactly
    pub async fn save_mfa_secret(&self, user_id: i32, mfa_secret: &str) -> AppResult<bool> {
        dialect_match!(self, {
            pg(pool) => {
                let result = q::query(r#"UPDATE "Users" SET mfa_secret = $1 WHERE userid = $2"#)
                    .bind(mfa_secret)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
                
                Ok(result.rows_affected() > 0)
            },
            mysql(pool) => {
                let result = sqlx::query("UPDATE Users SET MFA_Secret = ? WHERE UserID = ?")
                    .bind(mfa_secret)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
                
                Ok(result.rows_affected() > 0)
            },
        })
    }

    // Delete MFA secret - matches Python delete_mfa_secret function exactly
    pub async fn delete_mfa_secret(&self, user_id: i32) -> AppResult<bool> {
        dialect_match!(self, {
            pg(pool) => {
                let result = q::query(r#"UPDATE "Users" SET mfa_secret = NULL WHERE userid = $1"#)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
                
                Ok(result.rows_affected() > 0)
            },
            mysql(pool) => {
                let result = sqlx::query("UPDATE Users SET MFA_Secret = NULL WHERE UserID = ?")
                    .bind(user_id)
                    .execute(pool)
                    .await?;
                
                Ok(result.rows_affected() > 0)
            },
        })
    }

    // Get MFA secret - matches Python get_mfa_secret function
    pub async fn get_mfa_secret(&self, user_id: i32) -> AppResult<Option<String>> {
        dialect_match!(self, {
            pg(pool) => {
                let row = q::query(r#"SELECT mfa_secret FROM "Users" WHERE userid = $1"#)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = row {
                    Ok(row.try_get("mfa_secret").ok())
                } else {
                    Ok(None)
                }
            },
            mysql(pool) => {
                let row = sqlx::query("SELECT MFA_Secret FROM Users WHERE UserID = ?")
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = row {
                    Ok(row.try_get("MFA_Secret").ok())
                } else {
                    Ok(None)
                }
            },
        })
    }
}

#[cfg(test)]
mod totp_tests {
    use super::build_totp;

    // A secret in the exact format generate_mfa_secret stores (20 random bytes, unpadded base32).
    const SECRET: &str = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";

    #[test]
    fn current_code_verifies_and_wrong_code_does_not() {
        let totp = build_totp(SECRET, "login").unwrap();
        let code = totp.generate_current().to_string();
        assert_eq!(code.len(), 6);
        assert!(totp.check_current(&code).is_some());
        let wrong = format!("{:06}", (code.parse::<u32>().unwrap() + 1) % 1_000_000);
        assert!(totp.check_current(&wrong).is_none());
    }

    // RFC 6238 SHA1 test vector (T = 59s), proving the algorithm/step settings match what
    // authenticator apps enrolled under the previous totp-rs version expect.
    #[test]
    fn matches_rfc6238_sha1_vector() {
        let totp = build_totp(SECRET, "login").unwrap();
        assert_eq!(totp.generate(59).to_string(), "287082");
    }

    #[test]
    fn invalid_secret_is_an_error_not_a_panic() {
        assert!(build_totp("not base32!", "login").is_err());
    }

    #[test]
    fn provisioning_url_has_issuer_and_secret() {
        let url = build_totp(SECRET, "user@example.com").unwrap().to_url().unwrap();
        assert!(url.starts_with("otpauth://totp/"));
        assert!(url.contains("issuer=Pinepods"));
        assert!(url.contains(&format!("secret={SECRET}")));
    }
}
