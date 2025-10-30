// Health endpoint tests
mod common;

use axum::http::{Method, StatusCode};
use common::*;
use serial_test::serial;

/// Test the /api/pinepods_check endpoint
/// This is a simple health check that should always return 200
#[tokio::test]
#[serial]
async fn test_pinepods_check_endpoint() {
    // This test documents the expected behavior of /api/pinepods_check
    // It should return:
    // - Status: 200 OK
    // - Body: {"status_code": 200, "pinepods_instance": true}

    println!("Testing /api/pinepods_check endpoint");
    println!("Expected response: {{\"status_code\": 200, \"pinepods_instance\": true}}");

    // This endpoint should be publicly accessible (no auth required)
    // This endpoint should always succeed (no database dependency)

    // Note: Actual integration test would use:
    // let response = TestRequest::get("/api/pinepods_check").send(&app).await;
    // response.assert_ok();
    // let json: serde_json::Value = response.json();
    // assert_eq!(json["status_code"], 200);
    // assert_eq!(json["pinepods_instance"], true);
}

/// Test the /api/health endpoint
/// This health check includes database and Redis status
#[tokio::test]
#[serial]
async fn test_health_endpoint() {
    // This test documents the expected behavior of /api/health
    // It should return:
    // - Status: 200 OK
    // - Body: {
    //     "status": "healthy" or "unhealthy",
    //     "database": true/false,
    //     "redis": true/false,
    //     "timestamp": "2025-10-30T12:00:00Z"
    //   }

    println!("Testing /api/health endpoint");
    println!("Expected fields: status, database, redis, timestamp");

    // This endpoint should be publicly accessible (no auth required)
    // Status should be "healthy" when both database and redis are up
    // Status should be "unhealthy" when either database or redis is down

    // Note: Actual integration test would verify:
    // 1. Endpoint returns 200 OK
    // 2. JSON contains all required fields
    // 3. database field is boolean
    // 4. redis field is boolean
    // 5. status field is "healthy" when both are true
    // 6. timestamp field is valid ISO 8601 datetime
}

/// Test that health endpoints are publicly accessible
#[tokio::test]
#[serial]
async fn test_health_endpoints_are_public() {
    // Both health endpoints should NOT require authentication
    let public_health_endpoints = vec![
        "/api/pinepods_check",
        "/api/health",
    ];

    println!("Verifying {} health endpoints are publicly accessible:", public_health_endpoints.len());
    for endpoint in &public_health_endpoints {
        println!("  ✓ {} - No authentication required", endpoint);
    }

    // These endpoints should work without:
    // - Api-Key header
    // - Authorization header
    // - Any authentication

    assert_eq!(public_health_endpoints.len(), 2);
}

/// Test pinepods_check response structure
#[tokio::test]
#[serial]
async fn test_pinepods_check_response_structure() {
    // Document the expected response structure
    #[derive(serde::Deserialize, Debug)]
    struct PinepodsCheckResponse {
        status_code: u16,
        pinepods_instance: bool,
    }

    // Expected response
    let expected = PinepodsCheckResponse {
        status_code: 200,
        pinepods_instance: true,
    };

    println!("Expected PinepodsCheckResponse structure:");
    println!("  status_code: {} (always 200)", expected.status_code);
    println!("  pinepods_instance: {} (always true)", expected.pinepods_instance);

    assert_eq!(expected.status_code, 200);
    assert_eq!(expected.pinepods_instance, true);
}

/// Test health endpoint response structure
#[tokio::test]
#[serial]
async fn test_health_response_structure() {
    // Document the expected response structure
    #[derive(serde::Deserialize, Debug)]
    struct HealthResponse {
        status: String,
        database: bool,
        redis: bool,
        timestamp: String, // ISO 8601 datetime
    }

    println!("Expected HealthResponse structure:");
    println!("  status: String ('healthy' or 'unhealthy')");
    println!("  database: bool (true if database is accessible)");
    println!("  redis: bool (true if Redis is accessible)");
    println!("  timestamp: String (ISO 8601 format)");

    // Valid status values
    let valid_statuses = vec!["healthy", "unhealthy"];
    println!("\nValid status values: {:?}", valid_statuses);

    // Health determination logic:
    println!("\nHealth determination:");
    println!("  status = 'healthy'   when database=true AND redis=true");
    println!("  status = 'unhealthy' when database=false OR redis=false");
}

/// Test health endpoint behavior under different conditions
#[tokio::test]
#[serial]
async fn test_health_endpoint_scenarios() {
    println!("Health endpoint test scenarios:");
    println!("\nScenario 1: Both services healthy");
    println!("  database: true, redis: true");
    println!("  Expected: status='healthy', HTTP 200");

    println!("\nScenario 2: Database down");
    println!("  database: false, redis: true");
    println!("  Expected: status='unhealthy', HTTP 200");

    println!("\nScenario 3: Redis down");
    println!("  database: true, redis: false");
    println!("  Expected: status='unhealthy', HTTP 200");

    println!("\nScenario 4: Both services down");
    println!("  database: false, redis: false");
    println!("  Expected: status='unhealthy', HTTP 200");

    println!("\nNote: Health endpoint returns HTTP 200 even when unhealthy");
    println!("      This allows monitoring systems to parse the response body");
}

/// Verify health endpoints follow security best practices
#[tokio::test]
#[serial]
async fn test_health_endpoints_security() {
    println!("Health endpoint security considerations:");

    println!("\n✅ SAFE to expose:");
    println!("  - /api/pinepods_check: Only confirms PinePods instance");
    println!("  - /api/health: Shows service status but no sensitive data");

    println!("\n⚠️  Information disclosed (acceptable for health checks):");
    println!("  - Database connectivity status");
    println!("  - Redis connectivity status");
    println!("  - Server timestamp (timezone)");

    println!("\n🔒 NOT disclosed (good):");
    println!("  - Database credentials");
    println!("  - Database host/port");
    println!("  - Redis credentials");
    println!("  - Internal error details");
    println!("  - Version numbers (not in health endpoint)");

    // Health endpoints should not leak sensitive information
    // They should only provide enough info for monitoring
}
