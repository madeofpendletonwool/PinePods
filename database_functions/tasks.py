# tasks.py - Define Celery tasks with Valkey as broker
from celery import Celery
import time
import os
import asyncio
import datetime
import requests
from threading import Thread
import json
import sys
import logging
from typing import Dict, Any, Optional, List

# Make sure pinepods is in the Python path
sys.path.append('/pinepods')

database_type = str(os.getenv('DB_TYPE', 'mariadb'))

class Web_Key:
    def __init__(self):
        self.web_key = None

    def get_web_key(self, cnx):
        # Import only when needed to avoid circular imports
        from database_functions.functions import get_web_key as get_key
        self.web_key = get_key(cnx, database_type)
        return self.web_key

base_webkey = Web_Key()

# Set up logging
logger = logging.getLogger("celery_tasks")

# Import the WebSocket manager directly from clientapi
try:
    from clients.clientapi import manager as websocket_manager
    print("Successfully imported WebSocket manager from clientapi")
except ImportError as e:
    logger.error(f"Failed to import WebSocket manager: {e}")
    websocket_manager = None

# Create a dedicated event loop thread for async operations
_event_loop = None
_event_loop_thread = None

def start_background_loop():
    global _event_loop, _event_loop_thread

    # Only start if not already running
    if _event_loop is None:
        # Function to run event loop in background thread
        def run_event_loop():
            loop = asyncio.new_event_loop()
            asyncio.set_event_loop(loop)
            global _event_loop
            _event_loop = loop
            loop.run_forever()

        # Start the background thread
        _event_loop_thread = Thread(target=run_event_loop, daemon=True)
        _event_loop_thread.start()

        # Wait a moment for the loop to start
        time.sleep(0.1)
        print("Started background event loop for WebSocket broadcasts")

# Start the event loop when this module is imported
start_background_loop()

# Use the existing Valkey connection for Celery
valkey_host = os.environ.get("VALKEY_HOST", "localhost")
valkey_port = os.environ.get("VALKEY_PORT", "6379")
broker_url = f"redis://{valkey_host}:{valkey_port}/0"
backend_url = f"redis://{valkey_host}:{valkey_port}/0"

# Initialize Celery with Valkey as broker and result backend
celery_app = Celery('pinepods',
                   broker=broker_url,
                   backend=backend_url)

# Configure Celery for best performance with downloads
celery_app.conf.update(
    worker_concurrency=3,  # Limit to 3 concurrent downloads per worker
    task_acks_late=True,   # Only acknowledge tasks after they're done
    task_time_limit=1800,  # 30 minutes time limit
    task_soft_time_limit=1500,  # 25 minutes soft time limit
    worker_prefetch_multiplier=1,  # Don't prefetch more tasks than workers
)

