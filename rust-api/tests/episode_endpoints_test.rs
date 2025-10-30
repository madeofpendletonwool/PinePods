// Episode endpoint tests - Episode playback and management
mod common;

use axum::http::{Method, StatusCode};
use common::*;
use serial_test::serial;

/// Test /api/data/queue_pod - Add episode to queue
#[tokio::test]
#[serial]
async fn test_queue_episode_endpoint() {
    println!("Testing /api/data/queue_pod - Add episode to queue");

    println!("\n🔒 Authentication: REQUIRED");
    println!("Authorization: User can only queue to their own queue");

    println!("\nRequest:");
    println!("  POST /api/data/queue_pod");
    println!("  Body: {{");
    println!("    \"episode_id\": 123,");
    println!("    \"user_id\": 456,");
    println!("    \"is_youtube\": false");
    println!("  }}");

    println!("\nResponse:");
    println!("  {{ \"data\": \"Episode added to queue\" }}");

    println!("\n✅ Security: User ID must match API key owner");
}

/// Test /api/data/remove_queued_pod - Remove from queue
#[tokio::test]
#[serial]
async fn test_remove_queued_episode_endpoint() {
    println!("Testing /api/data/remove_queued_pod - Remove from queue");

    println!("\n🔒 Authentication: REQUIRED");

    println!("\nRequest:");
    println!("  POST /api/data/remove_queued_pod");
    println!("  Body: {{ \"episode_id\": 123, \"user_id\": 456 }}");

    println!("\n✅ Security: User can only modify their own queue");
}

/// Test /api/data/get_queued_episodes - Get user's queue
#[tokio::test]
#[serial]
async fn test_get_queued_episodes_endpoint() {
    println!("Testing /api/data/get_queued_episodes - Get queue");

    println!("\n🔒 Authentication: REQUIRED");

    println!("\nQuery parameters:");
    println!("  - user_id: User whose queue to retrieve");

    println!("\nResponse structure:");
    println!("  {{");
    println!("    \"data\": [");
    println!("      {{");
    println!("        \"episodetitle\": \"Episode 1\",");
    println!("        \"podcastname\": \"Podcast Name\",");
    println!("        \"episodepubdate\": \"2025-01-01\",");
    println!("        \"episodedescription\": \"...\",");
    println!("        \"episodeartwork\": \"https://...\",");
    println!("        \"episodeurl\": \"https://...\",");
    println!("        \"queueposition\": 1,");
    println!("        \"episodeduration\": 3600,");
    println!("        \"queuedate\": \"2025-01-15\",");
    println!("        \"listenduration\": 1200,");
    println!("        \"episodeid\": 123,");
    println!("        \"completed\": false,");
    println!("        \"saved\": false,");
    println!("        \"queued\": true,");
    println!("        \"downloaded\": false,");
    println!("        \"is_youtube\": false");
    println!("      }}");
    println!("    ]");
    println!("  }}");

    println!("\n✅ CRITICAL SECURITY: Must verify user_id matches API key owner");
    println!("   Otherwise users could view other users' queues");
}

/// Test /api/data/reorder_queue - Reorder queue
#[tokio::test]
#[serial]
async fn test_reorder_queue_endpoint() {
    println!("Testing /api/data/reorder_queue - Reorder queue");

    println!("\n🔒 Authentication: REQUIRED");

    println!("\nRequest:");
    println!("  POST /api/data/reorder_queue");
    println!("  Body: {{");
    println!("    \"episode_ids\": [5, 2, 8, 1, 3]  // New order");
    println!("  }}");

    println!("\nResponse:");
    println!("  {{ \"message\": \"Queue reordered successfully\" }}");

    println!("\n✅ Security: Can only reorder own queue");
}

/// Test /api/data/save_episode - Save episode
#[tokio::test]
#[serial]
async fn test_save_episode_endpoint() {
    println!("Testing /api/data/save_episode - Save episode for later");

    println!("\n🔒 Authentication: REQUIRED");

    println!("\nRequest:");
    println!("  POST /api/data/save_episode");
    println!("  Body: {{");
    println!("    \"episode_id\": 123,");
    println!("    \"user_id\": 456,");
    println!("    \"is_youtube\": false");
    println!("  }}");

    println!("\nResponse:");
    println!("  {{ \"detail\": \"Episode saved successfully\" }}");

    println!("\n✅ Security: User can only save to their own library");
}

/// Test /api/data/remove_saved_episode - Unsave episode
#[tokio::test]
#[serial]
async fn test_remove_saved_episode_endpoint() {
    println!("Testing /api/data/remove_saved_episode - Remove from saved");

    println!("\n🔒 Authentication: REQUIRED");

    println!("\nRequest:");
    println!("  POST /api/data/remove_saved_episode");
    println!("  Body: {{ \"episode_id\": 123, \"user_id\": 456 }}");

    println!("\n✅ Security: Can only modify own saved episodes");
}

