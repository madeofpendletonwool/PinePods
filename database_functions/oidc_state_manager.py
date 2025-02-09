from typing import Optional
from database_functions.valkey_client import valkey_client

class OIDCStateManager:
    def store_state(self, state: str, client_id: str) -> bool:
        """Store OIDC state and client_id with 10 minute expiration"""
        try:
            key = f"oidc_state:{state}"
            success = valkey_client.set(key, client_id)
            if success:
                valkey_client.expire(key, 600)  # 10 minutes
            return success
        except Exception as e:
            print(f"Error storing OIDC state: {e}")
            return False

    def get_client_id(self, state: str) -> Optional[str]:
        """Get client_id for state and delete it after retrieval"""
        try:
            key = f"oidc_state:{state}"
            client_id = valkey_client.get(key)
            if client_id:
                valkey_client.delete(key)
            return client_id
        except Exception as e:
            print(f"Error getting OIDC state: {e}")
            return None

oidc_state_manager = OIDCStateManager()
