from fastapi import APIRouter, Depends, HTTPException, status, Request, Response
from pydantic import BaseModel
from typing import List, Dict, Optional, Any
import database_functions.functions
from database_connections import get_database_connection, database_type

# Create models for the API
class DeviceCreate(BaseModel):
    user_id: int
    device_name: str
    device_type: Optional[str] = "desktop"
    device_caption: Optional[str] = None

class Device(BaseModel):
    id: int
    name: str
    type: str
    caption: Optional[str] = None
    last_sync: Optional[str] = None
    is_active: bool = True

class SyncRequest(BaseModel):
    user_id: int
    device_id: Optional[int] = None

class ApiResponse(BaseModel):
    success: bool
    message: str
    data: Optional[Any] = None

# Create the router
gpodder_router = APIRouter(prefix="/api/gpodder", tags=["gpodder"])

# Authentication function (assumed to be defined elsewhere)
async def get_api_key_from_header(request: Request):
    api_key = request.headers.get("X-API-Key")
    if not api_key:
        raise HTTPException(status_code=403, detail="API key is required")
    return api_key

async def has_elevated_access(api_key: str, cnx):
    user_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)
    is_admin = database_functions.functions.is_admin(cnx, database_type, user_id)
    return is_admin

@gpodder_router.get("/devices/{user_id}", response_model=List[Device])
async def get_user_devices(
    user_id: int,
    cnx=Depends(get_database_connection),
    api_key: str = Depends(get_api_key_from_header)
):
    """Get all GPodder devices for a user"""
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(
            status_code=403,
            detail="Your API key is either invalid or does not have correct permission"
        )

    # Check if the user has permission
    elevated_access = await has_elevated_access(api_key, cnx)
    if not elevated_access:
        user_id_from_api_key = database_functions.functions.id_from_api_key(cnx, database_type, api_key)
        if user_id != user_id_from_api_key:
            raise HTTPException(
                status_code=status.HTTP_403_FORBIDDEN,
                detail="You are not authorized to access these devices"
            )

    # Get devices
    devices = database_functions.functions.get_user_devices(cnx, database_type, user_id)
    if not devices:
        # Create a default device if none exists
        default_device_id = database_functions.functions.get_or_create_default_device(cnx, database_type, user_id)
        if default_device_id:
            devices = database_functions.functions.get_user_devices(cnx, database_type, user_id)
        else:
            return []

    return devices

@gpodder_router.post("/devices", response_model=Device)
async def create_device(
    device: DeviceCreate,
    cnx=Depends(get_database_connection),
    api_key: str = Depends(get_api_key_from_header)
):
    """Create a new GPodder device for a user"""
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(
            status_code=403,
            detail="Your API key is either invalid or does not have correct permission"
        )

    # Check if the user has permission
    elevated_access = await has_elevated_access(api_key, cnx)
    if not elevated_access:
        user_id_from_api_key = database_functions.functions.id_from_api_key(cnx, database_type, api_key)
        if device.user_id != user_id_from_api_key:
            raise HTTPException(
                status_code=status.HTTP_403_FORBIDDEN,
                detail="You are not authorized to create devices for this user"
            )

    # Create device
    device_id = database_functions.functions.create_or_update_device(
        cnx,
        database_type,
        device.user_id,
        device.device_name,
        device.device_type,
        device.device_caption
    )

    if not device_id:
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail="Failed to create device"
        )

    # Get the created device
    devices = database_functions.functions.get_user_devices(cnx, database_type, device.user_id)
    for d in devices:
        if d["id"] == device_id:
            return d

    # This should not happen
    raise HTTPException(
        status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
        detail="Device created but not found"
    )

@gpodder_router.post("/sync/force", response_model=ApiResponse)
async def force_full_sync(
    sync_request: SyncRequest,
    cnx=Depends(get_database_connection),
    api_key: str = Depends(get_api_key_from_header)
):
    """Force a full sync of all local podcasts to the GPodder server"""
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(
            status_code=403,
            detail="Your API key is either invalid or does not have correct permission"
        )

    # Check if the user has permission
    elevated_access = await has_elevated_access(api_key, cnx)
    if not elevated_access:
        user_id_from_api_key = database_functions.functions.id_from_api_key(cnx, database_type, api_key)
        if sync_request.user_id != user_id_from_api_key:
            raise HTTPException(
                status_code=status.HTTP_403_FORBIDDEN,
                detail="You are not authorized to force sync for this user"
            )

    # Get GPodder settings
    user_id = sync_request.user_id
    gpodder_settings = database_functions.functions.get_gpodder_settings(cnx, database_type, user_id)

    if not gpodder_settings or not gpodder_settings.get("gpodderurl"):
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail="GPodder settings not configured for this user"
        )

    # Get login name
    cursor = cnx.cursor()
    if database_type == "postgresql":
        query = 'SELECT GpodderLoginName FROM "Users" WHERE UserID = %s'
    else:
        query = "SELECT GpodderLoginName FROM Users WHERE UserID = %s"

    cursor.execute(query, (user_id,))
    result = cursor.fetchone()
    cursor.close()

    if not result:
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail="User not found"
        )

    gpodder_login = result[0] if isinstance(result, tuple) else result["gpodderloginname"]

    # Force sync
    success = database_functions.functions.force_full_sync_to_gpodder(
        database_type,
        cnx,
        user_id,
        gpodder_settings.get("gpodderurl"),
        gpodder_settings.get("gpoddertoken"),
        gpodder_login
    )

    if not success:
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail="Failed to force synchronization"
        )

    return ApiResponse(
        success=True,
        message="Successfully synchronized all podcasts to GPodder"
    )

