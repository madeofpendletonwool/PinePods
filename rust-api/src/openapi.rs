//! OpenAPI document definition for the PinePods API.
//!
//! The concrete paths/schemas are collected automatically from the
//! `#[utoipa::path]`-annotated handlers registered on the `OpenApiRouter`
//! (see `openapi_router()` in `main.rs`). This module only seeds the static
//! pieces: API info, tags, and the `Api-Key` security scheme.

use utoipa::openapi::security::{ApiKey, ApiKeyValue, SecurityScheme};
use utoipa::{Modify, OpenApi};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "PinePods API",
        version = env!("CARGO_PKG_VERSION"),
        description = "HTTP API for the PinePods podcast server. Most endpoints require an \
            `Api-Key` header tied to a user account; admin-only endpoints additionally \
            require an admin (or web) key. This reference is generated directly from the \
            backend source, so it stays in sync with the running code.",
        license(name = "GPL-3.0-or-later"),
    ),
    servers(
        (url = "/", description = "This PinePods server (same origin as the API)"),
    ),
    modifiers(&SecurityAddon),
    tags(
        (name = "health", description = "Health and instance checks (no auth)"),
        (name = "auth", description = "Authentication, sessions, MFA, and OIDC"),
        (name = "podcasts", description = "Podcast subscriptions, episodes, queue, and playback"),
        (name = "episodes", description = "Episode actions: download, save, share, bulk ops"),
        (name = "playlists", description = "Smart and manual playlists"),
        (name = "settings", description = "User and server settings"),
        (name = "sync", description = "gpodder / Nextcloud synchronization"),
        (name = "tasks", description = "Background tasks and progress"),
        (name = "feed", description = "Public RSS feed generation"),
        (name = "proxy", description = "Media and image proxying"),
        (name = "local", description = "Local podcasts and media"),
        (name = "youtube", description = "YouTube channel integration"),
    ),
)]
pub struct ApiDoc;

/// Registers the `Api-Key` header security scheme used across the API.
struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "api_key",
            SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::new("Api-Key"))),
        );
    }
}