# Task status tracking in Valkey for all types of tasks
class TaskManager:
    def __init__(self):
        from database_functions.valkey_client import valkey_client
        self.valkey_client = valkey_client

    def register_task(self, task_id: str, task_type: str, user_id: int, item_id: Optional[int] = None,
                      details: Optional[Dict[str, Any]] = None):
        """Register any Celery task for tracking"""
        task_data = {
            "task_id": task_id,
            "user_id": user_id,
            "type": task_type,
            "item_id": item_id,
            "progress": 0.0,
            "status": "PENDING",
            "details": details or {},
            "started_at": datetime.datetime.now().isoformat()
        }

        self.valkey_client.set(f"task:{task_id}", json.dumps(task_data))
        # Set TTL for 24 hours
        self.valkey_client.expire(f"task:{task_id}", 86400)

        # Add to user's active tasks list
        self._add_to_user_tasks(user_id, task_id)

        # Try to broadcast the update if the WebSocket module is available
        try:
            self._broadcast_update(task_id)
        except Exception as e:
            logger.error(f"Error broadcasting task update: {e}")

    def update_task(self, task_id: str, progress: float = None, status: str = None,
                  details: Dict[str, Any] = None):
        """Update any task's status and progress"""
        task_json = self.valkey_client.get(f"task:{task_id}")
        if task_json:
            task = json.loads(task_json)
            if progress is not None:
                task["progress"] = progress
            if status:
                task["status"] = status
            if details:
                if "details" not in task:
                    task["details"] = {}
                task["details"].update(details)

            self.valkey_client.set(f"task:{task_id}", json.dumps(task))

            # Try to broadcast the update
            try:
                self._broadcast_update(task_id)
            except Exception as e:
                logger.error(f"Error broadcasting task update: {e}")

    def complete_task(self, task_id: str, success: bool = True, result: Any = None):
        """Mark any task as complete or failed"""
        task_json = self.valkey_client.get(f"task:{task_id}")
        if task_json:
            task = json.loads(task_json)
            task["progress"] = 100.0 if success else 0.0
            task["status"] = "SUCCESS" if success else "FAILED"
            task["completed_at"] = datetime.datetime.now().isoformat()
            if result is not None:
                task["result"] = result

            self.valkey_client.set(f"task:{task_id}", json.dumps(task))

            # Try to broadcast the final update
            try:
                self._broadcast_update(task_id)
            except Exception as e:
                logger.error(f"Error broadcasting task update: {e}")

            # Keep completed tasks for 1 hour before expiring
            self.valkey_client.expire(f"task:{task_id}", 3600)

            # Remove from user's active tasks list after completion
            if success:
                self._remove_from_user_tasks(task.get("user_id"), task_id)

    def get_task(self, task_id: str) -> Dict[str, Any]:
        """Get any task's details"""
        task_json = self.valkey_client.get(f"task:{task_id}")
        if task_json:
            return json.loads(task_json)
        return {}

    def get_user_tasks(self, user_id: int) -> List[Dict[str, Any]]:
        """Get all active tasks for a user (all types)"""
        tasks_list_json = self.valkey_client.get(f"user_tasks:{user_id}")
        result = []

        if tasks_list_json:
            task_ids = json.loads(tasks_list_json)
            for task_id in task_ids:
                task_info = self.get_task(task_id)
                if task_info:
                    result.append(task_info)

        return result

    def _add_to_user_tasks(self, user_id: int, task_id: str):
        """Add a task to the user's active tasks list"""
        tasks_list_json = self.valkey_client.get(f"user_tasks:{user_id}")
        if tasks_list_json:
            tasks_list = json.loads(tasks_list_json)
            if task_id not in tasks_list:
                tasks_list.append(task_id)
        else:
            tasks_list = [task_id]

        self.valkey_client.set(f"user_tasks:{user_id}", json.dumps(tasks_list))
        # Set TTL for 7 days
        self.valkey_client.expire(f"user_tasks:{user_id}", 604800)

    def _remove_from_user_tasks(self, user_id: int, task_id: str):
        """Remove a task from the user's active tasks list"""
        tasks_list_json = self.valkey_client.get(f"user_tasks:{user_id}")
        if tasks_list_json:
            tasks_list = json.loads(tasks_list_json)
            if task_id in tasks_list:
                tasks_list.remove(task_id)
                self.valkey_client.set(f"user_tasks:{user_id}", json.dumps(tasks_list))

# Modified _broadcast_update method to avoid circular imports
    def _broadcast_update(self, task_id: str):
        """Broadcast task update via HTTP endpoint"""
        # Get task info
        task_info = self.get_task(task_id)
        if not task_info or "user_id" not in task_info:
            return

        user_id = task_info["user_id"]
        cnx = None

        try:
            cnx = get_direct_db_connection()

            # Import broadcaster - delay import to avoid circular dependency
            sys.path.insert(0, '/pinepods/database_functions')
            try:
                from websocket_broadcaster import broadcaster
            except ImportError:
                try:
                    from database_functions.websocket_broadcaster import broadcaster
                except ImportError as e:
                    print(f"Cannot import broadcaster from any location: {e}")
                    return

            # Get web key
            web_key = None
            try:
                # Get web key using class method to avoid direct import
                if not base_webkey.web_key:
                    base_webkey.get_web_key(cnx)
                web_key = base_webkey.web_key
            except Exception as e:
                print(f"Error getting web key: {str(e)}")
                # Fallback to a direct approach if needed
                try:
                    from database_functions.functions import get_web_key
                    web_key = get_web_key(cnx, database_type)
                except Exception as e2:
                    print(f"Fallback web key retrieval failed: {str(e2)}")
                    return

            # Progress and status details for better debugging
            progress = task_info.get("progress", 0)
            status = task_info.get("status", "unknown")
            print(f"Broadcasting task update for user {user_id}, task {task_id}, progress: {progress}, status: {status}")

            # Broadcast the update
            result = broadcaster.broadcast_task_update(user_id, task_info, web_key)
            if result:
                print(f"Successfully broadcast task update for task {task_id}, progress: {progress}%")
            else:
                print(f"Failed to broadcast task update for task {task_id}, progress: {progress}%")

        except Exception as e:
            print(f"Error in task broadcast setup: {str(e)}")
        finally:
            if cnx:
                # Close direct connection
                close_direct_db_connection(cnx)

