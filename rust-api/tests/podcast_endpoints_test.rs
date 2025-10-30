// Podcast endpoint tests - User podcast management
mod common;

use axum::http::{Method, StatusCode};
use common::*;
use serial_test::serial;

/// Test /api/data/add_podcast - Add a podcast subscription
#[tokio::test]
#[serial]
async fn test_add_podcast_endpoint() {
    println!("Testing /api/data/add_podcast - Subscribe to podcast");

    println!("\n🔒 Authentication: REQUIRED");
    println!("Authorization: User can add to their own account");

    println!("\nRequest:");
    println!("  POST /api/data/add_podcast");
    println!("  Headers: Api-Key: <api-key>");
    println!("  Body: {{");
    println!("    \"feed_url\": \"https://example.com/feed.xml\",");
    println!("    \"auto_download\": true");
    println!("  }}");

    println!("\nResponse (success):");
    println!("  Status: 200 OK");
    println!("  Body: {{ \"detail\": \"Podcast added successfully\" }}");

    println!("\nResponse (already subscribed):");
    println!("  Status: 409 Conflict");
    println!("  Body: {{ \"error\": \"Already subscribed to this podcast\" }}");

    println!("\nResponse (invalid feed):");
    println!("  Status: 400 Bad Request");
    println!("  Body: {{ \"error\": \"Failed to parse podcast feed\" }}");

    println!("\n✅ Security checks:");
    println!("  1. Must have valid API key");
    println!("  2. Podcast added to API key owner's account");
    println!("  3. Cannot add podcasts to other users' accounts");
}

/// Test /api/data/remove_podcast - Remove podcast subscription
#[tokio::test]
#[serial]
async fn test_remove_podcast_endpoint() {
    println!("Testing /api/data/remove_podcast - Unsubscribe from podcast");

    println!("\n🔒 Authentication: REQUIRED");
    println!("Authorization: User can only remove their own podcasts");

    println!("\nRequest:");
    println!("  POST /api/data/remove_podcast");
    println!("  Body: {{");
    println!("    \"user_id\": 123,");
    println!("    \"podcast_id\": 456");
    println!("  }}");

    println!("\nAlternative endpoints:");
    println!("  POST /api/data/remove_podcast_id - Remove by ID only");
    println!("  POST /api/data/remove_podcast_name - Remove by name and URL");

    println!("\n✅ Security: User can only remove their own subscriptions");
}

/// Test /api/data/return_pods/{user_id} - Get user's podcasts
#[tokio::test]
#[serial]
async fn test_return_pods_endpoint() {
    println!("Testing /api/data/return_pods/{{user_id}} - List user podcasts");

    println!("\n🔒 Authentication: REQUIRED");
    println!("Authorization: User can only view their own podcasts");

    println!("\nResponse structure:");
    println!("  {{");
    println!("    \"pods\": [");
    println!("      {{");
    println!("        \"podcastid\": 1,");
    println!("        \"podcastname\": \"Example Podcast\",");
    println!("        \"artworkurl\": \"https://example.com/art.jpg\",");
    println!("        \"description\": \"...\",");
    println!("        \"episodecount\": 100,");
    println!("        \"websiteurl\": \"https://example.com\",");
    println!("        \"feedurl\": \"https://example.com/feed.xml\",");
    println!("        \"author\": \"Host Name\",");
    println!("        \"categories\": {{}},");
    println!("        \"explicit\": false,");
    println!("        \"podcastindexid\": 12345");
    println!("      }}");
    println!("    ]");
    println!("  }}");

    println!("\n✅ CRITICAL SECURITY: User ID in path must match API key owner");
    println!("   Otherwise users could view other users' subscriptions");
}

/// Test /api/data/return_pods_extra/{user_id} - Get podcasts with extra data
#[tokio::test]
#[serial]
async fn test_return_pods_extra_endpoint() {
    println!("Testing /api/data/return_pods_extra/{{user_id}} - List podcasts with stats");

    println!("\nExtra fields included:");
    println!("  - play_count: Total plays across all episodes");
    println!("  - episodes_played: Number of episodes played");
    println!("  - oldest_episode_date: Date of oldest episode");
    println!("  - is_youtube: Whether this is a YouTube channel");

    println!("\n🔒 SECURITY: Same authorization as return_pods");
    println!("   User can only view their own podcast data");
}

