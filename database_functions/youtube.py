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

    try:
        with YoutubeDL(ydl_opts) as ydl:
            channel_url = f"https://www.youtube.com/channel/{channel_id}"
            channel_info = ydl.extract_info(
                channel_url,
                download=False,
                process=False
            )

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
    ydl_opts = {
        'format': 'bestaudio/best',
        'postprocessors': [{
            'key': 'FFmpegExtractAudio',
            'preferredcodec': 'mp3',
        }],
        'outtmpl': output_path
    }
    with YoutubeDL(ydl_opts) as ydl:
        ydl.download([f"https://www.youtube.com/watch?v={video_id}"])