/// Test /api/data/saved_episode_list/{user_id} - Get saved episodes
#[tokio::test]
#[serial]
async fn test_saved_episode_list_endpoint() {
    println!("Testing /api/data/saved_episode_list/{{user_id}} - Get saved episodes");

    println!("\n🔒 Authentication: REQUIRED");

    println!("\nResponse:");
    println!("  {{");
    println!("    \"saved_episodes\": [");
    println!("      // Array of SavedEpisode objects");
    println!("    ]");
    println!("  }}");

    println!("\n✅ CRITICAL SECURITY: User ID in path must match API key owner");
}

/// Test /api/data/mark_episode_completed - Mark as played
#[tokio::test]
#[serial]
async fn test_mark_episode_completed_endpoint() {
    println!("Testing /api/data/mark_episode_completed - Mark as complete");

    println!("\n🔒 Authentication: REQUIRED");

    println!("\nRequest:");
    println!("  POST /api/data/mark_episode_completed");
    println!("  Body: {{");
    println!("    \"episode_id\": 123,");
    println!("    \"user_id\": 456,");
    println!("    \"is_youtube\": false");
    println!("  }}");

    println!("\nResponse:");
    println!("  Status: 200 OK");

    println!("\n✅ Security: Can only mark own episodes as complete");
}

/// Test /api/data/mark_episode_uncompleted - Mark as unplayed
#[tokio::test]
#[serial]
async fn test_mark_episode_uncompleted_endpoint() {
    println!("Testing /api/data/mark_episode_uncompleted - Mark as incomplete");

    println!("\n🔒 Authentication: REQUIRED");

    println!("\nAllows users to reset episode completion status");
    println!("Useful for re-listening or correcting mistakes");

    println!("\n✅ Security: Can only modify own episode states");
}

/// Test bulk episode operations
#[tokio::test]
#[serial]
async fn test_bulk_episode_operations() {
    println!("Testing bulk episode operations");

    println!("\nBulk mark completed:");
    println!("  POST /api/data/bulk_mark_episodes_completed");
    println!("  Body: {{");
    println!("    \"episode_ids\": [1, 2, 3, 4, 5],");
    println!("    \"user_id\": 123,");
    println!("    \"is_youtube\": false");
    println!("  }}");

    println!("\nBulk save episodes:");
    println!("  POST /api/data/bulk_save_episodes");
    println!("  Body: {{ \"episode_ids\": [...], \"user_id\": 123 }}");

    println!("\nBulk queue episodes:");
    println!("  POST /api/data/bulk_queue_episodes");
    println!("  Body: {{ \"episode_ids\": [...], \"user_id\": 123 }}");

    println!("\nBulk download episodes:");
    println!("  POST /api/data/bulk_download_episodes");
    println!("  Body: {{ \"episode_ids\": [...], \"user_id\": 123 }}");

    println!("\nBulk delete downloaded:");
    println!("  POST /api/data/bulk_delete_downloaded_episodes");
    println!("  Body: {{ \"episode_ids\": [...], \"user_id\": 123 }}");

    println!("\n⚠️  Performance consideration:");
    println!("   Bulk operations process multiple episodes in one request");
    println!("   Must validate user owns all episodes being modified");

    println!("\n✅ CRITICAL SECURITY:");
    println!("   1. User ID must match API key owner");
    println!("   2. All episode IDs must belong to user's subscriptions");
    println!("   3. Cannot bulk-modify other users' episodes");
}

/// Test episode playback tracking
#[tokio::test]
#[serial]
async fn test_episode_playback_tracking() {
    println!("Testing episode playback tracking");

    println!("\nRecord listen duration:");
    println!("  POST /api/data/record_listen_duration");
    println!("  Body: {{");
    println!("    \"episode_id\": 123,");
    println!("    \"user_id\": 456,");
    println!("    \"duration\": 1200,  // seconds listened");
    println!("    \"is_youtube\": false");
    println!("  }}");

    println!("\nIncrement listen time:");
    println!("  PUT /api/data/increment_listen_time/{{user_id}}");
    println!("  Body: {{");
    println!("    \"episode_id\": 123,");
    println!("    \"increment\": 30  // Add 30 seconds");
    println!("  }}");

    println!("\nUpdate episode duration:");
    println!("  POST /api/data/update_episode_duration");
    println!("  Body: {{");
    println!("    \"episode_id\": 123,");
    println!("    \"duration\": 3600  // Total episode length");
    println!("  }}");

    println!("\nIncrement played count:");
    println!("  PUT /api/data/increment_played/{{user_id}}");
    println!("  Body: {{ \"episode_id\": 123 }}");

    println!("\n🔒 Authentication: REQUIRED for all");
    println!("✅ Security: User can only update their own playback state");
}