# Initialize a general task manager
task_manager = TaskManager()

# For backwards compatibility, keep the download_manager name too
download_manager = task_manager

# Function to get all active tasks including both downloads and other task types
def get_all_active_tasks(user_id: int) -> List[Dict[str, Any]]:
    """Get all active tasks for a user (all types)"""
    return task_manager.get_user_tasks(user_id)

# ----------------------
# IMPROVED CONNECTION HANDLING
# ----------------------

def get_direct_db_connection():
    """
    Create a direct database connection instead of using the pool
    This is more reliable for Celery workers to avoid pool exhaustion
    """
    db_host = os.environ.get("DB_HOST", "127.0.0.1")
    db_port = os.environ.get("DB_PORT", "3306")
    db_user = os.environ.get("DB_USER", "root")
    db_password = os.environ.get("DB_PASSWORD", "password")
    db_name = os.environ.get("DB_NAME", "pypods_database")

    print(f"Creating direct database connection for task")

    if database_type == "postgresql":
        import psycopg
        conninfo = f"host={db_host} port={db_port} user={db_user} password={db_password} dbname={db_name}"
        return psycopg.connect(conninfo)
    else:  # Default to MariaDB/MySQL
        import mysql.connector
        return mysql.connector.connect(
            host=db_host,
            port=db_port,
            user=db_user,
            password=db_password,
            database=db_name,
            collation="utf8mb4_general_ci"
        )

def close_direct_db_connection(cnx):
    """Close a direct database connection"""
    if cnx:
        try:
            cnx.close()
            print("Direct database connection closed")
        except Exception as e:
            print(f"Error closing direct connection: {str(e)}")

