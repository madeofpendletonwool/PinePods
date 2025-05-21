from typing import Optional

def send_email(server_name, server_port, from_email, to_email, send_mode, encryption, auth_required, username, password, subject, body):
    import smtplib
    from email.mime.multipart import MIMEMultipart
    from email.mime.text import MIMEText
    import ssl
    import socket

    try:
        if send_mode == "SMTP":
            # Set up the SMTP server.
            if encryption == "SSL/TLS":
                smtp = smtplib.SMTP_SSL(server_name, server_port, timeout=10)
            elif encryption == "STARTTLS":
                smtp = smtplib.SMTP(server_name, server_port, timeout=10)
                smtp.starttls()
            else:  # No encryption
                smtp = smtplib.SMTP(server_name, server_port, timeout=10)


            # Authenticate if needed.
            if auth_required:
                try:  # Trying to login and catching specific SMTPNotSupportedError
                    smtp.login(username, password)
                except smtplib.SMTPNotSupportedError:
                    return 'SMTP AUTH extension not supported by server.'

            # Create a message.
            msg = MIMEMultipart()
            msg['From'] = from_email
            msg['To'] = to_email
            msg['Subject'] = subject
            msg.attach(MIMEText(body, 'plain'))

            # Send the message.
            smtp.send_message(msg)
            smtp.quit()
            return 'Email sent successfully.'

        elif send_mode == "Sendmail":
            pass
    except ssl.SSLError:
        return 'SSL Wrong Version Number. Try another ssl type?'
    except smtplib.SMTPAuthenticationError:
        return 'Authentication Error: Invalid username or password.'
    except smtplib.SMTPRecipientsRefused:
        return 'Recipients Refused: Email address is not accepted by the server.'
    except smtplib.SMTPSenderRefused:
        return 'Sender Refused: Sender address is not accepted by the server.'
    except smtplib.SMTPDataError:
        return 'Unexpected server response: Possibly the message data was rejected by the server.'
    except socket.gaierror:
        return 'Server Not Found: Please check your server settings.'
    except ConnectionRefusedError:
        return 'Connection Refused: The server refused the connection.'
    except TimeoutError:
        return 'Timeout Error: The connection to the server timed out.'
    except smtplib.SMTPException as e:
        return f'Failed to send email: {str(e)}'



def sync_with_nextcloud(nextcloud_url, nextcloud_token):
    print("Starting Nextcloud Sync")

    headers = {
        "Authorization": f"Bearer {nextcloud_token}",
        "Content-Type": "application/json"
    }

    # Sync Subscriptions
    sync_subscriptions(nextcloud_url, headers)

    # Sync Episode Actions
    sync_episode_actions(nextcloud_url, headers)


def sync_subscriptions(nextcloud_url, headers, user_id):
    import requests
    # Implement fetching and updating subscriptions
    # Example GET request to fetch subscriptions
    response = requests.get(f"{nextcloud_url}/index.php/apps/gpoddersync/subscriptions", headers=headers)
    # Handle the response
    print(response.json())


def sync_subscription_change(nextcloud_url, headers, add, remove):
    import requests
    payload = {
        "add": add,
        "remove": remove
    }
    response = requests.post(f"{nextcloud_url}/index.php/apps/gpoddersync/subscription_change/create", json=payload,
                             headers=headers)

def sync_subscription_change_gpodder(gpodder_url, gpodder_login, auth, add, remove):
    import requests
    payload = {
        "add": add,
        "remove": remove
    }
    response = requests.post(f"{gpodder_url}/api/2/subscriptions/{gpodder_login}/default.json", json=payload, auth=auth)
    response.raise_for_status()
    print(f"Subscription changes synced with gPodder: {response.text}")


def sync_subscription_change_gpodder_session(session, gpodder_url, gpodder_login, add, remove):
    """Sync subscription changes using session-based authentication"""
    import logging

    logger = logging.getLogger(__name__)

    payload = {
        "add": add,
        "remove": remove
    }

    try:
        response = session.post(
            f"{gpodder_url}/api/2/subscriptions/{gpodder_login}/default.json",
            json=payload
        )
        response.raise_for_status()
        logger.info(f"Subscription changes synced with gPodder using session: {response.text}")
        return True
    except Exception as e:
        logger.error(f"Error syncing subscription changes with session: {str(e)}")
        return False

def sync_episode_actions(nextcloud_url, headers):
    print('test')
    # Implement fetching and creating episode actions
    # Similar to the sync_subscriptions method