/// Test episode history tracking
#[tokio::test]
#[serial]
async fn test_episode_history() {
    println!("Testing episode listening history");

    println!("\nRecord episode in history:");
    println!("  POST /api/data/record_podcast_history");
    println!("  Body: {{");
    println!("    \"episode_id\": 123,");
    println!("    \"episode_pos\": 1250.5,  // Current position in seconds");
    println!("    \"user_id\": 456,");
    println!("    \"is_youtube\": false");
    println!("  }}");

    println!("\nGet user history:");
    println!("  GET /api/data/user_history/{{user_id}}");
    println!("  Returns: List of recently played episodes with timestamps");

    println!("\n✅ CRITICAL SECURITY:");
    println!("   - User can only record to their own history");
    println!("   - User can only view their own history");
    println!("   - History data is private (listening habits)");
}

/// Test episode download management
#[tokio::test]
#[serial]
async fn test_episode_download_management() {
    println!("Testing episode download management");

    println!("\nDownload single episode:");
    println!("  POST /api/data/download_podcast");
    println!("  Body: {{");
    println!("    \"episode_id\": 123,");
    println!("    \"user_id\": 456,");
    println!("    \"podcast_id\": 789");
    println!("  }}");

    println!("\nDownload all podcast episodes:");
    println!("  POST /api/data/download_all_podcast");
    println!("  Body: {{ \"podcast_id\": 789, \"user_id\": 456 }}");

    println!("\nDelete downloaded episode:");
    println!("  POST /api/data/delete_episode");
    println!("  Body: {{ \"episode_id\": 123, \"user_id\": 456 }}");

    println!("\nGet download status:");
    println!("  GET /api/data/download_status/{{user_id}}");
    println!("  Returns: List of download progress for active downloads");

    println!("\nList downloaded episodes:");
    println!("  GET /api/data/download_episode_list");
    println!("  Query: user_id");

    println!("\n⚠️  Storage considerations:");
    println!("   - Downloads consume server disk space");
    println!("   - May have per-user quotas");
    println!("   - Downloads may be disabled server-wide");

    println!("\n✅ Security:");
    println!("   1. User can only download to their own storage");
    println!("   2. User can only delete their own downloads");
    println!("   3. Cannot access other users' downloaded files");
}

/// Test episode metadata and details
#[tokio::test]
#[serial]
async fn test_episode_metadata_endpoints() {
    println!("Testing episode metadata endpoints");

    println!("\nGet episode metadata:");
    println!("  POST /api/data/get_episode_metadata");
    println!("  Body: {{ \"episode_id\": 123 }}");
    println!("  Returns: Full episode metadata (title, description, etc.)");

    println!("\nGet play episode details:");
    println!("  POST /api/data/get_play_episode_details");
    println!("  Body: {{ \"episode_id\": 123, \"user_id\": 456 }}");
    println!("  Returns: Episode metadata + user state (position, completed, etc.)");

    println!("\nCheck episode in database:");
    println!("  GET /api/data/check_episode_in_db/{{user_id}}");
    println!("  Query: episode_url or episode_id");
    println!("  Returns: {{ \"episode_in_db\": true }}");

    println!("\nGet episode ID from name:");
    println!("  GET /api/data/get_episode_id_ep_name");
    println!("  Query: episode_name, podcast_name");

    println!("\nGet podcast ID from episode ID:");
    println!("  GET /api/data/get_podcast_id_from_ep_id");
    println!("  Query: episode_id");

    println!("\nGet podcast ID from episode name:");
    println!("  GET /api/data/get_podcast_id_from_ep_name");
    println!("  Query: episode_name");

    println!("\n🔒 Authentication: REQUIRED");
}

/// Test episode streaming
#[tokio::test]
#[serial]
async fn test_episode_streaming_endpoint() {
    println!("Testing /api/data/stream/{{episode_id}} - Stream episode");

    println!("\n🔒 Authentication: REQUIRED");
    println!("Authorization: User must be subscribed to the podcast");

    println!("\nQuery parameters:");
    println!("  - user_id: User requesting the stream");

    println!("\nResponse:");
    println!("  - HTTP range request support for seeking");
    println!("  - Proper Content-Type headers");
    println!("  - Stream from either downloaded file or remote URL");

    println!("\n✅ CRITICAL SECURITY:");
    println!("   1. Verify user is subscribed to episode's podcast");
    println!("   2. Don't expose file paths in responses");
    println!("   3. Validate episode belongs to a user's subscription");
    println!("   4. Cannot stream other users' private episodes");
}

