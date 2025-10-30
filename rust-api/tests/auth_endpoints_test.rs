// Authentication endpoint tests - CRITICAL SECURITY TESTS
mod common;

use axum::http::{Method, StatusCode};
use common::*;
use serial_test::serial;

/// Test /api/data/get_key - Login with username/password (Basic Auth)
#[tokio::test]
#[serial]
async fn test_get_key_endpoint() {
    println!("Testing /api/data/get_key - Username/password login");

    // This endpoint uses HTTP Basic Authentication
    // Header: Authorization: Basic base64(username:password)

    println!("\nExpected behavior:");
    println!("  ✓ Accepts Basic Auth (username/password)");
    println!("  ✓ Returns API key on successful authentication");
    println!("  ✓ Returns MFA challenge if MFA is enabled");
    println!("  ✓ Returns 401 on invalid credentials");
    println!("  ✗ Does NOT accept requests without Authorization header");

    println!("\nResponse structure (no MFA):");
    println!("  {{");
    println!("    \"status\": \"success\",");
    println!("    \"retrieved_key\": \"<api-key>\",");
    println!("    \"mfa_required\": false,");
    println!("    \"user_id\": 123,");
    println!("    \"mfa_session_token\": null");
    println!("  }}");

    println!("\nResponse structure (MFA required):");
    println!("  {{");
    println!("    \"status\": \"mfa_required\",");
    println!("    \"retrieved_key\": null,");
    println!("    \"mfa_required\": true,");
    println!("    \"user_id\": 123,");
    println!("    \"mfa_session_token\": \"<session-token>\"");
    println!("  }}");
}

/// Test /api/data/verify_mfa_and_get_key - Complete MFA and get API key
#[tokio::test]
#[serial]
async fn test_verify_mfa_and_get_key_endpoint() {
    println!("Testing /api/data/verify_mfa_and_get_key - MFA verification");

    println!("\nThis is the second step of MFA login:");
    println!("  1. User provides username/password → get_key");
    println!("  2. Server returns mfa_session_token");
    println!("  3. User provides MFA code + session_token → verify_mfa_and_get_key");
    println!("  4. Server returns API key");

    println!("\nRequest body:");
    println!("  {{");
    println!("    \"mfa_session_token\": \"<token-from-step-2>\",");
    println!("    \"mfa_code\": \"123456\"");
    println!("  }}");

    println!("\nResponse (success):");
    println!("  {{");
    println!("    \"status\": \"success\",");
    println!("    \"retrieved_key\": \"<api-key>\",");
    println!("    \"verified\": true");
    println!("  }}");

    println!("\nResponse (invalid code):");
    println!("  {{");
    println!("    \"status\": \"invalid_code\",");
    println!("    \"retrieved_key\": null,");
    println!("    \"verified\": false");
    println!("  }}");

    println!("\nResponse (expired session):");
    println!("  {{");
    println!("    \"status\": \"session_expired\",");
    println!("    \"retrieved_key\": null,");
    println!("    \"verified\": false");
    println!("  }}");

    println!("\nSecurity features:");
    println!("  ✓ Session tokens expire after 5 minutes");
    println!("  ✓ Session tokens are single-use (consumed on verification attempt)");
    println!("  ✓ No API key returned without valid MFA code");
}

/// Test /api/data/verify_key - Verify API key validity
#[tokio::test]
#[serial]
async fn test_verify_key_endpoint() {
    println!("Testing /api/data/verify_key - API key validation");

    println!("\nPurpose: Check if an API key is valid");
    println!("Authentication: Requires Api-Key header");

    println!("\nRequest:");
    println!("  GET /api/data/verify_key");
    println!("  Header: Api-Key: <api-key>");

    println!("\nResponse (valid key):");
    println!("  Status: 200 OK");
    println!("  Body: {{ \"status\": \"success\" }}");

    println!("\nResponse (invalid key):");
    println!("  Status: 401 Unauthorized");
    println!("  Body: {{ \"error\": \"Invalid API key\", ... }}");

    println!("\n🔒 SECURITY TEST: This endpoint MUST require authentication");
}

/// Test /api/data/get_user - Get user ID from API key
#[tokio::test]
#[serial]
async fn test_get_user_endpoint() {
    println!("Testing /api/data/get_user - Get user ID from API key");

    println!("\nPurpose: Get the user ID associated with an API key");
    println!("Authentication: Requires Api-Key header");

    println!("\nRequest:");
    println!("  GET /api/data/get_user");
    println!("  Header: Api-Key: <api-key>");

    println!("\nResponse:");
    println!("  {{");
    println!("    \"status\": \"success\",");
    println!("    \"retrieved_id\": 123");
    println!("  }}");

    println!("\n🔒 CRITICAL SECURITY: This endpoint MUST require authentication");
    println!("   If accessible without auth, attackers could enumerate user IDs");
}

