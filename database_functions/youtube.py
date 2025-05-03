from typing import Dict, Optional
from yt_dlp import YoutubeDL
from fastapi import HTTPException
import logging
import os
import datetime
from datetime import timedelta
import logging
from bs4 import BeautifulSoup
import time
import random
from database_functions import functions


async def get_channel_info(channel_id: str) -> Dict:
    """
    Get YouTube channel info using yt-dlp
    """
    ydl_opts = {
        'quiet': True,
        'extract_flat': True,
        'no_warnings': True,
        'playlist_items': '0'  # Just get channel info, not videos
    }
    print('in get channel info')

    try:
        with YoutubeDL(ydl_opts) as ydl:
            channel_url = f"https://www.youtube.com/channel/{channel_id}"
            channel_info = ydl.extract_info(
                channel_url,
                download=False,
                process=False
            )
            print(f'get info {channel_info}')

            # Get avatar URL
            thumbnail_url = None
            if channel_info and channel_info.get('thumbnails'):
                avatar_thumbnails = [t for t in channel_info['thumbnails']
                                   if t.get('id', '').startswith('avatar')]

                if avatar_thumbnails:
                    thumbnail_url = avatar_thumbnails[-1]['url']
                else:
                    avatar_thumbnails = [t for t in channel_info['thumbnails']
                                       if 'avatar' in t.get('url', '').lower()]
                    if avatar_thumbnails:
                        thumbnail_url = avatar_thumbnails[-1]['url']
                    else:
                        thumbnail_url = channel_info['thumbnails'][0]['url']
            print('did a bunch of thumbnail stuff')
            return {
                'channel_id': channel_id,
                'name': channel_info.get('channel', '') or channel_info.get('title', ''),
                'description': channel_info.get('description', '')[:500] if channel_info.get('description') else '',
                'thumbnail_url': thumbnail_url,
            }

    except Exception as e:
        logging.error(f"Error getting channel info: {e}")
        raise HTTPException(
            status_code=500,
            detail=f"Error fetching channel info: {str(e)}"
        )

def download_youtube_audio(video_id: str, output_path: str):
    """Download audio for a YouTube video"""
    # Remove .mp3 extension if present to prevent double extension
    if output_path.endswith('.mp3'):
        base_path = output_path[:-4]
    else:
        base_path = output_path

    ydl_opts = {
        'format': 'bestaudio/best',
        'postprocessors': [{
            'key': 'FFmpegExtractAudio',
            'preferredcodec': 'mp3',
        }],
        'outtmpl': base_path
    }
    with YoutubeDL(ydl_opts) as ydl:
        ydl.download([f"https://www.youtube.com/watch?v={video_id}"])


