import pytest
import pytest_asyncio
import os

# Use consistent environment variables
DB_USER = os.environ.get("DB_USER", "test_user")
DB_PASSWORD = os.environ.get("DB_PASSWORD", "test_password")
DB_HOST = os.environ.get("DB_HOST", "127.0.0.1")
DB_PORT = os.environ.get("DB_PORT", "5432")
DB_NAME = os.environ.get("DB_NAME", "test_db")

# Read the API key from the file
def get_admin_api_key():
    try:
        with open("/tmp/web_api_key.txt", "r") as f:
            return f.read().strip()
    except FileNotFoundError:
        raise RuntimeError("API key file not found. Ensure database setup has completed.")

# Get the API key once at module level
ADMIN_API_KEY = get_admin_api_key()

@pytest.mark.asyncio
async def test_pinepods_check(async_client):
    """Test the basic health check endpoint"""
    response = await async_client.get("/api/pinepods_check")
    assert response.status_code == 200
    assert response.json() == {"status_code": 200, "pinepods_instance": True}

@pytest.mark.asyncio
async def test_verify_api_key(async_client):
    """Test API key verification with admin web key"""
    response = await async_client.get(
        "/api/data/verify_key",
        headers={"Api-Key": ADMIN_API_KEY}
    )
    assert response.status_code == 200
    assert response.json() == {"status": "success"}

@pytest.mark.asyncio
async def test_get_podcast_details_dynamic(async_client):
    """Test fetching dynamic podcast details from the feed"""
    params = {
        "user_id": 1,  # Admin user ID is typically 1
        "podcast_title": "PinePods News",
        "podcast_url": "https://news.pinepods.online/feed.xml",
        "added": False,
        "display_only": False
    }
    response = await async_client.get(
        "/api/data/get_podcast_details_dynamic",
        params=params,
        headers={"Api-Key": ADMIN_API_KEY}
    )
    assert response.status_code == 200
    data = response.json()
    assert data["podcast_title"] == "Pinepods News Feed"
    assert data["podcast_url"] == "https://news.pinepods.online/feed.xml"


@pytest.mark.asyncio
async def test_add_podcast(async_client):
    """Test adding a podcast to the database"""
    # Mock the database functions
    import database_functions.functions

    # Store original function
    original_add_podcast = database_functions.functions.add_podcast

    # Mock the add_podcast function to return expected values
    def mock_add_podcast(*args, **kwargs):
        return (1, 1)  # Return a tuple of (podcast_id, first_episode_id)

    # Patch the function
    database_functions.functions.add_podcast = mock_add_podcast

    try:
        # First get the podcast details
        params = {
            "user_id": 1,
            "podcast_title": "PinePods News",
            "podcast_url": "https://news.pinepods.online/feed.xml",
            "added": False,
            "display_only": False
        }
        details_response = await async_client.get(
            "/api/data/get_podcast_details_dynamic",
            params=params,
            headers={"Api-Key": ADMIN_API_KEY}
        )
        podcast_details = details_response.json()

        # Then add the podcast
        add_request = {
            "podcast_values": {
                "pod_title": podcast_details["podcast_title"],
                "pod_artwork": podcast_details["podcast_artwork"],
                "pod_author": podcast_details["podcast_author"],
                "categories": podcast_details["podcast_categories"],
                "pod_description": podcast_details["podcast_description"],
                "pod_episode_count": podcast_details["podcast_episode_count"],
                "pod_feed_url": podcast_details["podcast_url"],
                "pod_website": podcast_details["podcast_link"],
                "pod_explicit": podcast_details["podcast_explicit"],
                "user_id": 1
            },
            "podcast_index_id": 0
        }

        response = await async_client.post(
            "/api/data/add_podcast",
            json=add_request,
            headers={"Api-Key": ADMIN_API_KEY}
        )

        assert response.status_code == 200
        data = response.json()
        assert data["success"] is True
        assert "podcast_id" in data
        assert "first_episode_id" in data

    finally:
        # Restore original function
        database_functions.functions.add_podcast = original_add_podcast
