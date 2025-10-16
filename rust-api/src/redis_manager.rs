use serde_json::Value;
use crate::{error::AppResult, redis_client::RedisClient};

pub struct ImportProgressManager {
    redis_client: RedisClient,
}

impl ImportProgressManager {
    pub fn new(redis_client: RedisClient) -> Self {
        Self { redis_client }
    }

    // Start import progress tracking - matches Python ImportProgressManager.start_import
    pub async fn start_import(&self, user_id: i32, total_podcasts: i32) -> AppResult<()> {
        let progress_data = serde_json::json!({
            "current": 0,
            "total": total_podcasts,
            "current_podcast": ""
        });
        
        let key = format!("import_progress:{}", user_id);
        self.redis_client.set_ex(&key, &progress_data.to_string(), 3600).await?;
        
        Ok(())
    }

    // Update import progress - matches Python ImportProgressManager.update_progress
    pub async fn update_progress(&self, user_id: i32, current: i32, current_podcast: &str) -> AppResult<()> {
        let key = format!("import_progress:{}", user_id);
        
        // Get current progress
        if let Some(progress_json) = self.redis_client.get::<String>(&key).await? {
            if let Ok(mut progress) = serde_json::from_str::<Value>(&progress_json) {
                progress["current"] = serde_json::Value::Number(serde_json::Number::from(current));
                progress["current_podcast"] = serde_json::Value::String(current_podcast.to_string());
                
                self.redis_client.set_ex(&key, &progress.to_string(), 3600).await?;
            }
        }
        
        Ok(())
    }

    // Get import progress - matches Python ImportProgressManager.get_progress
    pub async fn get_progress(&self, user_id: i32) -> AppResult<(i32, i32, String)> {
        let key = format!("import_progress:{}", user_id);
        
        if let Some(progress_json) = self.redis_client.get::<String>(&key).await? {
            if let Ok(progress) = serde_json::from_str::<Value>(&progress_json) {
                let current = progress.get("current").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                let total = progress.get("total").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                let current_podcast = progress.get("current_podcast").and_then(|v| v.as_str()).unwrap_or("").to_string();
                
                return Ok((current, total, current_podcast));
            }
        }
        
        Ok((0, 0, "".to_string()))
    }

    // Clear import progress - matches Python ImportProgressManager.clear_progress
    pub async fn clear_progress(&self, user_id: i32) -> AppResult<()> {
        let key = format!("import_progress:{}", user_id);
        self.redis_client.delete(&key).await?;
        Ok(())
    }
}

// Notification manager for sending test notifications
pub struct NotificationManager {
    redis_client: RedisClient,
}

impl NotificationManager {
    pub fn new(redis_client: RedisClient) -> Self {
        Self { redis_client }
    }

    // Send test notification - matches Python notification functionality
    pub async fn send_test_notification(&self, user_id: i32, platform: &str, settings: &serde_json::Value) -> AppResult<bool> {
        println!("Sending test notification for user {} on platform {}", user_id, platform);
        
        match platform {
            "ntfy" => self.send_ntfy_notification(settings).await,
            "gotify" => self.send_gotify_notification(settings).await,
            "http" => self.send_http_notification(settings).await,
            _ => {
                println!("Unsupported notification platform: {}", platform);
                Ok(false)
            }
        }
    }

    async fn send_ntfy_notification(&self, settings: &serde_json::Value) -> AppResult<bool> {
        let topic = settings.get("ntfy_topic").and_then(|v| v.as_str()).unwrap_or("");
        let server_url = settings.get("ntfy_server_url").and_then(|v| v.as_str()).unwrap_or("https://ntfy.sh");
        let username = settings.get("ntfy_username").and_then(|v| v.as_str());
        let password = settings.get("ntfy_password").and_then(|v| v.as_str());
        let access_token = settings.get("ntfy_access_token").and_then(|v| v.as_str());
        
        if topic.is_empty() {
            return Ok(false);
        }

        let client = reqwest::Client::new();
        let url = format!("{}/{}", server_url, topic);
        
        let mut request = client
            .post(&url)
            .header("Content-Type", "text/plain")
            .body("Test notification from PinePods");
        
        // Add authentication if provided
        if let Some(token) = access_token.filter(|t| !t.is_empty()) {
            // Use access token (preferred method)
            request = request.header("Authorization", format!("Bearer {}", token));
        } else if let (Some(user), Some(pass)) = (username.filter(|u| !u.is_empty()), password.filter(|p| !p.is_empty())) {
            // Use username/password basic auth
            request = request.basic_auth(user, Some(pass));
        }
        
        let response = request.send().await?;

        let status = response.status();
        let is_success = status.is_success();

        if !is_success {
            let response_text = response.text().await.unwrap_or_default();
            println!("Ntfy notification failed with status: {} - Response: {}", 
                     status, response_text);
        }

        Ok(is_success)
    }