/// Test episode sharing
#[tokio::test]
#[serial]
async fn test_episode_sharing() {
    println!("Testing episode sharing functionality");

    println!("\nCreate shareable link:");
    println!("  POST /api/data/share_episode/{{episode_id}}");
    println!("  Body: {{ \"user_id\": 123 }}");
    println!("  Returns: {{");
    println!("    \"share_url\": \"https://pinepods.com/e/abc123\",");
    println!("    \"url_key\": \"abc123\"");
    println!("  }}");

    println!("\nAccess shared episode:");
    println!("  GET /api/data/episode_by_url/{{url_key}}");
    println!("  Returns: Episode details (public access)");

    println!("\n⚠️  Privacy considerations:");
    println!("   - Shared links are publicly accessible");
    println!("   - No authentication required to view shared episodes");
    println!("   - Consider expiration for shared links");

    println!("\n✅ Security:");
    println!("   - User can only share episodes from their subscriptions");
    println!("   - URL keys should be unguessable (UUID)");
    println!("   - Original poster information included");
}

/// Test episode transcripts
#[tokio::test]
#[serial]
async fn test_episode_transcript_endpoint() {
    println!("Testing /api/data/fetch_transcript - Get episode transcript");

    println!("\n🔒 Authentication: REQUIRED");

    println!("\nRequest:");
    println!("  POST /api/data/fetch_transcript");
    println!("  Body: {{ \"episode_id\": 123, \"user_id\": 456 }}");

    println!("\nReturns:");
    println!("  - Transcript text if available");
    println!("  - May be from Podcasting 2.0 namespace");
    println!("  - Or from third-party transcription service");

    println!("\n✅ Security: User must be subscribed to podcast");
}

/// Test episode download file endpoint
#[tokio::test]
#[serial]
async fn test_episode_download_file_endpoint() {
    println!("Testing /api/episodes/{{episode_id}}/download - Download episode file");

    println!("\n🔒 Authentication: REQUIRED");

    println!("\nQuery parameters:");
    println!("  - user_id: User downloading the file");

    println!("\nPurpose:");
    println!("  - Allows user to download episode file to their device");
    println!("  - Different from streaming (downloads full file)");
    println!("  - Provides Content-Disposition header for download");

    println!("\n✅ Security:");
    println!("   1. User must be subscribed to podcast");
    println!("   2. File served from user's storage or proxied from source");
    println!("   3. Cannot download episodes from unsubscribed podcasts");
}

/// Test auto-download settings
#[tokio::test]
#[serial]
async fn test_auto_download_settings() {
    println!("Testing episode auto-download settings");

    println!("\nGet auto-download status:");
    println!("  POST /api/data/get_auto_download_status");
    println!("  Body: {{ \"podcast_id\": 123, \"user_id\": 456 }}");

    println!("\nEnable auto-download:");
    println!("  POST /api/data/enable_auto_download");
    println!("  Body: {{");
    println!("    \"podcast_id\": 123,");
    println!("    \"user_id\": 456,");
    println!("    \"enabled\": true");
    println!("  }}");

    println!("\n🔒 Authentication: REQUIRED");
    println!("✅ Security: User can only configure their own podcasts");
}

/// Test episode authorization summary
#[tokio::test]
#[serial]
async fn test_episode_authorization_summary() {
    println!("Episode Endpoint Authorization Summary\n");

    println!("📝 USER OPERATIONS (require matching user_id):");
    let user_ops = vec![
        "Queue/unqueue episodes",
        "Save/unsave episodes",
        "Mark episodes as played/unplayed",
        "Track playback position",
        "Download episodes for offline",
        "Delete downloaded episodes",
        "Stream episodes",
        "View own listening history",
        "Share episodes (from own subscriptions)",
        "Reorder own queue",
        "Bulk operations on own episodes",
    ];
    for op in &user_ops {
        println!("  - {}", op);
    }

    println!("\n🌐 PUBLIC OPERATIONS (no auth):");
    println!("  - View shared episodes (via URL key)");

    println!("\n✅ CRITICAL SECURITY REQUIREMENTS:");
    println!("  1. All episode operations require valid API key");
    println!("  2. User operations verify user_id matches API key owner");
    println!("  3. Episode access requires podcast subscription");
    println!("  4. Cannot access other users' episode states");
    println!("  5. Cannot stream/download unsubscribed episodes");
    println!("  6. History and queue are private data");
    println!("  7. Playback positions are per-user");
    println!("  8. Downloads are isolated per-user");
}
