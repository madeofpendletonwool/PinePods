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
    # Handle the response


def sync_episode_actions(nextcloud_url, headers):
    print('test')
    # Implement fetching and creating episode actions
    # Similar to the sync_subscriptions method

def get_podcast_values(feed_url, user_id):
    import feedparser
    import json
    # Parse the feed
    d = feedparser.parse(feed_url)

    # Initialize podcast_values as a dictionary
    podcast_values = {
        'pod_title': d.feed.title if hasattr(d.feed, 'title') else None,
        'pod_artwork': d.feed.image.href if hasattr(d.feed, 'image') and hasattr(d.feed.image, 'href') else None,
        'pod_author': d.feed.author if hasattr(d.feed, 'author') else None,
        'categories': [],
        'pod_description': d.feed.description if hasattr(d.feed, 'description') else None,
        'pod_episode_count': len(d.entries),
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