def process_youtube_videos(database_type, podcast_id: int, channel_id: str, cnx, feed_cutoff: int):
    """Background task to process videos and download audio"""

    logging.basicConfig(level=logging.INFO)
    logger = logging.getLogger(__name__)

    logger.info("="*50)
    logger.info(f"Starting YouTube channel processing")
    logger.info(f"Podcast ID: {podcast_id}")
    logger.info(f"Channel ID: {channel_id}")
    logger.info("="*50)

    try:
        cutoff_date = datetime.datetime.now(datetime.timezone.utc) - timedelta(days=feed_cutoff)
        logger.info(f"Cutoff date set to: {cutoff_date}")

        ydl_opts = {
            'quiet': True,
            'no_warnings': True,
            'extract_flat': True,  # Fast initial fetch
            'ignoreerrors': True,
        }

        logger.info("Initializing YouTube-DL with options:")
        logger.info(str(ydl_opts))

        recent_videos = []
        with YoutubeDL(ydl_opts) as ydl:
            channel_url = f"https://www.youtube.com/channel/{channel_id}/videos"
            logger.info(f"Fetching channel data from: {channel_url}")

            try:
                results = ydl.extract_info(channel_url, download=False)
                logger.info("Initial channel data fetch successful")
                logger.info(f"Raw result keys: {results.keys() if results else 'No results'}")
            except Exception as e:
                logger.error(f"Failed to fetch channel data: {str(e)}")
                raise

            if not results or 'entries' not in results:
                logger.error(f"No video list found in results")
                logger.error(f"Available keys: {results.keys() if results else 'None'}")
                return

            logger.info(f"Found {len(results.get('entries', []))} total videos")

            # Process each video
            for entry in results.get('entries', []):
                if not entry or not entry.get('id'):
                    logger.warning(f"Skipping invalid entry: {entry}")
                    continue

                try:
                    video_id = entry['id']
                    logger.info(f"\nProcessing video ID: {video_id}")

                    # Get upload date using BS4 method
                    published = functions.get_video_date(video_id)
                    if not published:
                        logger.warning(f"Could not determine upload date for video {video_id}, skipping")
                        continue

                    logger.info(f"Video publish date: {published}")

                    if published <= cutoff_date:
                        logger.info(f"Video {video_id} from {published} is too old, stopping processing")
                        break

                    video_data = {
                        'id': video_id,
                        'title': entry['title'],
                        'description': entry.get('description', ''),
                        'url': f"https://www.youtube.com/watch?v={video_id}",
                        'thumbnail': entry.get('thumbnails', [{}])[0].get('url', '') if entry.get('thumbnails') else '',
                        'publish_date': published,
                        'duration': entry.get('duration', 0)
                    }

                    logger.info("Collected video data:")
                    logger.info(str(video_data))

                    recent_videos.append(video_data)
                    logger.info(f"Successfully added video {video_id} to processing queue")

                except Exception as e:
                    logger.error(f"Error processing video entry:")
                    logger.error(f"Entry data: {entry}")
                    logger.error(f"Error: {str(e)}")
                    logger.exception("Full traceback:")
                    continue

        logger.info(f"\nProcessing complete - Found {len(recent_videos)} recent videos")

        if recent_videos:
            logger.info("\nStarting database updates")
            try:
                # Get existing videos first
                existing_videos = functions.get_existing_youtube_videos(cnx, database_type, podcast_id)

                # Filter out videos that already exist
                new_videos = []
                for video in recent_videos:
                    video_url = f"https://www.youtube.com/watch?v={video['id']}"
                    if video_url not in existing_videos:
                        new_videos.append(video)
                    else:
                        logger.info(f"Video already exists, skipping: {video['title']}")

                if new_videos:
                    functions.add_youtube_videos(cnx, database_type, podcast_id, new_videos)
                    logger.info(f"Successfully added {len(new_videos)} new videos")
                else:
                    logger.info("No new videos to add")
            except Exception as e:
                logger.error("Failed to add videos to database")
                logger.error(str(e))
                logger.exception("Full traceback:")
                raise

            logger.info("\nStarting audio downloads")
            for video in recent_videos:
                try:
                    output_path = f"/opt/pinepods/downloads/youtube/{video['id']}.mp3"
                    output_path_double = f"{output_path}.mp3"

                    logger.info(f"\nProcessing download for video: {video['id']}")
                    logger.info(f"Title: {video['title']}")
                    logger.info(f"Target path: {output_path}")

                    if os.path.exists(output_path) or os.path.exists(output_path_double):
                        logger.info(f"Audio file already exists, skipping download")
                        continue

                    logger.info("Starting download...")
                    download_youtube_audio(video['id'], output_path)
                    logger.info("Download completed successfully")

                except Exception as e:
                    logger.error(f"Failed to download video {video['id']}")
                    logger.error(str(e))
                    logger.exception("Full traceback:")
                    continue
        else:
            logger.info("No new videos to process")

    except Exception as e:
        logger.error("\nFatal error in process_youtube_channel")
        logger.error(str(e))
        logger.exception("Full traceback:")
        raise e
    finally:
        logger.info("\nCleaning up database connection")
        logger.info("="*50)
        logger.info("Channel processing complete")
        logger.info("="*50)