    async fn send_gotify_notification(&self, settings: &serde_json::Value) -> AppResult<bool> {
        let gotify_url = settings.get("gotify_url").and_then(|v| v.as_str()).unwrap_or("");
        let gotify_token = settings.get("gotify_token").and_then(|v| v.as_str()).unwrap_or("");
        
        if gotify_url.is_empty() || gotify_token.is_empty() {
            return Ok(false);
        }

        let client = reqwest::Client::new();
        let url = format!("{}/message?token={}", gotify_url, gotify_token);
        
        let payload = serde_json::json!({
            "title": "PinePods Test",
            "message": "Test notification from PinePods",
            "priority": 5
        });

        let response = client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await?;

        Ok(response.status().is_success())
    }

    async fn send_http_notification(&self, settings: &serde_json::Value) -> AppResult<bool> {
        let http_url = settings.get("http_url").and_then(|v| v.as_str()).unwrap_or("");
        let http_token = settings.get("http_token").and_then(|v| v.as_str()).unwrap_or("");
        let http_method = settings.get("http_method").and_then(|v| v.as_str()).unwrap_or("POST");
        
        if http_url.is_empty() {
            println!("HTTP URL is empty, cannot send notification");
            return Ok(false);
        }

        let client = reqwest::Client::new();
        
        // Build the request based on method
        let request_builder = match http_method.to_uppercase().as_str() {
            "GET" => {
                // For GET requests, add message as query parameter
                let url_with_params = if http_url.contains('?') {
                    format!("{}&message={}", http_url, urlencoding::encode("Test notification from PinePods"))
                } else {
                    format!("{}?message={}", http_url, urlencoding::encode("Test notification from PinePods"))
                };
                client.get(&url_with_params)
            },
            "POST" | _ => {
                // For POST requests, send JSON payload
                let payload = if http_url.contains("api.telegram.org") {
                    // Special handling for Telegram Bot API
                    let chat_id = if let Some(chat_id_str) = http_token.split(':').nth(1) {
                        // Extract chat_id from token if it contains chat_id (format: bot_token:chat_id)
                        chat_id_str
                    } else {
                        // Default chat_id - user needs to configure this properly
                        "YOUR_CHAT_ID"
                    };
                    
                    serde_json::json!({
                        "chat_id": chat_id,
                        "text": "Test notification from PinePods"
                    })
                } else {
                    // Generic JSON payload
                    serde_json::json!({
                        "title": "PinePods Test",
                        "message": "Test notification from PinePods",
                        "text": "Test notification from PinePods"
                    })
                };
                
                client.post(http_url)
                    .header("Content-Type", "application/json")
                    .json(&payload)
            }
        };

        // Add authorization header if token is provided
        let request_builder = if !http_token.is_empty() {
            if http_url.contains("api.telegram.org") {
                // For Telegram, token goes in URL path, not header
                request_builder
            } else {
                // For other services, add as Bearer token
                request_builder.header("Authorization", format!("Bearer {}", http_token))
            }
        } else {
            request_builder
        };

        match request_builder.send().await {
            Ok(response) => {
                let status = response.status();
                let is_success = status.is_success();
                
                if !is_success {
                    let response_text = response.text().await.unwrap_or_default();
                    println!("HTTP notification failed with status: {} - Response: {}", 
                             status, response_text);
                }
                
                Ok(is_success)
            },
            Err(e) => {
                println!("HTTP notification request failed: {}", e);
                Ok(false)
            }
        }
    }
}