@gpodder_router.post("/sync", response_model=ApiResponse)
async def sync_with_gpodder(
    sync_request: SyncRequest,
    cnx=Depends(get_database_connection),
    api_key: str = Depends(get_api_key_from_header)
):
    """Sync podcasts from GPodder to local database"""
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(
            status_code=403,
            detail="Your API key is either invalid or does not have correct permission"
        )

    # Check if the user has permission
    elevated_access = await has_elevated_access(api_key, cnx)
    if not elevated_access:
        user_id_from_api_key = database_functions.functions.id_from_api_key(cnx, database_type, api_key)
        if sync_request.user_id != user_id_from_api_key:
            raise HTTPException(
                status_code=status.HTTP_403_FORBIDDEN,
                detail="You are not authorized to sync for this user"
            )

    # Get GPodder settings
    user_id = sync_request.user_id
    gpodder_settings = database_functions.functions.get_gpodder_settings(cnx, database_type, user_id)

    if not gpodder_settings or not gpodder_settings.get("gpodderurl"):
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail="GPodder settings not configured for this user"
        )

    # Get login name and pod_sync_type
    cursor = cnx.cursor()
    if database_type == "postgresql":
        query = 'SELECT GpodderLoginName, Pod_Sync_Type FROM "Users" WHERE UserID = %s'
    else:
        query = "SELECT GpodderLoginName, Pod_Sync_Type FROM Users WHERE UserID = %s"

    cursor.execute(query, (user_id,))
    result = cursor.fetchone()
    cursor.close()

    if not result:
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail="User not found"
        )

    if isinstance(result, tuple):
        gpodder_login = result[0]
        pod_sync_type = result[1]
    else:
        gpodder_login = result["gpodderloginname"]
        pod_sync_type = result["pod_sync_type"]

    # Perform sync
    success = database_functions.functions.refresh_gpodder_subscription(
        database_type,
        cnx,
        user_id,
        gpodder_settings.get("gpodderurl"),
        gpodder_settings.get("gpoddertoken"),
        gpodder_login,
        pod_sync_type
    )

    if not success:
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail="Failed to synchronize with GPodder"
        )

    return ApiResponse(
        success=True,
        message="Successfully synchronized with GPodder"
    )

@gpodder_router.get("/test-connection", response_model=ApiResponse)
async def test_gpodder_connection(
    user_id: int,
    gpodder_url: str,
    gpodder_username: str,
    gpodder_password: str,
    cnx=Depends(get_database_connection),
    api_key: str = Depends(get_api_key_from_header)
):
    """Test connection to GPodder server"""
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(
            status_code=403,
            detail="Your API key is either invalid or does not have correct permission"
        )

    # Check if the user has permission
    elevated_access = await has_elevated_access(api_key, cnx)
    if not elevated_access:
        user_id_from_api_key = database_functions.functions.id_from_api_key(cnx, database_type, api_key)
        if user_id != user_id_from_api_key:
            raise HTTPException(
                status_code=status.HTTP_403_FORBIDDEN,
                detail="You are not authorized to test connection for this user"
            )

    try:
        import requests
        from requests.auth import HTTPBasicAuth

        # Try to connect to GPodder server
        auth = HTTPBasicAuth(gpodder_username, gpodder_password)

        # Try session-based first
        session = requests.Session()

        try:
            # Try to establish a session
            login_url = f"{gpodder_url}/api/2/auth/{gpodder_username}/login.json"
            login_response = session.post(login_url, auth=auth)
            login_response.raise_for_status()

            # If session works, try to get subscriptions
            test_url = f"{gpodder_url}/api/2/subscriptions/{gpodder_username}.json"
            response = session.get(test_url)
            response.raise_for_status()

            return ApiResponse(
                success=True,
                message="Successfully connected to GPodder server using session authentication",
                data={"auth_type": "session"}
            )

        except Exception as e:
            # Fall back to basic auth
            test_url = f"{gpodder_url}/api/2/subscriptions/{gpodder_username}.json"
            response = requests.get(test_url, auth=auth)
            response.raise_for_status()

            return ApiResponse(
                success=True,
                message="Successfully connected to GPodder server using basic authentication",
                data={"auth_type": "basic"}
            )

    except Exception as e:
        return ApiResponse(
            success=False,
            message=f"Failed to connect to GPodder server: {str(e)}",
            data=None
        )
