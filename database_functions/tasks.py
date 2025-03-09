# tasks.py - Define Celery tasks with Valkey as broker
from celery import Celery
import time
import os
import datetime
import requests
import json
import logging
from typing import Dict, Any, Optional, List

# Set up logging
logger = logging.getLogger("celery_tasks")

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

# DownloadProgressManager for tracking downloads using Valkey
class DownloadProgressManager:
    def __init__(self):
        from database_functions.valkey_client import valkey_client
        self.valkey_client = valkey_client

    def start_download(self, task_id: str, user_id: int, item_id: int, is_youtube: bool):
        """Register a new download task"""
        self.valkey_client.set(f"download:{task_id}", json.dumps({
            "task_id": task_id,
            "user_id": user_id,
            "item_id": item_id,
            "type": "video" if is_youtube else "episode",
            "progress": 0.0,
            "status": "PENDING",
            "started_at": datetime.datetime.now().isoformat()
        }))
        # Set TTL for 24 hours
        self.valkey_client.expire(f"download:{task_id}", 86400)

        # Add to user's active downloads list
        self._add_to_user_downloads(user_id, task_id)

    def update_progress(self, task_id: str, progress: float, status: str = None):
        """Update download progress"""
        download_json = self.valkey_client.get(f"download:{task_id}")
        if download_json:
            download = json.loads(download_json)
            download["progress"] = progress
            if status:
                download["status"] = status
            self.valkey_client.set(f"download:{task_id}", json.dumps(download))

    def complete_download(self, task_id: str, success: bool = True):
        """Mark download as complete or failed"""
        download_json = self.valkey_client.get(f"download:{task_id}")
        if download_json:
            download = json.loads(download_json)
            download["progress"] = 100.0 if success else 0.0
            download["status"] = "SUCCESS" if success else "FAILED"
            download["completed_at"] = datetime.datetime.now().isoformat()
            self.valkey_client.set(f"download:{task_id}", json.dumps(download))

            # Remove from user's active downloads after 1 hour
            self.valkey_client.expire(f"download:{task_id}", 3600)

            # If success, remove from active downloads list
            if success:
                self._remove_from_user_downloads(download.get("user_id"), task_id)

    def get_download(self, task_id: str) -> Dict[str, Any]:
        """Get download status"""
        download_json = self.valkey_client.get(f"download:{task_id}")
        if download_json:
            return json.loads(download_json)
        return {}

    def get_user_downloads(self, user_id: int) -> List[Dict[str, Any]]:
        """Get all active downloads for a user"""
        downloads_list_json = self.valkey_client.get(f"user_downloads:{user_id}")
        result = []

        if downloads_list_json:
            download_ids = json.loads(downloads_list_json)
            for task_id in download_ids:
                download_info = self.get_download(task_id)
                if download_info:
                    result.append(download_info)

        return result

    def _add_to_user_downloads(self, user_id: int, task_id: str):
        """Add a task to the user's active downloads list"""
        downloads_list_json = self.valkey_client.get(f"user_downloads:{user_id}")
        if downloads_list_json:
            downloads_list = json.loads(downloads_list_json)
            if task_id not in downloads_list:
                downloads_list.append(task_id)
        else:
            downloads_list = [task_id]

        self.valkey_client.set(f"user_downloads:{user_id}", json.dumps(downloads_list))
        # Set TTL for 7 days
        self.valkey_client.expire(f"user_downloads:{user_id}", 604800)

    def _remove_from_user_downloads(self, user_id: int, task_id: str):
        """Remove a task from the user's active downloads list"""
        downloads_list_json = self.valkey_client.get(f"user_downloads:{user_id}")
        if downloads_list_json:
            downloads_list = json.loads(downloads_list_json)
            if task_id in downloads_list:
                downloads_list.remove(task_id)
                self.valkey_client.set(f"user_downloads:{user_id}", json.dumps(downloads_list))

# Initialize the download progress manager
download_manager = DownloadProgressManager()