# Minimal changes to download_podcast_task that should work right away
@celery_app.task(bind=True, max_retries=3)
def download_podcast_task(self, episode_id: int, user_id: int, database_type: str):
    """
    Celery task to download a podcast episode.
    Uses retries with exponential backoff for handling transient failures.
    """
    task_id = self.request.id
    print(f"DOWNLOAD TASK STARTED: ID={task_id}, Episode={episode_id}, User={user_id}")
    cnx = None

    try:
        # Get a direct connection to fetch the title first
        cnx = get_direct_db_connection()
        cursor = cnx.cursor()

        # Get the episode title and podcast name
        if database_type == "postgresql":
            # First try to get both the episode title and podcast name
            query = '''
                SELECT e."episodetitle", p."podcastname"
                FROM "Episodes" e
                JOIN "Podcasts" p ON e."podcastid" = p."podcastid"
                WHERE e."episodeid" = %s
            '''
        else:
            query = '''
                SELECT e.EpisodeTitle, p.PodcastName
                FROM Episodes e
                JOIN Podcasts p ON e.PodcastID = p.PodcastID
                WHERE e.EpisodeID = %s
            '''

        cursor.execute(query, (episode_id,))
        result = cursor.fetchone()
        cursor.close()

        # Extract episode title and podcast name
        episode_title = None
        podcast_name = None
        if result:
            if isinstance(result, dict):
                # Dictionary result
                if "episodetitle" in result:  # PostgreSQL lowercase
                    episode_title = result["episodetitle"]
                    podcast_name = result.get("podcastname")
                else:  # MariaDB uppercase
                    episode_title = result["EpisodeTitle"]
                    podcast_name = result.get("PodcastName")
            else:
                # Tuple result
                episode_title = result[0] if len(result) > 0 else None
                podcast_name = result[1] if len(result) > 1 else None

        # Format a nice display title
        display_title = "Unknown Episode"
        if episode_title and episode_title != "None" and episode_title.strip():
            display_title = episode_title
        elif podcast_name:
            display_title = f"{podcast_name} - Episode"
        else:
            display_title = f"Episode #{episode_id}"

        print(f"Using display title for episode {episode_id}: {display_title}")

        # Register task with more details
        task_manager.register_task(
            task_id=task_id,
            task_type="podcast_download",
            user_id=user_id,
            item_id=episode_id,
            details={
                "episode_id": episode_id,
                "episode_title": display_title,
                "status_text": f"Preparing to download {display_title}"
            }
        )

        # Define a progress callback with the display title
        def progress_callback(progress, status=None):
            status_message = ""
            if status == "DOWNLOADING":
                status_message = f"Downloading {display_title}"
            elif status == "PROCESSING":
                status_message = f"Processing {display_title}"
            elif status == "FINALIZING":
                status_message = f"Finalizing {display_title}"

            task_manager.update_task(task_id, progress, status, {
                "episode_id": episode_id,
                "episode_title": display_title,
                "status_text": status_message
            })

        # Close the connection used for title lookup
        close_direct_db_connection(cnx)

        # Get a fresh connection for the download
        cnx = get_direct_db_connection()

        # Import the download function
        from database_functions.functions import download_podcast

        print(f"Starting download for episode: {episode_id} ({display_title}), user: {user_id}, task: {task_id}")

        # Execute the download with progress reporting
        success = download_podcast(
            cnx,
            database_type,
            episode_id,
            user_id,
            task_id,
            progress_callback=progress_callback
        )

        # Mark task as complete with a nice message
        completion_message = f"{'Successfully downloaded' if success else 'Failed to download'} {display_title}"
        task_manager.complete_task(
            task_id,
            success,
            {
                "episode_id": episode_id,
                "episode_title": display_title,
                "status_text": completion_message
            }
        )

        return success
    except Exception as exc:
        print(f"Error downloading podcast {episode_id}: {str(exc)}")
        # Mark task as failed
        task_manager.complete_task(
            task_id,
            False,
            {
                "episode_id": episode_id,
                "episode_title": f"Episode #{episode_id}",
                "status_text": f"Download failed: {str(exc)}"
            }
        )
        # Retry with exponential backoff (5s, 25s, 125s)
        countdown = 5 * (2 ** self.request.retries)
        self.retry(exc=exc, countdown=countdown)
    finally:
        # Always close the connection
        if cnx:
            close_direct_db_connection(cnx)

