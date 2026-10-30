// OIDC helpers shared by the login callback, the admin settings API and env-based setup.

use serde::{Deserialize, Serialize};
use std::time::Duration;

const WELL_KNOWN_PATH: &str = "/.well-known/openid-configuration";
const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(10);
pub const CALLBACK_PATH: &str = "/api/auth/callback";

/// The parts of an OpenID Provider configuration document that PinePods uses.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct OidcDiscovery {
    pub issuer: Option<String>,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub userinfo_endpoint: String,
    #[serde(default)]
    pub scopes_supported: Vec<String>,
    #[serde(default)]
    pub claims_supported: Vec<String>,
}

/// Accepts either an issuer URL or the full `.well-known/openid-configuration` URL.
pub fn discovery_document_url(input: &str) -> Result<String, String> {
    let trimmed = input.trim().trim_end_matches('/');
    let parsed = url::Url::parse(trimmed).map_err(|e| format!("Invalid discovery URL: {}", e))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("Discovery URL must use http or https".to_string());
    }
    if parsed.path().ends_with(WELL_KNOWN_PATH) {
        Ok(trimmed.to_string())
    } else {
        Ok(format!("{}{}", trimmed, WELL_KNOWN_PATH))
    }
}

/// Fetches and parses a provider's discovery document. Providers are often on the LAN, so this
/// uses the unguarded service client; only admins and env config can trigger it.
pub async fn discover(input: &str) -> Result<OidcDiscovery, String> {
    let url = discovery_document_url(input)?;
    let response = crate::services::http::service_client()
        .get(&url)
        .header("Accept", "application/json")
        .timeout(DISCOVERY_TIMEOUT)
        .send()
        .await
        .map_err(|e| format!("Could not reach {}: {}", url, e))?;

    let status = response.status();
    if !status.is_success() {
        return Err(format!("{} returned {}", url, status));
    }

    response
        .json::<OidcDiscovery>()
        .await
        .map_err(|e| format!("{} is not a valid OpenID configuration: {}", url, e))
}

/// Checks a client-supplied redirect URI before it is sent to the provider's token endpoint.
/// It must be an http(s) URL pointing at PinePods' callback.
pub fn validate_redirect_uri(uri: &str) -> Option<String> {
    let parsed = url::Url::parse(uri).ok()?;
    if !matches!(parsed.scheme(), "http" | "https")
        || parsed.host_str().is_none()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || !parsed.path().ends_with(CALLBACK_PATH)
    {
        return None;
    }
    // Keep the client's exact string: the token request must repeat it byte for byte.
    Some(uri.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovery_url_from_issuer() {
        assert_eq!(
            discovery_document_url("https://auth.example.com/application/o/pinepods/").unwrap(),
            "https://auth.example.com/application/o/pinepods/.well-known/openid-configuration"
        );
        assert_eq!(
            discovery_document_url("https://id.example.com/.well-known/openid-configuration").unwrap(),
            "https://id.example.com/.well-known/openid-configuration"
        );
        assert!(discovery_document_url("ftp://id.example.com").is_err());
        assert!(discovery_document_url("not a url").is_err());
    }

    #[test]
    fn redirect_uri_keeps_port_and_rejects_others() {
        assert_eq!(
            validate_redirect_uri("https://pods.example.com:12345/api/auth/callback").as_deref(),
            Some("https://pods.example.com:12345/api/auth/callback")
        );
        assert_eq!(
            validate_redirect_uri("https://example.com/pinepods/api/auth/callback").as_deref(),
            Some("https://example.com/pinepods/api/auth/callback")
        );
        assert!(validate_redirect_uri("https://evil.example.com/steal").is_none());
        assert!(validate_redirect_uri("pinepods://auth/callback").is_none());
        assert!(validate_redirect_uri("https://x.example.com/api/auth/callback?next=/").is_none());
    }

    #[test]
    fn discovery_document_parses_minimal_fields() {
        let doc: OidcDiscovery = serde_json::from_str(
            r#"{"issuer":"https://id.example.com","authorization_endpoint":"https://id.example.com/authorize",
                "token_endpoint":"https://id.example.com/token","userinfo_endpoint":"https://id.example.com/userinfo",
                "jwks_uri":"https://id.example.com/jwks"}"#,
        )
        .unwrap();
        assert_eq!(doc.token_endpoint, "https://id.example.com/token");
        assert!(doc.scopes_supported.is_empty());
    }
}
