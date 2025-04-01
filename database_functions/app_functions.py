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
        headers = {
            'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/58.0.3029.110 Safari/537.3',
            'Accept-Language': 'en-US,en;q=0.9',
        }
        print(f"Fetching URL: {feed_url}")
        print(f"Headers: {headers}")
        if username and password:
            print(f"Using auth for user: {username}")
            response = requests.get(feed_url, headers=headers, auth=HTTPBasicAuth(username, password))
        else:
            response = requests.get(feed_url, headers=headers)

        response.raise_for_status()  # Raise an exception for HTTP errors
        feed_content = response.content
    except requests.RequestException as e:
        print(f"Response headers: {response.headers}")
        print(f"Response content: {response.content}")
        raise ValueError(f"Error fetching the feed: {str(e)}")

    # Parse the feed
    d = feedparser.parse(feed_content)

    # Initialize podcast_values as a dictionary
    podcast_values = {
        'pod_title': d.feed.title if hasattr(d.feed, 'title') else None,
        'pod_artwork': d.feed.image.href if hasattr(d.feed, 'image') and hasattr(d.feed.image, 'href') else None,
        'pod_author': d.feed.author if hasattr(d.feed, 'author') else None,
        'categories': [],
        'pod_description': d.feed.description if hasattr(d.feed, 'description') else None,
        'pod_episode_count': len(d.entries) if display_only else 0,
        'pod_feed_url': feed_url,
        'pod_website': d.feed.link if hasattr(d.feed, 'link') else None,
        'pod_explicit': False,
        'user_id': user_id
    }

    if not podcast_values['pod_artwork'] and hasattr(d.feed, 'itunes_image'):
        podcast_values['pod_artwork'] = d.feed.itunes_image['href']

    if not podcast_values['pod_author'] and hasattr(d.feed, 'itunes_author'):
        podcast_values['pod_author'] = d.feed.itunes_author

    # Extracting categories, primarily from iTunes
    if hasattr(d.feed, 'itunes_category'):
        for cat in d.feed.itunes_category:
            podcast_values['categories'].append(cat['text'])
            if 'itunes_category' in cat:
                for subcat in cat['itunes_category']:
                    podcast_values['categories'].append(subcat['text'])

    # Now, check if categories list is empty after attempting to populate it
    if not podcast_values['categories']:
        podcast_values['categories'] = ""  # Set to empty string if no categories found
    else:
        categories_dict = {str(i): cat for i, cat in enumerate(podcast_values['categories'], start=1)}
        podcast_values['categories'] = json.dumps(categories_dict)  # Serialize populated categories dict

    if not podcast_values['pod_description'] and hasattr(d.feed, 'itunes_summary'):
        podcast_values['pod_description'] = d.feed.itunes_summary

    # Check for explicit content
    if hasattr(d.feed, 'itunes_explicit'):
        podcast_values['pod_explicit'] = d.feed.itunes_explicit == 'yes'

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