@celery_app.task(bind=True, max_retries=3)
def download_youtube_video_task(self, video_id: int, user_id: int, database_type: str):
    """
    Celery task to download a YouTube video.
    Uses retries with exponential backoff for handling transient failures.
    """
    task_id = self.request.id
    print(f"YOUTUBE DOWNLOAD TASK STARTED: ID={task_id}, Video={video_id}, User={user_id}")
    cnx = None

    try:
        # Get a direct connection to fetch the title first
        cnx = get_direct_db_connection()
        cursor = cnx.cursor()

        # Get the video title and channel name
        if database_type == "postgresql":
            # First try to get both the video title and channel name
            query = '''
                SELECT v."videotitle", p."podcastname"
                FROM "YouTubeVideos" v
                JOIN "Podcasts" p ON v."podcastid" = p."podcastid"
                WHERE v."videoid" = %s
            '''
        else:
            query = '''
                SELECT v.VideoTitle, p.PodcastName
                FROM YouTubeVideos v
                JOIN Podcasts p ON v.PodcastID = p.PodcastID
                WHERE v.VideoID = %s
            '''

        cursor.execute(query, (video_id,))
        result = cursor.fetchone()
        cursor.close()

        # Extract video title and channel name
        video_title = None
        channel_name = None
        if result:
            if isinstance(result, dict):
                # Dictionary result
                if "videotitle" in result:  # PostgreSQL lowercase
                    video_title = result["videotitle"]
                    channel_name = result.get("podcastname")
                else:  # MariaDB uppercase
                    video_title = result["VideoTitle"]
                    channel_name = result.get("PodcastName")
            else:
                # Tuple result
                video_title = result[0] if len(result) > 0 else None
                channel_name = result[1] if len(result) > 1 else None

        # Format a nice display title
        display_title = "Unknown Video"
        if video_title and video_title != "None" and video_title.strip():
            display_title = video_title
        elif channel_name:
            display_title = f"{channel_name} - Video"
        else:
            display_title = f"YouTube Video #{video_id}"

        print(f"Using display title for video {video_id}: {display_title}")

        # Register task with more details
        task_manager.register_task(
            task_id=task_id,
            task_type="youtube_download",
            user_id=user_id,
            item_id=video_id,
            details={
                "item_id": video_id,
                "item_title": display_title,
                "status_text": f"Preparing to download {display_title}"
            }
        )

        # Close the connection used for title lookup
        close_direct_db_connection(cnx)

        # Get a fresh connection for the download
        cnx = get_direct_db_connection()

        # Import the download function
        from database_functions.functions import download_youtube_video

        print(f"Starting download for YouTube video: {video_id} ({display_title}), user: {user_id}, task: {task_id}")

        # Define a progress callback with the display title
        def progress_callback(progress, status=None):
            status_message = ""
            if status == "DOWNLOADING":
                status_message = f"Downloading {display_title}"
            elif status == "PROCESSING":
                status_message = f"Processing {display_title}"
            elif status == "FINALIZING":
                status_message = f"Finalizing {display_title}"

            task_manager.update_task(task_id, progress, status, {
                "item_id": video_id,
                "item_title": display_title,
                "status_text": status_message
            })

        # Check if the download_youtube_video function accepts progress_callback parameter
        import inspect
        try:
            signature = inspect.signature(download_youtube_video)
            has_progress_callback = 'progress_callback' in signature.parameters
        except (TypeError, ValueError):
            has_progress_callback = False

        # Execute the download with progress callback if supported, otherwise without it
        if has_progress_callback:
            success = download_youtube_video(
                cnx,
                database_type,
                video_id,
                user_id,
                task_id,
                progress_callback=progress_callback
            )
        else:
            # Call without the progress_callback parameter
            success = download_youtube_video(
                cnx,
                database_type,
                video_id,
                user_id,
                task_id
            )

            # Since we can't use progress callbacks directly, manually update progress after completion
            task_manager.update_task(task_id, 100 if success else 0,
                                    "SUCCESS" if success else "FAILED",
                                    {
                                        "item_id": video_id,
                                        "item_title": display_title,
                                        "status_text": f"{'Download complete' if success else 'Download failed'}"
                                    })

        # Mark task as complete with a nice message
        completion_message = f"{'Successfully downloaded' if success else 'Failed to download'} {display_title}"
        task_manager.complete_task(
            task_id,
            success,
            {
                "item_id": video_id,
                "item_title": display_title,
                "status_text": completion_message
            }
        )

        return success
    except Exception as exc:
        print(f"Error downloading YouTube video {video_id}: {str(exc)}")
        # Mark task as failed but include video title in the details
        task_manager.complete_task(
            task_id,
            False,
            {
                "item_id": video_id,
                "item_title": f"YouTube Video #{video_id}",
                "status_text": f"Download failed: {str(exc)}"
            }
        )
        # Retry with exponential backoff (5s, 25s, 125s)
        countdown = 5 * (2 ** self.request.retries)
        self.retry(exc=exc, countdown=countdown)
    finally:
        # Always close the connection
        if cnx:
            close_direct_db_connection(cnx)