def get_podcast_values(feed_url, user_id, username: Optional[str] = None, password: Optional[str] = None, display_only: bool = False):
    import feedparser
    import json
    import requests
    from requests.auth import HTTPBasicAuth

    # Use requests to fetch the feed content
    try:
        # Simpler headers that worked in the original version
        headers = {
            'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/58.0.3029.110 Safari/537.3',
            'Accept-Language': 'en-US,en;q=0.9',
        }
        print(f"Fetching URL: {feed_url}")

        if username and password:
            print(f"Using auth for user: {username}")
            response = requests.get(feed_url, headers=headers, auth=HTTPBasicAuth(username, password))
        else:
            response = requests.get(feed_url, headers=headers)

        response.raise_for_status()
        # Use binary content which worked in the original version
        feed_content = response.content

    except requests.RequestException as e:
        try:
            if 'response' in locals():
                print(f"Response headers: {response.headers}")
                print(f"Response content: {response.content[:500]}")
        except:
            pass
        raise ValueError(f"Error fetching the feed: {str(e)}")

    # Parse the feed
    d = feedparser.parse(feed_content)
    print(f"Feed parsed - title: {d.feed.get('title', 'Unknown')}")

    # Initialize podcast_values as in the original version that worked
    podcast_values = {
        'pod_title': d.feed.title if hasattr(d.feed, 'title') else None,
        'pod_artwork': None,  # We'll set this with multiple checks below
        'pod_author': d.feed.author if hasattr(d.feed, 'author') else None,
        'categories': [],
        'pod_description': d.feed.description if hasattr(d.feed, 'description') else None,
        'pod_episode_count': len(d.entries) if display_only else 0,
        'pod_feed_url': feed_url,
        'pod_website': d.feed.link if hasattr(d.feed, 'link') else None,
        'pod_explicit': False,
        'user_id': user_id
    }

    # Enhanced image URL extraction combining both approaches
    if hasattr(d.feed, 'image'):
        if hasattr(d.feed.image, 'href'):
            podcast_values['pod_artwork'] = d.feed.image.href
        elif hasattr(d.feed.image, 'url'):  # Added for news feed format
            podcast_values['pod_artwork'] = d.feed.image.url
        elif isinstance(d.feed.image, dict):
            if 'href' in d.feed.image:
                podcast_values['pod_artwork'] = d.feed.image['href']
            elif 'url' in d.feed.image:
                podcast_values['pod_artwork'] = d.feed.image['url']

    # iTunes image fallback
    if not podcast_values['pod_artwork'] and hasattr(d.feed, 'itunes_image'):
        if hasattr(d.feed.itunes_image, 'href'):
            podcast_values['pod_artwork'] = d.feed.itunes_image.href
        elif isinstance(d.feed.itunes_image, dict) and 'href' in d.feed.itunes_image:
            podcast_values['pod_artwork'] = d.feed.itunes_image['href']

    # Author fallback
    if not podcast_values['pod_author'] and hasattr(d.feed, 'itunes_author'):
        podcast_values['pod_author'] = d.feed.itunes_author

    # Description fallbacks
    if not podcast_values['pod_description']:
        if hasattr(d.feed, 'subtitle'):
            podcast_values['pod_description'] = d.feed.subtitle
        elif hasattr(d.feed, 'itunes_summary'):
            podcast_values['pod_description'] = d.feed.itunes_summary

    # Category extraction with robust error handling
    try:
        if hasattr(d.feed, 'itunes_category'):
            if isinstance(d.feed.itunes_category, list):
                for cat in d.feed.itunes_category:
                    if isinstance(cat, dict) and 'text' in cat:
                        podcast_values['categories'].append(cat['text'])
                    elif hasattr(cat, 'text'):
                        podcast_values['categories'].append(cat.text)
            elif isinstance(d.feed.itunes_category, dict) and 'text' in d.feed.itunes_category:
                podcast_values['categories'].append(d.feed.itunes_category['text'])
    except Exception as e:
        print(f"Error extracting categories: {e}")

    # Handle empty categories
    if not podcast_values['categories']:
        podcast_values['categories'] = {'1': 'Podcasts'}  # Default category
    else:
        categories_dict = {str(i): cat for i, cat in enumerate(podcast_values['categories'], start=1)}
        podcast_values['categories'] = categories_dict

    # Add explicit check with robust handling
    try:
        if hasattr(d.feed, 'itunes_explicit'):
            if isinstance(d.feed.itunes_explicit, str):
                podcast_values['pod_explicit'] = d.feed.itunes_explicit.lower() in ('yes', 'true', '1')
            elif isinstance(d.feed.itunes_explicit, bool):
                podcast_values['pod_explicit'] = d.feed.itunes_explicit
    except Exception as e:
        print(f"Error checking explicit flag: {e}")

    # Print values for debugging
    print("Extracted podcast values:")
    for key, value in podcast_values.items():
        print(f"{key}: {value}")

    return podcast_values



def check_valid_feed(feed_url: str, username: Optional[str] = None, password: Optional[str] = None):
    """
    Check if the provided URL points to a valid podcast feed.
    Raises ValueError if the feed is invalid.
    """
    import feedparser
    import requests
    # Use requests to fetch the feed content
    try:
        if username and password:
            response = requests.get(feed_url, auth=(username, password))
        else:
            response = requests.get(feed_url)

        response.raise_for_status()  # Raise an exception for HTTP errors
        feed_content = response.content
    except requests.RequestException as e:
        raise ValueError(f"Error fetching the feed: {str(e)}")

    # Parse the feed
    parsed_feed = feedparser.parse(feed_content)

    # Check for basic RSS or Atom feed structure
    if not parsed_feed.get('version'):
        raise ValueError("Invalid podcast feed URL or content.")

    # Check for essential elements in the feed
    if not ('title' in parsed_feed.feed and 'link' in parsed_feed.feed and 'description' in parsed_feed.feed):
        raise ValueError("Feed missing required attributes: title, link, or description.")

    # If it passes the above checks, it's likely a valid feed
    return parsed_feed
