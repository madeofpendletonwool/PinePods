import os
import redis
from redis.exceptions import RedisError

class ValkeyClient:
    def __init__(self):
        self.host = os.environ.get("VALKEY_HOST", "localhost")
        self.port = int(os.environ.get("VALKEY_PORT", 6379))
        self.client = None

    def connect(self):
        try:
            self.client = redis.Redis(
                host=self.host,
                port=self.port,
                decode_responses=True,
                health_check_interval=10,
                socket_connect_timeout=5,
                retry_on_timeout=True,
                socket_keepalive=True
            )
            self.client.ping()  # Test the connection
            print("Successfully connected to Valkey")
        except RedisError as e:
            print(f"Failed to connect to Valkey: {e}")
            self.client = None

    def get(self, key):
        if not self.client:
            self.connect()
        try:
            return self.client.get(key)
        except RedisError as e:
            print(f"Error getting key from Valkey: {e}")
            return None

    def set(self, key, value):
        if not self.client:
            self.connect()
        try:
            return self.client.set(key, value)
        except RedisError as e:
            print(f"Error setting key in Valkey: {e}")
            return False

    def delete(self, key):
        if not self.client:
            self.connect()
        try:
            return self.client.delete(key)
        except RedisError as e:
            print(f"Error deleting key from Valkey: {e}")
            return False

valkey_client = ValkeyClient()