@celery_app.task(bind=True, max_retries=3)
def download_podcast_task(self, episode_id: int, user_id: int, database_type: str):
    """
    Celery task to download a podcast episode.
    Uses retries with exponential backoff for handling transient failures.
    """
    task_id = self.request.id
    print(f"DOWNLOAD TASK STARTED: ID={task_id}, Episode={episode_id}, User={user_id}")

    try:
        # Register the download
        download_manager.start_download(task_id, user_id, episode_id, False)

        # Get a database connection from the new module
        from database_functions.db_client import create_database_connection, close_database_connection
        cnx = create_database_connection()

        # Import the download function
        from database_functions.functions import download_podcast

        print(f"Starting download for episode: {episode_id}, user: {user_id}, task: {task_id}")

        # Execute the download
        success = download_podcast(cnx, database_type, episode_id, user_id, task_id)

        # Mark download as complete
        download_manager.complete_download(task_id, success)

        # Close connection properly
        close_database_connection(cnx)

        return success
    except Exception as exc:
        print(f"Error downloading podcast {episode_id}: {str(exc)}")
        # Mark download as failed
        download_manager.complete_download(task_id, False)
        # Retry with exponential backoff (5s, 25s, 125s)
        countdown = 5 * (2 ** self.request.retries)
        self.retry(exc=exc, countdown=countdown)

@celery_app.task(bind=True, max_retries=3)
def download_youtube_video_task(self, video_id: int, user_id: int, database_type: str):
    """
    Celery task to download a YouTube video.
    Uses retries with exponential backoff for handling transient failures.
    """
    task_id = self.request.id

    try:
        # Register the download
        download_manager.start_download(task_id, user_id, video_id, True)

        # Create a new database connection for this task
        from database_functions.functions import create_database_connection, download_youtube_video
        cnx = create_database_connection()

        print(f"Starting download for YouTube video: {video_id}, user: {user_id}, task: {task_id}")

        # Modify download_youtube_video to accept task_id for progress tracking
        success = download_youtube_video(cnx, database_type, video_id, user_id, task_id)

        # Mark download as complete
        download_manager.complete_download(task_id, success)

        cnx.close()
        return success
    except Exception as exc:
        print(f"Error downloading YouTube video {video_id}: {str(exc)}")
        # Mark download as failed
        download_manager.complete_download(task_id, False)
        # Retry with exponential backoff (5s, 25s, 125s)
        countdown = 5 * (2 ** self.request.retries)
        self.retry(exc=exc, countdown=countdown)

@celery_app.task
def queue_podcast_downloads(podcast_id: int, user_id: int, database_type: str, is_youtube: bool = False):
    """
    Task to queue individual download tasks for all episodes/videos in a podcast.
    This adds downloads to the queue in small batches to prevent overwhelming the system.
    """
    from database_functions.db_client import create_database_connection, close_database_connection
    from database_functions.functions import (
        get_episode_ids_for_podcast,
        get_video_ids_for_podcast,
        check_downloaded
    )

    cnx = create_database_connection()
    try:
        if is_youtube:
            item_ids = get_video_ids_for_podcast(cnx, database_type, podcast_id)
            print(f"Queueing {len(item_ids)} YouTube videos for download")
        else:
            item_ids = get_episode_ids_for_podcast(cnx, database_type, podcast_id)
            print(f"Queueing {len(item_ids)} podcast episodes for download")

        # Add items to download queue in batches to prevent overwhelming the system
        batch_size = 5  # Process 5 items at a time
        for i in range(0, len(item_ids), batch_size):
            batch = item_ids[i:i+batch_size]

            for item_id in batch:
                if not check_downloaded(cnx, database_type, user_id, item_id, is_youtube):
                    if is_youtube:
                        download_youtube_video_task.delay(item_id, user_id, database_type)
                    else:
                        download_podcast_task.delay(item_id, user_id, database_type)

            # Add a small delay between batches to prevent sudden spikes
            if i + batch_size < len(item_ids):
                time.sleep(2)

        return f"Queued {len(item_ids)} items for download"
    finally:
        close_database_connection(cnx)

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


@celery_app.task
def debug_task(x, y):
    """Simple debug task that prints output"""
    result = x + y
    print(f"CELERY DEBUG TASK EXECUTED: {x} + {y} = {result}")
    return result