@celery_app.task
def queue_podcast_downloads(podcast_id: int, user_id: int, database_type: str, is_youtube: bool = False):
    """
    Task to queue individual download tasks for all episodes/videos in a podcast.
    This adds downloads to the queue in small batches to prevent overwhelming the system.
    """
    cnx = None

    try:
        # Get a direct connection
        cnx = get_direct_db_connection()

        from database_functions.functions import (
            get_episode_ids_for_podcast,
            get_video_ids_for_podcast,
            check_downloaded
        )

        if is_youtube:
            item_ids = get_video_ids_for_podcast(cnx, database_type, podcast_id)
            print(f"Queueing {len(item_ids)} YouTube videos for download")

            # Process YouTube items in batches
            batch_size = 5
            for i in range(0, len(item_ids), batch_size):
                batch = item_ids[i:i+batch_size]
                for item_id in batch:
                    if not check_downloaded(cnx, database_type, user_id, item_id, is_youtube):
                        download_youtube_video_task.delay(item_id, user_id, database_type)

                # Add a small delay between batches
                if i + batch_size < len(item_ids):
                    time.sleep(2)
        else:
            # Get episode IDs (should return dicts with id and title)
            episodes = get_episode_ids_for_podcast(cnx, database_type, podcast_id)
            print(f"Queueing {len(episodes)} podcast episodes for download")

            # Process episodes in batches
            batch_size = 5
            for i in range(0, len(episodes), batch_size):
                batch = episodes[i:i+batch_size]

                for episode in batch:
                    # Handle both possible formats (dict or simple ID)
                    if isinstance(episode, dict) and "id" in episode:
                        episode_id = episode["id"]
                    else:
                        # Fall back to treating it as just an ID
                        episode_id = episode

                    if not check_downloaded(cnx, database_type, user_id, episode_id, is_youtube):
                        # Pass just the ID, the task will look up the title
                        download_podcast_task.delay(episode_id, user_id, database_type)

                # Add a small delay between batches
                if i + batch_size < len(episodes):
                    time.sleep(2)

        return f"Queued {len(episodes if not is_youtube else item_ids)} items for download"
    finally:
        if cnx:
            close_direct_db_connection(cnx)

# Helper task to clean up old download records
@celery_app.task
def cleanup_old_downloads():
    """
    Periodic task to clean up old download records from Valkey
    """
    from database_functions.valkey_client import valkey_client

    # This would need to be implemented with a scan operation
    # For simplicity, we rely on Redis/Valkey TTL mechanisms
    print("Running download cleanup task")

# Example task for refreshing podcast feeds
@celery_app.task(bind=True, max_retries=2)
def refresh_feed_task(self, user_id: int, database_type: str):
    """
    Celery task to refresh podcast feeds for a user.
    """
    task_id = self.request.id
    cnx = None

    try:
        # Register task
        task_manager.register_task(
            task_id=task_id,
            task_type="feed_refresh",
            user_id=user_id,
            details={"description": "Refreshing podcast feeds"}
        )

        # Get a direct database connection
        cnx = get_direct_db_connection()

        # Get list of podcasts to refresh
        # Then update progress as each one completes
        try:
            # Here you would have your actual feed refresh implementation
            # with periodic progress updates
            task_manager.update_task(task_id, 10, "PROGRESS", {"status_text": "Fetching podcast list"})

            # Simulate feed refresh process with progress updates
            # Replace with your actual implementation
            total_podcasts = 10  # Example count
            for i in range(total_podcasts):
                # Update progress for each podcast
                progress = (i + 1) / total_podcasts * 100
                task_manager.update_task(
                    task_id,
                    progress,
                    "PROGRESS",
                    {"status_text": f"Refreshing podcast {i+1}/{total_podcasts}"}
                )

                # Simulated work - replace with actual refresh logic
                time.sleep(0.5)

            # Complete the task
            task_manager.complete_task(task_id, True, {"refreshed_count": total_podcasts})
            return True

        except Exception as e:
            raise e

    except Exception as exc:
        print(f"Error refreshing feeds for user {user_id}: {str(exc)}")
        task_manager.complete_task(task_id, False, {"error": str(exc)})
        self.retry(exc=exc, countdown=30)
    finally:
        # Always close the connection
        if cnx:
            close_direct_db_connection(cnx)

# Simple debug task
@celery_app.task
def debug_task(x, y):
    """Simple debug task that prints output"""
    result = x + y
    print(f"CELERY DEBUG TASK EXECUTED: {x} + {y} = {result}")
    return result