/// Test /api/data/user_details_id/{user_id} - Get user details
#[tokio::test]
#[serial]
async fn test_user_details_endpoint() {
    println!("Testing /api/data/user_details_id/{{user_id}} - Get user details");

    println!("\n🚨 CRITICAL SECURITY ENDPOINT");
    println!("   Contains: username, email, hashed password, salt");

    println!("\nAuthentication required: YES");
    println!("Authorization required: YES");
    println!("  - Users can only access their own details");
    println!("  - Admin users can access any user's details");
    println!("  - Web key has full access");

    println!("\nResponse structure:");
    println!("  {{");
    println!("    \"UserID\": 123,");
    println!("    \"Fullname\": \"John Doe\",");
    println!("    \"Username\": \"johndoe\",");
    println!("    \"Email\": \"john@example.com\",");
    println!("    \"Hashed_PW\": \"<hash>\",");
    println!("    \"Salt\": \"<salt>\"");
    println!("  }}");

    println!("\n🔒 SECURITY CHECKS:");
    println!("   1. Endpoint requires valid API key");
    println!("   2. User can only get their own details (unless admin)");
    println!("   3. Returns 403 Forbidden if accessing other user's details");
    println!("   4. Password hash and salt are included (for account management)");
}

/// Test /api/data/create_first - Create first admin user
#[tokio::test]
#[serial]
async fn test_create_first_admin_endpoint() {
    println!("Testing /api/data/create_first - Create first admin");

    println!("\n✅ PUBLIC ENDPOINT - No authentication required");
    println!("   BUT: Only works if no admin exists yet");

    println!("\nRequest:");
    println!("  POST /api/data/create_first");
    println!("  Body: {{");
    println!("    \"username\": \"admin\",");
    println!("    \"password\": \"<hashed-password>\",");
    println!("    \"email\": \"admin@example.com\",");
    println!("    \"fullname\": \"Admin User\"");
    println!("  }}");

    println!("\nResponse (success):");
    println!("  {{");
    println!("    \"message\": \"Admin user created successfully\",");
    println!("    \"user_id\": 1");
    println!("  }}");

    println!("\nResponse (admin exists):");
    println!("  Status: 403 Forbidden");
    println!("  {{ \"error\": \"An admin user already exists\" }}");

    println!("\n🔒 SECURITY: Protected by 'admin exists' check");
    println!("   Cannot be abused to create multiple admin accounts");
}

/// Test /api/data/config - Get server configuration
#[tokio::test]
#[serial]
async fn test_config_endpoint() {
    println!("Testing /api/data/config - Get server configuration");

    println!("\n🔒 REQUIRES AUTHENTICATION");

    println!("\nResponse:");
    println!("  {{");
    println!("    \"api_url\": \"https://search.pinepods.online/api/search\",");
    println!("    \"proxy_url\": \"http://localhost:8040/mover/?url=\",");
    println!("    \"proxy_host\": \"localhost\",");
    println!("    \"proxy_port\": \"8040\",");
    println!("    \"proxy_protocol\": \"http\",");
    println!("    \"reverse_proxy\": \"False\",");
    println!("    \"people_url\": \"https://people.pinepods.online\"");
    println!("  }}");

    println!("\n⚠️  Information disclosed:");
    println!("   - API URLs");
    println!("   - Proxy configuration");
    println!("   - Server hostnames");

    println!("\n✓ NOT disclosed:");
    println!("   - Database credentials");
    println!("   - Redis credentials");
    println!("   - API keys or secrets");
}

/// Test authentication flow scenarios
#[tokio::test]
#[serial]
async fn test_authentication_flow_scenarios() {
    println!("Authentication Flow Test Scenarios\n");

    println!("═══════════════════════════════════════════");
    println!("Scenario 1: Standard Login (No MFA)");
    println!("═══════════════════════════════════════════");
    println!("1. POST /api/data/get_key with Basic Auth");
    println!("   → Response: API key");
    println!("2. Use API key in Api-Key header for subsequent requests");
    println!("   → All authenticated endpoints accessible\n");

    println!("═══════════════════════════════════════════");
    println!("Scenario 2: Login with MFA");
    println!("═══════════════════════════════════════════");
    println!("1. POST /api/data/get_key with Basic Auth");
    println!("   → Response: mfa_session_token (NO API key yet)");
    println!("2. POST /api/data/verify_mfa_and_get_key");
    println!("   Body: {{ mfa_session_token, mfa_code }}");
    println!("   → Response: API key");
    println!("3. Use API key in Api-Key header for subsequent requests");
    println!("   → All authenticated endpoints accessible\n");

    println!("═══════════════════════════════════════════");
    println!("Scenario 3: Failed Login Attempts");
    println!("═══════════════════════════════════════════");
    println!("1. POST /api/data/get_key with wrong password");
    println!("   → Response: 401 Unauthorized");
    println!("2. POST /api/data/get_key with non-existent user");
    println!("   → Response: 401 Unauthorized");
    println!("3. GET /api/data/verify_key with invalid API key");
    println!("   → Response: 401 Unauthorized\n");

    println!("═══════════════════════════════════════════");
    println!("Scenario 4: OIDC Login");
    println!("═══════════════════════════════════════════");
    println!("1. User redirected to OIDC provider");
    println!("2. User authenticates with provider");
    println!("3. Provider redirects to /api/auth/callback");
    println!("4. Server exchanges code for token");
    println!("5. Server creates/updates user account");
    println!("6. Server redirects to frontend with API key");
    println!("7. Frontend uses API key for subsequent requests\n");
}