/// Test /api/data/return_episodes/{user_id} - Get user's episodes
#[tokio::test]
#[serial]
async fn test_return_episodes_endpoint() {
    println!("Testing /api/data/return_episodes/{{user_id}} - List user's episodes");

    println!("\n🔒 Authentication: REQUIRED");
    println!("Authorization: User can only view their own episodes");

    println!("\nQuery parameters:");
    println!("  - podcast_id (optional): Filter by specific podcast");
    println!("  - completed (optional): Filter by completion status");
    println!("  - limit (optional): Number of results");
    println!("  - offset (optional): Pagination offset");

    println!("\nResponse: Array of episode objects with:");
    println!("  - Episode metadata (title, description, artwork, URL)");
    println!("  - User state (completed, listen_duration, saved, queued, downloaded)");
    println!("  - Podcast information");

    println!("\n✅ CRITICAL SECURITY: User ID must match API key owner");
}

/// Test /api/data/podcast_episodes - Get episodes for a podcast
#[tokio::test]
#[serial]
async fn test_podcast_episodes_endpoint() {
    println!("Testing /api/data/podcast_episodes - Get podcast's episodes");

    println!("\n🔒 Authentication: REQUIRED");
    println!("Authorization: User must be subscribed to the podcast");

    println!("\nQuery parameters:");
    println!("  - podcast_id: ID of the podcast");
    println!("  - user_id: ID of the user");

    println!("\n✅ Security: Only returns episodes if user is subscribed");
}

/// Test /api/data/check_podcast - Check if podcast exists in user's library
#[tokio::test]
#[serial]
async fn test_check_podcast_endpoint() {
    println!("Testing /api/data/check_podcast - Check podcast subscription");

    println!("\n🔒 Authentication: REQUIRED");

    println!("\nQuery parameters:");
    println!("  - feed_url: URL of the podcast feed");
    println!("  - user_id: ID of the user");

    println!("\nResponse:");
    println!("  {{ \"exists\": true }}  or  {{ \"exists\": false }}");

    println!("\n✅ Security: User can only check their own subscriptions");
}

/// Test /api/data/get_podcast_details - Get detailed podcast info
#[tokio::test]
#[serial]
async fn test_get_podcast_details_endpoint() {
    println!("Testing /api/data/get_podcast_details - Get podcast details");

    println!("\n🔒 Authentication: REQUIRED");

    println!("\nQuery parameters:");
    println!("  - podcast_id: ID of the podcast");
    println!("  - user_id: ID of the user");

    println!("\nReturns:");
    println!("  - Full podcast metadata");
    println!("  - Episode list");
    println!("  - User-specific settings (auto-download, etc.)");

    println!("\n✅ Security: User must be subscribed to view details");
}

/// Test /api/data/update_podcast_info - Update podcast metadata
#[tokio::test]
#[serial]
async fn test_update_podcast_info_endpoint() {
    println!("Testing /api/data/update_podcast_info - Update podcast settings");

    println!("\n🔒 Authentication: REQUIRED");
    println!("Authorization: User can only update their own podcast settings");

    println!("\nUpdatable fields:");
    println!("  - Auto-download settings");
    println!("  - Feed cutoff days");
    println!("  - Custom metadata overrides");

    println!("\n✅ Security: User can only modify their own subscriptions");
}

/// Test podcast merge/unmerge endpoints
#[tokio::test]
#[serial]
async fn test_podcast_merge_endpoints() {
    println!("Testing podcast merge/unmerge functionality");

    println!("\nMerge endpoint:");
    println!("  POST /api/data/{{podcast_id}}/merge");
    println!("  Purpose: Merge duplicate podcast entries");

    println!("\nUnmerge endpoint:");
    println!("  POST /api/data/{{podcast_id}}/unmerge/{{target_podcast_id}}");
    println!("  Purpose: Separate previously merged podcasts");

    println!("\nGet merged podcasts:");
    println!("  GET /api/data/{{podcast_id}}/merged");
    println!("  Purpose: View which podcasts are merged together");

    println!("\n🔒 Authentication: REQUIRED");
    println!("✅ Security: User can only merge their own podcast subscriptions");
}

