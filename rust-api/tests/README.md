# PinePods Rust API Tests

This directory contains the test suite for the PinePods Rust API.

## Test Structure

```
tests/
├── common/
│   └── mod.rs              # Shared test utilities and helpers
├── auth_coverage_test.rs   # Authentication coverage tests (CRITICAL SECURITY)
├── auth_endpoints_test.rs  # Authentication endpoint behavior tests
├── health_test.rs          # Health check endpoint tests
└── README.md               # This file
```

## Test Categories

### 1. Authentication Coverage Tests (`auth_coverage_test.rs`)

**PURPOSE**: Ensures ALL endpoints are properly protected with authentication.

This is the **most critical security test** in the suite. It verifies that:
- All public endpoints are documented and intentionally public
- All authenticated endpoints require API key authentication
- No endpoints are accidentally exposed without authentication

**Tests include:**
- `test_authenticated_endpoints_require_auth` - Documents all 200+ authenticated endpoints
- `test_critical_endpoints_documentation` - Highlights high-risk endpoints
- `test_public_endpoints_are_intentional` - Verifies public endpoints are justified
- `test_authentication_coverage_summary` - Reports >95% auth coverage

**Why this matters**: If any authenticated endpoint is accessible without a key, it's a **CRITICAL security vulnerability** that could expose user data, allow unauthorized actions, or compromise the system.

### 2. Health Endpoint Tests (`health_test.rs`)

Tests for the health check endpoints:
- `/api/pinepods_check` - Simple instance check
- `/api/health` - Database and Redis status

These tests document:
- Expected response structures
- Public accessibility (no auth required)
- Information disclosure (acceptable for monitoring)

### 3. Authentication Endpoint Tests (`auth_endpoints_test.rs`)

Tests for authentication flows and endpoints:
- **Login flow**: Username/password authentication
- **MFA flow**: Two-factor authentication
- **API key operations**: Verification and user lookup
- **OIDC flow**: OAuth2/OpenID Connect authentication
- **Password reset flow**: Email-based password recovery

These tests document:
- Request/response formats
- Security features (session expiration, single-use codes, etc.)
- Authorization requirements (user vs admin)
- Attack prevention mechanisms

## Running Tests

### Run all tests:
```bash
cd rust-api
cargo test
```

### Run specific test file:
```bash
cargo test --test auth_coverage_test
cargo test --test health_test
cargo test --test auth_endpoints_test
```

### Run with output (see println! statements):
```bash
cargo test -- --nocapture
```

### Run tests in CI:
Tests run automatically on:
- Pull requests to `main`
- Pushes to `main`
- Manual workflow dispatch

See `.github/workflows/ci.yaml` for the `rust-api-tests` job configuration.

## Test Environment

Tests use environment variables for configuration:

```bash
# Database (PostgreSQL or MySQL)
TEST_DB_TYPE=postgresql
TEST_DB_HOST=localhost
TEST_DB_PORT=5432
TEST_DB_USER=test_user
TEST_DB_PASSWORD=test_password
TEST_DB_NAME=test_db

# Redis/Valkey
TEST_REDIS_HOST=localhost
TEST_REDIS_PORT=6379

# API URLs (required by config)
SEARCH_API_URL=https://search.pinepods.online/api/search
PEOPLE_API_URL=https://people.pinepods.online
```

## Test Utilities (`common/mod.rs`)

Shared utilities for all tests:

### Test Configuration
```rust
let config = test_config();
// Creates a test Config with defaults that can be overridden by env vars
```

### HTTP Request Builder
```rust
let response = TestRequest::get("/api/health")
    .api_key("test-key")
    .send(&app)
    .await;

response.assert_ok();
let json: Value = response.json();
```

### Endpoint Lists
```rust
let public = public_endpoints();        // List of public endpoints
let authenticated = authenticated_endpoints();  // List of auth-required endpoints
```

### Test Data Generators
```rust
let username = random_username();
let email = random_email();
let password = random_password();
```

## Current Test Coverage

### ✅ Implemented
- **Authentication coverage** (200+ endpoints documented)
- **Health endpoints** (behavior documentation)
- **Authentication flows** (login, MFA, OIDC, password reset)
- **CI integration** (automated testing on PRs)

### 🚧 To Be Implemented
- **Integration tests** (actual HTTP requests to endpoints)
- **Podcast endpoint tests**
- **Episode endpoint tests**
- **Settings endpoint tests**
- **User management tests**
- **Database integration tests**

## Security Testing Priority

These endpoints are **CRITICAL** and must be verified to require authentication:

### User Data & Authentication
- `/api/data/get_user` - Returns user ID from API key
- `/api/data/user_details_id/{id}` - Returns sensitive user details
- `/api/data/create_api_key` - Creates new API keys
- `/api/data/delete_api_key` - Deletes API keys

### User Management
- `/api/data/add_user` - Creates new users
- `/api/data/user/delete/{id}` - Deletes users
- `/api/data/user/set_isadmin` - Grants admin privileges
- `/api/data/set_password/{id}` - Changes passwords

### Server Operations
- `/api/data/backup_server` - Backs up entire server
- `/api/data/restore_server` - Restores from backup
- `/api/data/config` - Returns server configuration
- `/api/data/get_email_settings` - Returns email credentials

### OIDC Configuration
- `/api/data/add_oidc_provider` - Adds authentication provider
- `/api/data/list_oidc_providers` - Lists providers with secrets
- `/api/data/remove_oidc_provider` - Removes provider

## Public Endpoints (No Auth Required)

Only these endpoints should be accessible without authentication:

1. `/api/pinepods_check` - Health check
2. `/api/health` - Service status
3. `/api/data/self_service_status` - Registration settings
4. `/api/data/public_oidc_providers` - Login options
5. `/api/data/create_first` - First admin creation (guarded)
6. `/api/auth/store_state` - OIDC state storage
7. `/api/auth/callback` - OIDC callback
8. `/api/data/reset_password_create_code` - Password reset request
9. `/api/data/verify_and_reset_password` - Password reset completion

**All other endpoints MUST require authentication.**

## Adding New Tests

### 1. Create a new test file:
```rust
// tests/my_feature_test.rs
mod common;

use common::*;
use serial_test::serial;

#[tokio::test]
#[serial]
async fn test_my_feature() {
    // Test implementation
}
```

### 2. Update endpoint lists in `common/mod.rs`:
```rust
pub fn authenticated_endpoints() -> Vec<(&'static str, Method)> {
    vec![
        // ... existing endpoints
        ("/api/my_new_endpoint", Method::GET),  // Add new endpoint
    ]
}
```

### 3. Run the tests:
```bash
cargo test --test my_feature_test
```

## Best Practices

1. **Always document public endpoints**: If an endpoint doesn't require auth, document WHY
2. **Test authorization, not just authentication**: Verify users can only access their own data
3. **Test error cases**: Invalid inputs, missing parameters, malformed requests
4. **Use serial_test**: Prevents test interference when sharing resources
5. **Document security implications**: Note what information is disclosed

## Continuous Integration

The `rust-api-tests` job in GitHub Actions:
- Runs on every PR and push to main
- Sets up PostgreSQL and Redis services
- Caches Cargo dependencies for faster builds
- Runs all tests with verbose output
- Fails the build if any test fails

## Resources

- [Rust Testing Book](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [Axum Testing Guide](https://docs.rs/axum/latest/axum/extract/index.html#testing)
- [Tower Test Utilities](https://docs.rs/tower-test/latest/tower_test/)
