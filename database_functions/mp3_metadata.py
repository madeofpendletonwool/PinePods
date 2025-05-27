from mutagen.easyid3 import EasyID3
from mutagen.id3 import ID3, APIC, ID3NoHeaderError
from mutagen.mp3 import MP3
import mutagen
import requests

def add_podcast_metadata(file_path, metadata):
    """
    Add metadata to a downloaded podcast MP3 file.

    Args:
        file_path (str): Path to the MP3 file
        metadata (dict): Dictionary containing metadata with keys:
            - title: Episode title
            - artist: Podcast author/creator
            - album: Podcast name
            - date: Publication date
            - description: Episode description
            - artwork_url: URL to episode/podcast artwork
    """
    try:
        # Try to load existing ID3 tags, create them if they don't exist
        try:
            audio = EasyID3(file_path)
        except mutagen.id3.ID3NoHeaderError:
            audio = MP3(file_path)
            audio.add_tags()
            audio.save()
            audio = EasyID3(file_path)

        # Add basic text metadata using valid EasyID3 keys
        if 'title' in metadata:
            audio['title'] = metadata['title']
        if 'artist' in metadata:
            audio['artist'] = metadata['artist']
        if 'album' in metadata:
            audio['album'] = metadata['album']
        if 'date' in metadata:
            audio['date'] = metadata['date']

        # Save the text metadata
        audio.save()

        # Handle artwork separately (requires full ID3)
        if 'artwork_url' in metadata and metadata['artwork_url']:
            try:
                # Download artwork
                artwork_response = requests.get(metadata['artwork_url'])
                artwork_response.raise_for_status()

                # Add artwork to the file
                audio = ID3(file_path)
                audio.add(APIC(
                    encoding=3,  # UTF-8
                    mime='image/jpeg',  # Assume JPEG
                    type=3,  # Cover image
                    desc='Cover',
                    data=artwork_response.content
                ))
                audio.save()
            except Exception as e:
                print(f"Failed to add artwork: {str(e)}")

    except Exception as e:
        print(f"Failed to add metadata to {file_path}: {str(e)}")
