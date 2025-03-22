# websocket_broadcaster.py - Simple HTTP-based WebSocket broadcaster
import requests
import logging
import json

class WebSocketBroadcaster:
    def __init__(self):
        # Hard-coded to use the internal container port
        self.server_url = "http://localhost:8032"

    def broadcast_task_update(self, user_id, task_data, api_key):
        """Send task update via HTTP to the broadcast endpoint"""
        try:
            # Prepare the message
            message = {
                "event": "update",
                "task": task_data
            }

            # Send to the broadcast endpoint
            response = requests.post(
                f"{self.server_url}/api/tasks/broadcast",
                json={"user_id": user_id, "message": message},
                headers={"Api-Key": api_key},
                timeout=2  # Short timeout to avoid blocking
            )

            # Check result
            if response.status_code == 200:
                print(f"Successfully sent update for task {task_data.get('task_id')}")
                return True
            else:
                print(f"Failed to send update: {response.status_code} - {response.text}")
                return False
        except Exception as e:
            print(f"Error sending broadcast: {str(e)}")
            return False

# Create a singleton instance
broadcaster = WebSocketBroadcaster()
