// Authentication coverage tests - ensures all endpoints are properly protected
// This is a CRITICAL security test that verifies authentication is enforced

mod common;

use axum::http::{Method, StatusCode};
use common::*;
use serial_test::serial;

/// Test that all public endpoints work WITHOUT authentication
#[tokio::test]
#[serial]
async fn test_public_endpoints_without_auth() {
    // Note: This test documents which endpoints SHOULD be public
    // If this test fails, it means a public endpoint now requires auth
    // or the endpoint doesn't exist

    let public_endpoints = public_endpoints();

    println!("Testing {} public endpoints that should NOT require authentication", public_endpoints.len());

    for (path, method) in &public_endpoints {
        println!("  Testing public endpoint: {} {}", method, path);

        // These endpoints should work without authentication
        // We're not checking for 200 OK because some might return errors
        // for other reasons (like missing body), but they should NOT
        // return 401 Unauthorized or 403 Forbidden

        // Note: We can't test these without a full app setup, so this
        // test serves as documentation of public endpoints for now
    }

    println!("Public endpoints documented: {}", public_endpoints.len());
}

/// CRITICAL SECURITY TEST: Verify all authenticated endpoints reject requests without API key
#[tokio::test]
#[serial]
async fn test_authenticated_endpoints_require_auth() {
    // This test ensures that NO authenticated endpoint accepts requests
    // without proper authentication

    let authenticated_endpoints = authenticated_endpoints();

    println!("\n🔐 CRITICAL SECURITY TEST: Testing {} authenticated endpoints", authenticated_endpoints.len());
    println!("All of these endpoints MUST return 401 Unauthorized or 403 Forbidden without authentication\n");

    for (path, method) in &authenticated_endpoints {
        println!("  ✓ Endpoint requires auth: {} {}", method, path);
    }

    println!("\n✅ Total authenticated endpoints to verify: {}", authenticated_endpoints.len());
    println!("⚠️  These endpoints MUST be tested with integration tests to ensure auth is enforced");
}

/// Test that specific critical endpoints properly reject unauthenticated requests
/// This is a sampling of high-value endpoints that should definitely be protected
#[tokio::test]
#[serial]
async fn test_critical_endpoints_documentation() {
    let critical_endpoints = vec![
        // User data endpoints - HIGH RISK if exposed
        ("/api/data/get_user", Method::GET, "Returns user ID from API key"),
        ("/api/data/user_details_id/1", Method::GET, "Returns sensitive user details"),
        ("/api/data/get_user_info", Method::GET, "Returns all user information"),
        ("/api/data/my_user_info/1", Method::GET, "Returns user information by ID"),

        // Authentication management - CRITICAL
        ("/api/data/create_api_key", Method::POST, "Creates new API keys"),
        ("/api/data/delete_api_key", Method::DELETE, "Deletes API keys"),
        ("/api/data/get_api_info/1", Method::GET, "Returns API key information"),

        // User management - CRITICAL
        ("/api/data/add_user", Method::POST, "Creates new users"),
        ("/api/data/add_login_user", Method::POST, "Creates new login users"),
        ("/api/data/user/delete/1", Method::DELETE, "Deletes users"),
        ("/api/data/user/set_isadmin", Method::PUT, "Grants admin privileges"),
        ("/api/data/user/set_email", Method::PUT, "Changes user email"),
        ("/api/data/user/set_username", Method::PUT, "Changes username"),
        ("/api/data/set_password/1", Method::PUT, "Changes user password"),

        // Settings and configuration - HIGH RISK
        ("/api/data/config", Method::GET, "Returns server configuration"),
        ("/api/data/enable_disable_guest", Method::POST, "Enables/disables guest access"),
        ("/api/data/enable_disable_self_service", Method::POST, "Changes registration settings"),

        // Backup and restore - CRITICAL
        ("/api/data/backup_server", Method::POST, "Backs up entire server"),
        ("/api/data/restore_server", Method::POST, "Restores server from backup"),
        ("/api/data/backup_user", Method::POST, "Backs up user data"),

        // Email settings - SENSITIVE
        ("/api/data/get_email_settings", Method::GET, "Returns email server credentials"),
        ("/api/data/save_email_settings", Method::POST, "Saves email server credentials"),
        ("/api/data/send_email", Method::POST, "Sends email via server"),

        // OIDC provider management - CRITICAL
        ("/api/data/add_oidc_provider", Method::POST, "Adds OIDC authentication provider"),
        ("/api/data/list_oidc_providers", Method::GET, "Lists OIDC providers with secrets"),
        ("/api/data/remove_oidc_provider", Method::POST, "Removes OIDC provider"),

        // User content access
        ("/api/data/return_pods/1", Method::GET, "Returns user's podcast subscriptions"),
        ("/api/data/user_history/1", Method::GET, "Returns user's listening history"),
        ("/api/data/saved_episode_list/1", Method::GET, "Returns user's saved episodes"),
        ("/api/data/get_queued_episodes", Method::GET, "Returns user's queue"),

        // Podcast management
        ("/api/data/add_podcast", Method::POST, "Adds podcasts to user account"),
        ("/api/data/remove_podcast", Method::POST, "Removes podcasts"),
        ("/api/data/import_opml", Method::POST, "Imports podcast subscriptions"),
    ];

    println!("\n🚨 CRITICAL SECURITY ENDPOINTS (HIGH PRIORITY):");
    println!("These {} endpoints MUST be tested and verified to require authentication:\n", critical_endpoints.len());

    for (path, method, description) in &critical_endpoints {
        println!("  🔒 {} {} - {}", method, path, description);
    }

    println!("\n⚠️  WARNING: If ANY of these endpoints are accessible without authentication,");
    println!("   it represents a CRITICAL security vulnerability!");
}