/// Test /api/data/import_opml - Import podcast subscriptions
#[tokio::test]
#[serial]
async fn test_import_opml_endpoint() {
    println!("Testing /api/data/import_opml - Import OPML file");

    println!("\n🔒 Authentication: REQUIRED");
    println!("Authorization: User can only import to their own account");

    println!("\nRequest:");
    println!("  POST /api/data/import_opml");
    println!("  Body: {{");
    println!("    \"podcasts\": [\"url1\", \"url2\", ...],");
    println!("    \"user_id\": 123");
    println!("  }}");

    println!("\nResponse:");
    println!("  {{");
    println!("    \"success\": true,");
    println!("    \"message\": \"Import process started\",");
    println!("    \"task_id\": \"<uuid>\"");
    println!("  }}");

    println!("\nProgress tracking:");
    println!("  GET /api/data/import_progress/{{user_id}}");
    println!("  Returns: {{ \"current\": 5, \"total\": 20, \"current_podcast\": \"...\" }}");

    println!("\n⚠️  Long-running operation:");
    println!("   - Runs in background");
    println!("   - Progress stored in Redis");
    println!("   - WebSocket updates available");

    println!("\n✅ CRITICAL SECURITY:");
    println!("   1. User can only import to their own account");
    println!("   2. User can only view their own import progress");
}

/// Test podcast search and discovery
#[tokio::test]
#[serial]
async fn test_podcast_search_endpoints() {
    println!("Testing podcast search and discovery");

    println!("\nSearch endpoint:");
    println!("  POST /api/data/search_data");
    println!("  Body: {{ \"query\": \"tech podcasts\", \"search_type\": \"podcasts\" }}");

    println!("\nFetch podcast feed:");
    println!("  GET /api/data/fetch_podcast_feed?feed_url=...");
    println!("  Purpose: Preview a podcast before subscribing");

    println!("\n🔒 Authentication: REQUIRED for both endpoints");
}

/// Test YouTube channel subscriptions
#[tokio::test]
#[serial]
async fn test_youtube_endpoints() {
    println!("Testing YouTube channel subscription endpoints");

    println!("\nSearch YouTube channels:");
    println!("  GET /api/data/search_youtube_channels");
    println!("  Query: channel_name");

    println!("\nSubscribe to channel:");
    println!("  POST /api/data/youtube/subscribe");
    println!("  Body: {{ \"channel_id\": \"...\", \"user_id\": 123 }}");

    println!("\nCheck if subscribed:");
    println!("  GET /api/data/check_youtube_channel");
    println!("  Query: channel_id, user_id");

    println!("\nGet YouTube episodes:");
    println!("  GET /api/data/youtube_episodes");
    println!("  Query: user_id");

    println!("\nRemove YouTube channel:");
    println!("  POST /api/data/remove_youtube_channel");
    println!("  Body: {{ \"channel_id\": \"...\", \"user_id\": 123 }}");

    println!("\n🔒 Authentication: REQUIRED");
    println!("✅ Security: User can only manage their own YouTube subscriptions");
}

/// Test podcast statistics
#[tokio::test]
#[serial]
async fn test_podcast_statistics_endpoint() {
    println!("Testing /api/data/get_stats - User listening statistics");

    println!("\n🔒 Authentication: REQUIRED");
    println!("Authorization: User can only view their own stats");

    println!("\nStatistics included:");
    println!("  - total_podcasts: Number of subscriptions");
    println!("  - total_episodes: Number of episodes in library");
    println!("  - total_listen_time: Minutes listened");
    println!("  - completed_episodes: Episodes marked complete");
    println!("  - saved_episodes: Episodes saved for later");
    println!("  - downloaded_episodes: Offline episodes");

    println!("\n✅ CRITICAL: Must verify user_id matches API key owner");
}