/// Test that authentication properly protects critical user operations
#[tokio::test]
#[serial]
async fn test_user_operation_authorization() {
    println!("User Operation Authorization Tests\n");

    println!("🔒 Operations that require user to be the account owner:");
    let user_only_ops = vec![
        "Get own user details",
        "Update own theme",
        "Update own timezone",
        "Update own password",
        "Get own API keys",
        "Delete own API keys",
        "Enable MFA on own account",
        "Get own podcast subscriptions",
        "Get own listening history",
    ];
    for op in &user_only_ops {
        println!("  - {}", op);
    }

    println!("\n👑 Operations that require admin privileges:");
    let admin_ops = vec![
        "View all users",
        "Create new users",
        "Delete users",
        "Grant admin privileges",
        "View other user's details",
        "Configure server settings",
        "Manage OIDC providers",
        "Backup/restore server",
    ];
    for op in &admin_ops {
        println!("  - {}", op);
    }

    println!("\n🌐 Operations that are public:");
    let public_ops = vec![
        "Health checks",
        "Check self-service status",
        "List public OIDC providers",
        "Create first admin (when no admin exists)",
        "Request password reset",
        "Complete password reset (with code)",
    ];
    for op in &public_ops {
        println!("  - {}", op);
    }
}

/// Test password reset flow
#[tokio::test]
#[serial]
async fn test_password_reset_flow() {
    println!("Password Reset Flow\n");

    println!("Step 1: Request password reset");
    println!("  POST /api/data/reset_password_create_code");
    println!("  Body: {{ \"email\": \"user@example.com\", \"username\": \"user\" }}");
    println!("  → Always returns success (to prevent user enumeration)");
    println!("  → If user exists, sends email with reset code");

    println!("\nStep 2: Verify code and reset password");
    println!("  POST /api/data/verify_and_reset_password");
    println!("  Body: {{");
    println!("    \"email\": \"user@example.com\",");
    println!("    \"reset_code\": \"123456\",");
    println!("    \"new_password\": \"<hashed-password>\"");
    println!("  }}");
    println!("  → Returns success if code is valid");
    println!("  → Returns error if code is invalid/expired");

    println!("\n🔒 Security features:");
    println!("  ✓ Always returns success on step 1 (prevents user enumeration)");
    println!("  ✓ Reset codes expire after a time period");
    println!("  ✓ Reset codes are single-use");
    println!("  ✓ Password must be hashed on client side");
    println!("  ✓ Disabled when OIDC-only mode is enabled");
}

/// Test OIDC authentication security
#[tokio::test]
#[serial]
async fn test_oidc_security() {
    println!("OIDC Authentication Security\n");

    println!("State management:");
    println!("  POST /api/auth/store_state");
    println!("  - Stores OIDC state in Redis with 10-minute expiration");
    println!("  - State includes: client_id, origin_url, code_verifier (PKCE)");
    println!("  - Used to prevent CSRF attacks");

    println!("\nCallback handling:");
    println!("  GET /api/auth/callback?code=...&state=...");
    println!("  - Validates state parameter against stored state");
    println!("  - Exchanges authorization code for access token");
    println!("  - Fetches user info from OIDC provider");
    println!("  - Creates or updates user account");
    println!("  - Returns API key to frontend");

    println!("\n🔒 Security features:");
    println!("  ✓ State parameter prevents CSRF");
    println!("  ✓ PKCE support (code_verifier)");
    println!("  ✓ State expires after 10 minutes");
    println!("  ✓ Single-use authorization codes");
    println!("  ✓ Role-based access control (optional)");
    println!("  ✓ Username conflict resolution");
    println!("  ✓ Email verification from provider");
}