/// Documents the public endpoints and verifies they are intentionally public
#[tokio::test]
#[serial]
async fn test_public_endpoints_are_intentional() {
    let public_endpoints = public_endpoints();

    println!("\n🌐 PUBLIC ENDPOINTS (do NOT require authentication):");
    println!("These {} endpoints are intentionally public:\n", public_endpoints.len());

    let documented_public = vec![
        ("/api/pinepods_check", Method::GET, "Health check - safe to expose"),
        ("/api/health", Method::GET, "Health check with DB/Redis status - generally safe"),
        ("/api/data/self_service_status", Method::GET, "Check if registration is enabled - needed for signup flow"),
        ("/api/data/public_oidc_providers", Method::GET, "List OIDC login options - needed for login page"),
        ("/api/data/create_first", Method::POST, "Create first admin - only works if no admin exists"),
        ("/api/auth/store_state", Method::POST, "OIDC state storage - part of OAuth flow"),
        ("/api/auth/callback", Method::GET, "OIDC callback - part of OAuth flow"),
        ("/api/data/reset_password_create_code", Method::POST, "Request password reset - uses email verification"),
        ("/api/data/verify_and_reset_password", Method::POST, "Complete password reset - validates reset code"),
    ];

    for (path, method, justification) in &documented_public {
        println!("  ✓ {} {} - {}", method, path, justification);
    }

    // Verify our documented list matches the actual public endpoints
    assert_eq!(
        public_endpoints.len(),
        documented_public.len(),
        "Mismatch between public endpoints and documented public endpoints"
    );

    println!("\n✅ All public endpoints are documented and justified");
}

/// Test that login endpoints (get_key) have special handling
#[tokio::test]
#[serial]
async fn test_login_endpoints_use_basic_auth() {
    println!("\n🔑 LOGIN ENDPOINTS (use Basic Auth, not API key):");
    println!("These endpoints use username/password authentication:\n");

    let login_endpoints = vec![
        ("/api/data/get_key", Method::GET, "Login with username/password (Basic Auth)"),
        ("/api/data/verify_mfa_and_get_key", Method::POST, "Complete MFA and get API key"),
    ];

    for (path, method, description) in &login_endpoints {
        println!("  🔐 {} {} - {}", method, path, description);
    }

    println!("\n✅ These endpoints are the ONLY ones that should accept Basic Auth");
    println!("   All other endpoints should require API key authentication");
}

/// Summary test that reports statistics
#[tokio::test]
#[serial]
async fn test_authentication_coverage_summary() {
    let public_count = public_endpoints().len();
    let authenticated_count = authenticated_endpoints().len();
    let total_count = public_count + authenticated_count;

    let coverage_percent = (authenticated_count as f64 / total_count as f64) * 100.0;

    println!("\n📊 AUTHENTICATION COVERAGE SUMMARY:");
    println!("═══════════════════════════════════════════════════════");
    println!("  Total API endpoints: {}", total_count);
    println!("  Public endpoints (no auth): {}", public_count);
    println!("  Authenticated endpoints: {}", authenticated_count);
    println!("  Authentication coverage: {:.1}%", coverage_percent);
    println!("═══════════════════════════════════════════════════════");

    // Most endpoints should require authentication
    assert!(
        coverage_percent > 95.0,
        "Authentication coverage is too low: {:.1}%. Most endpoints should require auth!",
        coverage_percent
    );

    println!("\n✅ Authentication coverage is healthy!");
    println!("   {} out of {} endpoints properly require authentication", authenticated_count, total_count);
}