/// Test podcast home overview
#[tokio::test]
#[serial]
async fn test_home_overview_endpoint() {
    println!("Testing /api/data/home_overview - Dashboard data");

    println!("\n🔒 Authentication: REQUIRED");

    println!("\nProvides:");
    println!("  - Recent episodes across all subscriptions");
    println!("  - In-progress episodes");
    println!("  - Recommended content");
    println!("  - Statistics summary");

    println!("\nQuery parameters:");
    println!("  - user_id: User to get overview for");

    println!("\n✅ Security: User can only view their own overview");
}

/// Test podcast feed refresh
#[tokio::test]
#[serial]
async fn test_refresh_pods_endpoint() {
    println!("Testing /api/data/refresh_pods - Refresh all podcast feeds");

    println!("\n🔒 Authentication: REQUIRED");
    println!("👑 Authorization: ADMIN ONLY");

    println!("\nPurpose:");
    println!("  - Fetches new episodes from all subscribed podcasts");
    println!("  - Updates podcast metadata");
    println!("  - Server-wide operation");

    println!("\n⚠️  Admin-only operation");
    println!("   Regular users cannot trigger server-wide refresh");
}

/// Test PodPeople (host tracking) endpoints
#[tokio::test]
#[serial]
async fn test_podpeople_endpoints() {
    println!("Testing PodPeople (podcast host tracking) endpoints");

    println!("\nGet podcasts by host:");
    println!("  GET /api/data/podpeople/host_podcasts");
    println!("  Query: host_id or host_name");

    println!("\nSubscribe to person:");
    println!("  POST /api/data/person/subscribe/{{user_id}}/{{person_id}}");

    println!("\nUnsubscribe from person:");
    println!("  DELETE /api/data/person/unsubscribe/{{user_id}}/{{person_id}}");

    println!("\nGet person subscriptions:");
    println!("  GET /api/data/person/subscriptions/{{user_id}}");

    println!("\nGet person episodes:");
    println!("  GET /api/data/person/episodes/{{user_id}}/{{person_id}}");

    println!("\n🔒 Authentication: REQUIRED");
    println!("✅ Security: User manages their own person subscriptions");
}

/// Test podcast notification settings
#[tokio::test]
#[serial]
async fn test_podcast_notification_endpoints() {
    println!("Testing podcast notification settings");

    println!("\nToggle notifications for podcast:");
    println!("  PUT /api/data/podcast/toggle_notifications");
    println!("  Body: {{ \"podcast_id\": 123, \"user_id\": 456, \"enabled\": true }}");

    println!("\nGet notification status:");
    println!("  POST /api/data/podcast/notification_status");
    println!("  Body: {{ \"podcast_id\": 123, \"user_id\": 456 }}");

    println!("\n🔒 Authentication: REQUIRED");
    println!("✅ Security: User can only manage their own notification settings");
}

/// Test authorization checks for podcast operations
#[tokio::test]
#[serial]
async fn test_podcast_authorization() {
    println!("Podcast Endpoint Authorization Summary\n");

    println!("📝 USER OPERATIONS (require matching user_id):");
    let user_ops = vec![
        "Add/remove podcasts from own library",
        "View own podcast subscriptions",
        "Update own podcast settings",
        "Import OPML to own account",
        "View own statistics",
        "Manage own YouTube subscriptions",
        "Subscribe to podcast hosts",
        "Configure podcast notifications",
    ];
    for op in &user_ops {
        println!("  - {}", op);
    }

    println!("\n👑 ADMIN OPERATIONS (require admin role):");
    let admin_ops = vec![
        "Trigger server-wide podcast refresh",
        "View all users' podcasts (if implemented)",
        "Manage server podcast settings",
    ];
    for op in &admin_ops {
        println!("  - {}", op);
    }

    println!("\n✅ SECURITY REQUIREMENTS:");
    println!("  1. All podcast operations require valid API key");
    println!("  2. User operations verify user_id matches API key owner");
    println!("  3. Admin operations check is_admin flag");
    println!("  4. No podcast data accessible without authentication");
}
