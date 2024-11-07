import pytest
import pytest_asyncio

@pytest.mark.asyncio
async def test_health_check(async_client):
    """Test the health check endpoint"""
    response = await async_client.get("/api/pinepods_check")
    assert response.status_code == 200
    # Check for the expected response data
    assert response.json() == {"status_code": 200, "pinepods_instance": True}
