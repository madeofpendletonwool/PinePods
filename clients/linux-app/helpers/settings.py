# Various flet imports
import flet as ft
from flet import Text, colors, icons, ButtonStyle, Row, alignment, border_radius, animation, \
    MainAxisAlignment, padding

import internal_functions.functions
import Auth.Passfunctions
import api_functions.functions
from api_functions.functions import call_api_config
import app_functions.functions
from helpers import api
from helpers import user


# Other Imports
import os
import pyotp
import requests
import time
import datetime
import qrcode
import appdirs

appname = "pinepods"
appauthor = "Gooseberry Development"

# user_data_dir would be the equivalent to the home directory you were using
user_data_dir = appdirs.user_data_dir(appname, appauthor)
metadata_dir = os.path.join(user_data_dir, 'metadata')
backup_dir = os.path.join(user_data_dir, 'backups')
assets_dir = os.path.join(user_data_dir, 'assets')

class Settings:
    def __init__(self, page, app_api, active_user, user_data_dir, pr_instance, modify_user, login_username, login_password, new_nav):
        self.page = page
        self.app_api = app_api
        self.active_user = active_user
        self.user_data_dir = user_data_dir
        self.pr_instance = pr_instance
        self.modify_user = modify_user
        self.login_username = login_username
        self.login_password = login_password
        self.new_nav = new_nav
        # Guest login Setup
        self.guest_status_bool = api_functions.functions.call_guest_status(self.app_api.url, self.app_api.headers)
        self.disable_guest_notify = ft.Text(
            f'Guest user is currently {"enabled" if self.guest_status_bool else "disabled"}')
        self.guest_check()
        # Self Service user create setup
        self.self_service_bool = api_functions.functions.call_self_service_status(self.app_api.url,
                                                                                  self.app_api.headers)
        self.self_service_notify = ft.Text(
            f'Self Service user creation is currently {"enabled" if self.self_service_bool else "disabled"}')
        self.self_service_check()
        # local settings clear
        self.settings_clear_options()
        # Backup Settings Setup
        self.settings_backup_data()
        # Import Settings Setup
        self.settings_import_data()
        # Server Downloads Setup
        self.download_status_bool = api_functions.functions.call_download_status(self.app_api.url,
                                                                                 self.app_api.headers)
        self.disable_download_notify = ft.Text(
            f'Downloads are currently {"enabled" if self.download_status_bool else "disabled"}')
        self.downloads_check()

        # MFA Settings Setup
        self.check_mfa_status = api_functions.functions.call_check_mfa_enabled(self.app_api.url, self.app_api.headers,
                                                                               self.active_user.user_id)
        print('in settings')
        self.mfa_check()
        # Setup gpodder functionality
        self.check_gpodder_status = api_functions.functions.call_check_gpodder_access(self.app_api.url, self.app_api.headers,
                                                                                      self.active_user.user_id)
        print(self.check_gpodder_status)
        print('checked gpod settings')
        self.gpodder_setup()

        if self.active_user.user_is_admin:
            # New User Creation Setup
            self.user_table_rows = []
            self.user_table_load()
            # Email Settings Setup
            self.email_information = api_functions.functions.call_get_email_info(self.app_api.url,
                                                                                 self.app_api.headers)
            self.email_table_rows = []
            self.email_table_load()

    def setup_user_for_otp(self):
        # generate a new secret for the user
        secret = pyotp.random_base32()

        # create a provisioning URL that the user can scan with their OTP app
        provisioning_url = pyotp.totp.TOTP(secret).provisioning_uri(name=self.active_user.email, issuer_name='PinePods')

        # convert this provisioning URL into a QR code and display it to the user
        # generate the QR code
        img = qrcode.make(provisioning_url)

        # Get current timestamp
        self.active_user.mfa_timestamp = datetime.datetime.now().strftime("%Y%m%d%H%M%S")

        # Save it to a file with a unique name
        filename = f"{self.user_data_dir}/{self.active_user.user_id}_qrcode_{self.active_user.mfa_timestamp}.png"  # for example
        img.save(filename)
        self.active_user.mfa_secret = secret

        return filename

    def gpodder_setup(self):
        gpodder_option_text = Text('Gpodder Sync:', color=self.active_user.font_color, size=16)

        if self.check_gpodder_status['data']:
            print('in true')
            gpodder_option_desc = Text(
                "Note: Signing out of Gpodder Sync will remove the syncing that Pinepods does occasionally with Gpodder. This will not remove any podcasts from your account.",
                color=self.active_user.font_color)
            self.gpodder_sign_in_button = ft.ElevatedButton(f'Sign Out of Gpodder Sync',
                                                            on_click=self.gpodder_sign_out,
                                                            bgcolor=self.active_user.main_color,
                                                            color=self.active_user.accent_color)
        else:
            print('in false')
            gpodder_option_desc = Text(
                "Note: This option allows you to setup gpodder sync in Pinepods. Click the sign in button below to sync your podcasts up. Note that if you have any existing subscriptions in your gpodder account Pinepods will add those to it's database and then sync any additional subscriptions it already has up with Gpodder. From there, Pinepods will occasionally sync with gpodder. Otherwise, you can manually run a sync from here once signed in.",
                color=self.active_user.font_color)
            self.gpodder_sign_in_button = ft.ElevatedButton(f'Sign in',
                                                            on_click=self.gpodder_sign_in,
                                                            bgcolor=self.active_user.main_color,
                                                            color=self.active_user.accent_color)
        gpodder_backup_col = ft.Column(
            controls=[gpodder_option_text, gpodder_option_desc, self.gpodder_sign_in_button])
        self.setting_gpodder_con = ft.Container(content=gpodder_backup_col)
        self.setting_gpodder_con.padding = padding.only(left=70, right=50)
        print('bottom setup')

    def settings_backup_data(self):
        backup_option_text = Text('Backup Data:', color=self.active_user.font_color, size=16)
        backup_option_desc = Text(
            "Note: This option allows you to backup data in Pinepods. This can be used to backup podcasts to an opml file, or if you're an admin, it can also backup server information for a full restore. Like users, and current server settings.",
            color=self.active_user.font_color)
        self.settings_backup_button = ft.ElevatedButton(f'Backup Data',
                                                        on_click=self.backup_data,
                                                        bgcolor=self.active_user.main_color,
                                                        color=self.active_user.accent_color)
        setting_backup_col = ft.Column(
            controls=[backup_option_text, backup_option_desc, self.settings_backup_button])
        self.setting_backup_con = ft.Container(content=setting_backup_col)
        self.setting_backup_con.padding = padding.only(left=70, right=50)

    def settings_import_data(self):
        import_option_text = Text('Import Data:', color=self.active_user.font_color, size=16)
        import_option_desc = Text(
            "Note: This option allows you to import backed up data into Pinepods. You can import OPML files for podcast rss feeds and, if you're an admin, you can import entire server information.",
            color=self.active_user.font_color)
        self.settings_import_button = ft.ElevatedButton(f'Import Data',
                                                        on_click=self.import_data,
                                                        bgcolor=self.active_user.main_color,
                                                        color=self.active_user.accent_color)
        setting_import_col = ft.Column(
            controls=[import_option_text, import_option_desc, self.settings_import_button])
        self.setting_import_con = ft.Container(content=setting_import_col)
        self.setting_import_con.padding = padding.only(left=70, right=50)

    def settings_clear_options(self):
        setting_option_text = Text('Clear existing client data:', color=self.active_user.font_color, size=16)
        setting_option_desc = Text(
            "Note: This will clear all your local client data. If you have locally downloaded episodes they will be deleted and your saved configuration will be removed. You'll need to setup both your server configuration and your user login",
            color=self.active_user.font_color)
        self.settings_clear_button = ft.ElevatedButton(f'Clear Client',
                                                       on_click=self.active_user.logout_pinepods_clear_local,
                                                       bgcolor=self.active_user.main_color,
                                                       color=self.active_user.accent_color)
        setting_option_col = ft.Column(
            controls=[setting_option_text, setting_option_desc, self.settings_clear_button])
        self.setting_option_con = ft.Container(content=setting_option_col)
        self.setting_option_con.padding = padding.only(left=70, right=50)

    def update_mfa_status(self):
        self.check_mfa_status = api_functions.functions.call_check_mfa_enabled(
            self.self.app_api.url, self.self.app_api.headers, self.active_user.user_id
        )
        if self.check_mfa_status:
            self.mfa_button.text = f'Re-Setup MFA for your account'
            self.mfa_button.on_click = self.mfa_option_change
            if 'mfa_remove_button' not in dir(self):  # create mfa_remove_button if it doesn't exist
                self.mfa_remove_button = ft.ElevatedButton(f'Remove MFA for your account',
                                                           on_click=self.remove_mfa,
                                                           bgcolor=self.active_user.main_color,
                                                           color=self.active_user.accent_color)
            if self.mfa_button_row is None:
                self.mfa_button_row = ft.Row()
            self.mfa_button_row.controls = [self.mfa_button, self.mfa_remove_button]
        else:
            self.mfa_button.text = f'Setup MFA for your account'
            self.mfa_button.on_click = self.setup_mfa
            if 'mfa_remove_button' in dir(
                    self):  # remove mfa_remove_button from mfa_button_row.controls if it exists
                self.mfa_button_row.controls = [self.mfa_button]
        self.mfa_container.content = self.mfa_column
        self.page.update()

    def remove_mfa(self, e):
        delete_confirm = api_functions.functions.call_delete_mfa_secret(self.app_api.url, self.app_api.headers,
                                                                        self.active_user.user_id)
        if delete_confirm:
            self.page.snack_bar = ft.SnackBar(content=ft.Text(
                f"MFA now removed from your account. You'll no longer be prompted at login"))
            self.page.snack_bar.open = True
            self.update_mfa_status()
            self.page.update()
        else:
            self.page.snack_bar = ft.SnackBar(
                content=ft.Text(f"Error removing MFA settings. Maybe it's not already setup?"))
            self.page.snack_bar.open = True
            self.page.update()

    def setup_mfa(self, e):
        def close_mfa_dlg(e):
            mfa_dlg.open = False
            os.remove(f"{self.user_data_dir}/{self.active_user.user_id}_qrcode_{self.active_user.mfa_timestamp}.png")
            self.page.update()

        def close_validate_mfa_dlg(page):
            validate_mfa_dlg.open = False
            try:
                os.remove(f"{self.user_data_dir}/{self.active_user.user_id}_qrcode_{self.active_user.mfa_timestamp}.png")
            except:
                pass
            self.page.update()

        def complete_mfa(e):
            # Get the OTP entered by the user
            close_validate_mfa_dlg(self.page)
            self.page.update()

            entered_otp = mfa_confirm_box.value

            # Verify the OTP
            totp = pyotp.TOTP(self.active_user.mfa_secret)
            if totp.verify(entered_otp, valid_window=1):
                # If the OTP is valid, save the MFA secret
                api_functions.functions.call_save_mfa_secret(self.app_api.url, self.app_api.headers,
                                                             self.active_user.user_id, self.active_user.mfa_secret)

                # Close the dialog and show a success message
                close_validate_mfa_dlg(self.page)
                self.page.snack_bar = ft.SnackBar(
                    content=ft.Text(f"MFA now configured! On next login you'll be prompted for your code!"))
                self.page.snack_bar.open = True
                self.update_mfa_status()
                return True
            else:
                # If the OTP is not valid, show an error message
                self.page.snack_bar = ft.SnackBar(content=ft.Text(
                    f"The entered OTP is incorrect. It also may have timed out before you entered it. Please cancel and try again."))
                self.page.snack_bar.open = True
            self.page.update()

        mfa_confirm_box = ft.TextField(label="MFA Code", icon=ft.icons.LOCK_CLOCK, hint_text='123456')
        mfa_validate_select_row = ft.Row(
            controls=[
                ft.TextButton("Confirm", on_click=complete_mfa),
                ft.TextButton("Cancel", on_click=lambda x: (close_validate_mfa_dlg(self.page)))
            ],
            alignment=ft.MainAxisAlignment.END
        )
        validate_mfa_dlg = ft.AlertDialog(
            modal=True,
            title=ft.Text(f"Confirm MFA:"),
            content=ft.Column(controls=[
                ft.Text(f'Please confirm the code from your authenticator app.', selectable=True),
                # ], tight=True),
                mfa_confirm_box,
                # actions=[
                mfa_validate_select_row
            ],
                tight=True),
            actions_alignment=ft.MainAxisAlignment.END,
        )

        def validate_mfa(e):
            close_mfa_dlg(self.page)
            self.page.update()
            time.sleep(.3)

            self.page.dialog = validate_mfa_dlg
            validate_mfa_dlg.open = True
            self.page.update()

        img_data_url = self.setup_user_for_otp()
        mfa_select_row = ft.Row(
            controls=[
                ft.TextButton("Continue", on_click=validate_mfa),
                ft.TextButton("Close", on_click=lambda x: (close_mfa_dlg(self.page)))
            ],
            alignment=ft.MainAxisAlignment.END
        )
        mfa_dlg = ft.AlertDialog(
            modal=True,
            title=ft.Text(f"Setup MFA:"),
            content=ft.Column(controls=[
                ft.Text(
                    f'Scan the code below with your authenticator app and then click continue to validate your code.',
                    selectable=True),
                ft.Image(src=img_data_url, width=200, height=200),
                ft.Text(f'MFA Secret for manual entry: {self.active_user.mfa_secret}', selectable=True),
                ft.Text('Enter TOTP as the type if doing manual entry', selectable=True),
                mfa_select_row
            ],
                tight=True),
            actions_alignment=ft.MainAxisAlignment.END,
        )
        self.page.dialog = mfa_dlg
        mfa_dlg.open = True
        self.page.update()

    def gpodder_sign_in(self, e):
        from threading import Thread, Event
        print('sign in start')

        class NextcloudAuthenticator:
            LOGIN_ENDPOINT = "/index.php/login/v2"

            def __init__(self, server_url, page, on_success_callback):
                self.server_url = server_url
                self.page = page
                self.auth_completed_event = Event()
                self.credentials = None
                self.headers = {"User-Agent": "Pinepods"}
                self.on_success_callback = on_success_callback

            def start_login(self):
                # Start authentication in a new thread to avoid blocking the UI
                thread = Thread(target=self._initiate_login)
                thread.daemon = True
                thread.start()

            def poll_for_auth_completion(self, endpoint, token):
                while not self.auth_completed_event.is_set():
                    try:
                        print(f"Polling {endpoint} with token {token}")
                        res = requests.post(endpoint, json={"token": token}, headers=self.headers)
                        print(f"Response from server: {res.status_code}, {res.text}")
                        if res.status_code == 200:
                            self.credentials = res.json()
                            print(f"Credentials received: {self.credentials}")
                            self.auth_completed_event.set()
                            if self.on_success_callback:
                                self.on_success_callback(self.credentials)  # Pass credentials directly
                            break
                        time.sleep(5)
                    except Exception as e:
                        print(f"Error while polling: {e}")

            def _initiate_login(self):
                auth_url = f"{self.server_url}{self.LOGIN_ENDPOINT}"
                try:
                    res = requests.post(auth_url, headers=self.headers)
                    if res.status_code == 200:
                        response = res.json()
                        self._open_url_in_browser(response["login"])
                        # Store polling endpoint and token for later use
                        self.poll_endpoint = response["poll"]["endpoint"]
                        self.poll_token = response["poll"]["token"]
                        print(f'nextcloud info: token: {self.poll_token} endpoint: {self.poll_endpoint}')
                        # self.active_user.nextcloud_endpoint = self.poll_endpoint
                        # self.active_user.nextcloud_token = self.poll_token

                        # Now start polling in a separate thread
                        poll_thread = Thread(target=self.poll_for_auth_completion,
                                             args=(self.poll_endpoint, self.poll_token))
                        poll_thread.start()
                    else:
                        print("Authentication initiation failed: ", res.status_code)
                        # Handle failed authentication initiation
                except Exception as e:
                    print("Error during authentication initiation: ", e)
                    # Handle exceptions (network issues, etc.)

            def _open_url_in_browser(self, url):
                # Use Flet's method to open the URL
                self.page.launch_url(url)

        print('server box below')

        server_box = ft.TextField(label="Server URL", hint_text='https://nextcloud.myserver.com')

        def auto_close_gpodder_wait_diag(e):
            print('cancel select')
            gpodder_wait_diag.open = False
            self.pr_instance.rm_stack()
            self.page.update()

        def close_gpodder_wait_diag(page):
            print('cancel select')
            gpodder_wait_diag.open = False
            self.pr_instance.rm_stack()
            page.update()

        gpodder_wait_diag_row = ft.Row(
            controls=[
                ft.TextButton("Cancel", on_click=lambda x: (close_gpodder_wait_diag(self.page)))
            ],
            alignment=ft.MainAxisAlignment.END
        )
        gpodder_wait_diag = ft.AlertDialog(
            modal=True,
            title=ft.Text(f"Logging Into Nextcloud!"),
            content=ft.Column(controls=[
                ft.Text(f'Please login to {server_box.value} from your browser.', selectable=True),
                gpodder_wait_diag_row

            ],
                tight=True),
            actions_alignment=ft.MainAxisAlignment.END,
        )

        def on_auth_success(credentials):
            print(credentials)

            # Assigning credentials to self.active_user
            self.active_user.nextcloud_endpoint = credentials['server']
            self.active_user.nextcloud_token = credentials['appPassword']

            # Rest of the code
            self.page.snack_bar = ft.SnackBar(
                ft.Text(f"You Have Authenticated with Nextcloud. Podcast Sync Will Now Begin!"))
            self.page.snack_bar.open = True
            close_gpodder_wait_diag(self.page)
            self.pr_instance.rm_stack()
            self.page.update()

            # Ensure the token is being passed correctly
            print(f'Nextcloud token after auth success: {self.active_user.nextcloud_token}')

            api_functions.functions.call_add_gpodder_settings(
                self.app_api.url,
                self.app_api.headers,
                self.active_user.user_id,
                self.active_user.nextcloud_endpoint,
                self.active_user.nextcloud_token,
            )

            # Optionally call self.active_user.sync_with_nextcloud() if needed

            # self.active_user.sync_with_nextcloud()

        # Code to handle what happens after successful authentication

        def on_auth_click(page, server_value):
            close_gpodder_diag(page)
            time.sleep(0.1)
            page.update()
            self.pr_instance.touch_stack()

            self.page.dialog = gpodder_wait_diag
            gpodder_wait_diag.open = True
            page.update()
            server_url = server_value.value
            auth = NextcloudAuthenticator(server_url, page, lambda creds: on_auth_success(creds))
            auth.start_login()
            # on_auth_success()

        # auth_button = ft.TextButton(text="Sign In", on_click=on_auth_click)
        # Add server_box and auth_button to your Flet UI in the appropriate place

        def close_gpodder_diag(page):
            gpodder_diag.open = False
            self.page.update()

        print('after close')

        gpodder_diag_select_row = ft.Row(
            controls=[
                ft.TextButton("Confirm", on_click=lambda x: (on_auth_click(self.page, server_box))),
                ft.TextButton("Cancel", on_click=lambda x: (close_gpodder_diag(self.page)))
            ],
            alignment=ft.MainAxisAlignment.END
        )
        gpodder_diag = ft.AlertDialog(
            modal=True,
            title=ft.Text(f"Nextcloud Server Name:"),
            content=ft.Column(controls=[
                ft.Text(f'Please enter your nextcloud server name below.', selectable=True),
                # ], tight=True),
                server_box,
                gpodder_diag_select_row
            ],
                tight=True),
            actions_alignment=ft.MainAxisAlignment.END,
        )
        self.page.dialog = gpodder_diag
        gpodder_diag.open = True
        self.page.update()

    def gpodder_sign_out(self, e):
        api_functions.functions.call_remove_gpodder_settings(self.app_api.url, self.app_api.headers, self.active_user.user_id)
        self.page.snack_bar = ft.SnackBar(ft.Text(f"You've been signed out from Gpodder Sync."))
        self.page.snack_bar.open = True
        self.page.update()

    def guest_check(self):
        if self.guest_status_bool:
            self.guest_status = 'enabled'
            self.guest_info_button = ft.ElevatedButton(f'Disable Guest User',
                                                       on_click=self.guest_user_change,
                                                       bgcolor=self.active_user.main_color,
                                                       color=self.active_user.accent_color)
        else:
            self.guest_status = 'disabled'
            self.guest_info_button = ft.ElevatedButton(f'Enable Guest User',
                                                       on_click=self.guest_user_change,
                                                       bgcolor=self.active_user.main_color,
                                                       color=self.active_user.accent_color)

    def self_service_check(self):
        if self.self_service_bool:
            self.self_service_status = 'enabled'
            self.self_service_button = ft.ElevatedButton(f'Disable Self Service User Creation',
                                                         on_click=self.self_service_change,
                                                         bgcolor=self.active_user.main_color,
                                                         color=self.active_user.accent_color)
        else:
            self.self_service_status = 'disabled'
            self.self_service_button = ft.ElevatedButton(f'Enable Self Service User Creation',
                                                         on_click=self.self_service_change,
                                                         bgcolor=self.active_user.main_color,
                                                         color=self.active_user.accent_color)

    def downloads_check(self):
        if self.download_status_bool:
            self.download_info_button = ft.ElevatedButton(f'Disable Podcast Downloads',
                                                          on_click=self.download_option_change,
                                                          bgcolor=self.active_user.main_color,
                                                          color=self.active_user.accent_color)
        else:
            self.download_info_button = ft.ElevatedButton(f'Enable Podcast Downloads',
                                                          on_click=self.download_option_change,
                                                          bgcolor=self.active_user.main_color,
                                                          color=self.active_user.accent_color)

    def mfa_check(self):
        self.mfa_warning = ft.Text(
            'Note: when setting up MFA you have 1 minute to enter the code or it will expire. If it expires just cancel and try again.',
            color=self.active_user.font_color, size=12)

        if self.check_mfa_status:
            self.mfa_text = ft.Text(f'Setup MFA', color=self.active_user.font_color,
                                    size=16)
            self.mfa_button = ft.ElevatedButton(f'Re-Setup MFA for your account',
                                                on_click=self.mfa_option_change,
                                                bgcolor=self.active_user.main_color,
                                                color=self.active_user.accent_color)
            self.mfa_remove_button = ft.ElevatedButton(f'Remove MFA for your account',
                                                       on_click=self.remove_mfa,
                                                       bgcolor=self.active_user.main_color,
                                                       color=self.active_user.accent_color)
            self.mfa_button_row = ft.Row(
                controls=[self.mfa_button, self.mfa_remove_button])
            self.mfa_column = ft.Column(controls=[self.mfa_text, self.mfa_warning, self.mfa_button_row])
        else:
            self.mfa_text = ft.Text(f'Setup MFA', color=self.active_user.font_color,
                                    size=16)
            self.mfa_button = ft.ElevatedButton(f'Setup MFA for your account', on_click=self.setup_mfa,
                                                bgcolor=self.active_user.main_color,
                                                color=self.active_user.accent_color)
            self.mfa_column = ft.Column(controls=[self.mfa_text, self.mfa_warning, self.mfa_button])

        # Update mfa_container content
        self.mfa_container = ft.Container(content=self.mfa_column)
        self.mfa_container.padding = padding.only(left=70, right=50)
        self.mfa_container.content = self.mfa_column
        self.page.update()

    def email_table_load(self):
        server_info = self.email_information['Server_Name'] + ':' + str(
            self.email_information['Server_Port'])
        from_email = self.email_information['From_Email']
        send_mode = self.email_information['Send_Mode']
        encryption = self.email_information['Encryption']
        auth = self.email_information['Auth_Required']

        if auth == 1:
            auth_user = self.email_information['Username']
        else:
            auth_user = 'Auth not defined!'

        # Create a new data row with the user information
        row = ft.DataRow(
            cells=[
                ft.DataCell(ft.Text(server_info)),
                ft.DataCell(ft.Text(from_email)),
                ft.DataCell(ft.Text(send_mode)),
                ft.DataCell(ft.Text(encryption)),
                ft.DataCell(ft.Text(auth_user))
            ]
        )

        # Append the row to the list of data rows
        self.email_table_rows.append(row)

        self.email_table = ft.DataTable(
            bgcolor=self.active_user.main_color,
            border=ft.border.all(2, self.active_user.main_color),
            border_radius=10,
            vertical_lines=ft.border.BorderSide(3, self.active_user.tertiary_color),
            horizontal_lines=ft.border.BorderSide(1, self.active_user.tertiary_color),
            heading_row_color=self.active_user.nav_color1,
            heading_row_height=100,
            data_row_color={"hovered": self.active_user.font_color},
            # show_checkbox_column=True,
            columns=[
                ft.DataColumn(ft.Text("Server Name"), numeric=True),
                ft.DataColumn(ft.Text("From Email")),
                ft.DataColumn(ft.Text("Send Mode")),
                ft.DataColumn(ft.Text("Encryption?")),
                ft.DataColumn(ft.Text("Username"))
            ],
            rows=self.email_table_rows
        )
        pw_reset_current = Text('Existing Email Server Values:', color=self.active_user.font_color, size=16)
        self.email_edit_column = ft.Column(controls=[pw_reset_current, self.email_table])
        self.email_edit_container = ft.Container(content=self.email_edit_column)
        self.email_edit_container.padding = padding.only(left=70, right=50)

    def create_email_table(self):
        return ft.DataTable(
            bgcolor=self.active_user.main_color,
            border=ft.border.all(2, self.active_user.main_color),
            border_radius=10,
            vertical_lines=ft.border.BorderSide(3, self.active_user.tertiary_color),
            horizontal_lines=ft.border.BorderSide(1, self.active_user.tertiary_color),
            heading_row_color=self.active_user.nav_color1,
            heading_row_height=100,
            data_row_color={"hovered": self.active_user.font_color},
            # show_checkbox_column=True,
            columns=[
                ft.DataColumn(ft.Text("Server Name"), numeric=True),
                ft.DataColumn(ft.Text("From Email")),
                ft.DataColumn(ft.Text("Send Mode")),
                ft.DataColumn(ft.Text("Encryption?")),
                ft.DataColumn(ft.Text("Username"))
            ],
            rows=self.email_table_rows
        )

    def email_table_update(self):
        self.email_information = api_functions.functions.call_get_email_info(self.app_api.url, self.app_api.headers)
        self.email_table_rows.clear()
        server_info = self.email_information['Server_Name'] + ':' + str(
            self.email_information['Server_Port'])
        from_email = self.email_information['From_Email']
        send_mode = self.email_information['Send_Mode']
        encryption = self.email_information['Encryption']
        auth = self.email_information['Auth_Required']

        if auth == 1:
            auth_user = self.email_information['Username']
        else:
            auth_user = 'Auth not defined!'

        # Create a new data row with the user information
        row = ft.DataRow(
            cells=[
                ft.DataCell(ft.Text(server_info)),
                ft.DataCell(ft.Text(from_email)),
                ft.DataCell(ft.Text(send_mode)),
                ft.DataCell(ft.Text(encryption)),
                ft.DataCell(ft.Text(auth_user))
            ]
        )
        # Append the row to the list of data rows
        self.email_table_rows.append(row)
        self.email_table = self.create_email_table()
        self.page.update()

    def user_table_load(self):
        edit_user_text = ft.Text('Modify existing Users (Select a user to modify properties):',
                                 color=self.active_user.font_color, size=16)
        user_information = api_functions.functions.call_get_user_info(self.app_api.url, self.app_api.headers)

        for entry in user_information:
            user_id = entry['UserID']
            fullname = entry['Fullname']
            username = entry['Username']
            email = entry['Email']
            is_admin_numeric = entry['IsAdmin']
            if is_admin_numeric == 1:
                is_admin = 'yes'
            else:
                is_admin = 'no'

            # Create a new data row with the user information
            row = ft.DataRow(
                cells=[
                    ft.DataCell(ft.Text(user_id)),
                    ft.DataCell(ft.Text(fullname)),
                    ft.DataCell(ft.Text(username)),
                    ft.DataCell(ft.Text(email)),
                    ft.DataCell(ft.Text(str(is_admin))),
                ],
                on_select_changed=(
                    lambda username_copy, is_admin_numeric_copy, fullname_copy, email_copy,
                           user_id_copy:
                    lambda x: (self.modify_user.open_edit_user(username_copy, is_admin_numeric_copy,
                                                          fullname_copy, email_copy, user_id_copy),
                               self.user_table_update())
                )(username, is_admin_numeric, fullname, email, user_id)
            )

            # Append the row to the list of data rows
            self.user_table_rows.append(row)

        self.user_table = ft.DataTable(
            bgcolor=self.active_user.main_color,
            border=ft.border.all(2, self.active_user.main_color),
            border_radius=10,
            vertical_lines=ft.border.BorderSide(3, self.active_user.tertiary_color),
            horizontal_lines=ft.border.BorderSide(1, self.active_user.tertiary_color),
            heading_row_color=self.active_user.nav_color1,
            heading_row_height=100,
            data_row_color={"hovered": self.active_user.font_color},
            columns=[
                ft.DataColumn(ft.Text("User ID"), numeric=True),
                ft.DataColumn(ft.Text("Fullname")),
                ft.DataColumn(ft.Text("Username")),
                ft.DataColumn(ft.Text("Email")),
                ft.DataColumn(ft.Text("Admin User"))
            ],
            rows=self.user_table_rows
        )
        self.user_edit_column = ft.Column(controls=[edit_user_text, self.user_table])
        self.user_edit_container = ft.Container(content=self.user_edit_column)
        self.user_edit_container.padding = padding.only(left=70, right=50)

    def user_table_update(self):
        user_information = api_functions.functions.call_get_user_info(self.app_api.url, self.app_api.headers)
        self.user_table_rows.clear()

        for entry in user_information:
            user_id = entry['UserID']
            fullname = entry['Fullname']
            username = entry['Username']
            email = entry['Email']
            is_admin_numeric = entry['IsAdmin']
            if is_admin_numeric == 1:
                is_admin = 'yes'
            else:
                is_admin = 'no'

            # Create a new data row with the user information
            row = ft.DataRow(
                cells=[
                    ft.DataCell(ft.Text(user_id)),
                    ft.DataCell(ft.Text(fullname)),
                    ft.DataCell(ft.Text(username)),
                    ft.DataCell(ft.Text(email)),
                    ft.DataCell(ft.Text(str(is_admin))),
                ],
                on_select_changed=(
                    lambda username_copy, is_admin_numeric_copy, fullname_copy, email_copy, user_id_copy:
                    lambda x: (self.modify_user.open_edit_user(username_copy, is_admin_numeric_copy,
                                                          fullname_copy, email_copy, user_id_copy),
                               self.user_table_update())
                )(username, is_admin_numeric, fullname, email, user_id)
            )

            self.user_table_rows.append(row)
        self.user_table = self.create_user_table()
        self.page.update()

    def create_user_table(self):
        return ft.DataTable(
            bgcolor=self.active_user.main_color,
            border=ft.border.all(2, self.active_user.main_color),
            border_radius=10,
            vertical_lines=ft.border.BorderSide(3, self.active_user.tertiary_color),
            horizontal_lines=ft.border.BorderSide(1, self.active_user.tertiary_color),
            heading_row_color=self.active_user.nav_color1,
            heading_row_height=100,
            data_row_color={"hovered": self.active_user.font_color},
            columns=[
                ft.DataColumn(ft.Text("User ID"), numeric=True),
                ft.DataColumn(ft.Text("Fullname")),
                ft.DataColumn(ft.Text("Username")),
                ft.DataColumn(ft.Text("Email")),
                ft.DataColumn(ft.Text("Admin User"))
            ],
            rows=self.user_table_rows
        )

    def import_data(self, e):
        def close_import_dlg(page):
            import_dlg.open = False
            self.page.update()

        def import_user():
            import xml.etree.ElementTree as ET

            def import_pick_result(e: ft.FilePickerResultEvent):
                if e.files:
                    self.active_user.import_file = e.files[0].path
                tree = ET.parse(self.active_user.import_file)
                root = tree.getroot()

                podcasts = []
                for outline in root.findall('.//outline'):
                    podcast_data = {
                        'title': outline.get('title'),
                        'xmlUrl': outline.get('xmlUrl')
                    }
                    podcasts.append(podcast_data)

                self.pr_instance.touch_stack()
                close_import_dlg(self.page)
                self.page.update()
                for podcast in podcasts:

                    if not podcast.get('title') or not podcast.get('xmlUrl'):
                        close_import_dlg(self.page)
                        self.page.snack_bar = ft.SnackBar(
                            content=ft.Text(f"This does not appear to be a valid opml file"))
                        self.page.snack_bar.open = True
                        self.page.update()
                        return False

                    # Get the podcast values
                    podcast_values = internal_functions.functions.get_podcast_values(podcast['xmlUrl'],
                                                                                     self.active_user.user_id)

                    # Call add_podcast for each podcast
                    return_value = api_functions.functions.call_add_podcast(self.app_api.url, self.app_api.headers,
                                                                            podcast_values,
                                                                            self.active_user.user_id)
                    if return_value:
                        self.page.snack_bar = ft.SnackBar(
                            content=ft.Text(f"{podcast_values[0]} Imported!")
                        )
                    else:
                        self.page.snack_bar = ft.SnackBar(
                            content=ft.Text(f"{podcast_values[0]} already added!")
                        )
                    self.page.snack_bar.open = True
                    self.page.update()

                if self.pr_instance.active_pr == True:
                    self.pr_instance.rm_stack()
                self.page.snack_bar = ft.SnackBar(
                    content=ft.Text(
                        f"OPML Successfully imported! You should now be subscribed to podcasts defined in the file!"))
                self.page.snack_bar.open = True
                self.page.update()

                return True

            file_picker = ft.FilePicker(on_result=import_pick_result)
            self.page.overlay.append(file_picker)
            self.page.update()
            file_picker.pick_files()

        def import_server():
            def import_server_result(e: ft.FilePickerResultEvent):
                close_import_dlg(self.page)
                self.page.update()

                if e.files:
                    file_path = e.files[0].path
                    with open(file_path, 'r') as file:
                        file_contents = file.read()

                    def run_full_restore(e):
                        self.pr_instance.touch_stack()
                        close_restore_pass_win(self.page)
                        self.page.update()
                        time.sleep(1)

                        restore_status = api_functions.functions.call_restore_server(self.app_api.url,
                                                                                     self.app_api.headers,
                                                                                     backup_database_pass.value,
                                                                                     file_contents)

                        def start_login(page):
                            page.go("/login")

                        if restore_status.get("success") == True:
                            self.page.snack_bar = ft.SnackBar(
                                content=ft.Text(f"Server Restore Successful! Now logging out!"))
                            self.page.snack_bar.open = True
                            self.pr_instance.rm_stack()
                            self.page.update()
                            time.sleep(1.5)

                            self.active_user = user.User(self.page)
                            self.pr_instance.rm_stack()
                            self.login_username.visible = True
                            self.login_password.visible = True

                            start_login(self.page)
                            self.new_nav.navbar.border = ft.border.only(
                                right=ft.border.BorderSide(2, self.active_user.tertiary_color))
                            self.new_nav.navbar_stack = ft.Stack([self.new_nav.navbar], expand=True)
                            self.page.overlay.append(self.new_nav.navbar_stack)
                            self.new_nav.navbar.visible = False
                            self.page.update()
                        else:
                            error_message = restore_status.get("error_message", "Unknown error.")
                            self.page.snack_bar = ft.SnackBar(
                                content=ft.Text(f"Server Restore failed: {error_message}"))
                            self.page.snack_bar.open = True
                            self.page.update()

                    def close_restore_pass_win(page):
                        close_restore_pass_dlg.open = False
                        self.page.update()

                    backup_pass_text = ft.Text(
                        f"WARNING: You are about to run a full restore on your server! This will remove absolutely everything currently stored in your database and revert to the data that's part of the backup you restore to. If you're unsure what you're doing DO NOT proceed. If you are certain you'd like to restore the database with a previous backup please enter your database root password below.",
                        selectable=True)
                    backup_occur_text = ft.Text(
                        f"After the restore is complete you will be logged out from Pinepods as the restore operation will restore your users to the users included in the backup. Make certain you know the login details to a user that's an admin in the backup you are about to restore to.")

                    backup_select_pass_row = ft.Row(
                        controls=[
                            ft.TextButton("Submit", on_click=run_full_restore),
                            ft.TextButton("Close", on_click=lambda x: (close_restore_pass_win(self.page)))
                        ],
                        alignment=ft.MainAxisAlignment.END
                    )
                    backup_database_pass = ft.TextField(label="Database Password", icon=ft.icons.HANDYMAN,
                                                        hint_text='My_Datab@$$_P@SS', password=True,
                                                        can_reveal_password=True)

                    close_restore_pass_dlg = ft.AlertDialog(
                        modal=True,
                        title=ft.Text(f"Restore Data:"),
                        content=ft.Column(controls=[
                            backup_pass_text,
                            backup_occur_text,
                            backup_database_pass,
                            backup_select_pass_row
                        ],
                            tight=True),
                        actions_alignment=ft.MainAxisAlignment.END,
                    )
                    self.page.dialog = close_restore_pass_dlg
                    close_restore_pass_dlg.open = True
                    self.page.update()

            file_picker = ft.FilePicker(on_result=import_server_result)
            self.page.overlay.append(file_picker)
            self.page.update()
            file_picker.pick_files()

        user_import_select = ft.TextButton("Import OPML of Podcasts", on_click=lambda x: (import_user()))
        server_import_select = ft.TextButton("Import Entire Server Information",
                                             on_click=lambda x: (import_server()))

        if not self.active_user.user_is_admin:
            server_import_select.visible = False

        import_select_row = ft.Row(
            controls=[
                ft.TextButton("Close", on_click=lambda x: (close_import_dlg(self.page)))
            ],
            alignment=ft.MainAxisAlignment.END
        )

        import_dlg = ft.AlertDialog(
            modal=True,
            title=ft.Text(f"Import Data:"),
            content=ft.Column(controls=[
                ft.Text(
                    f'Select an option below to import data.',
                    selectable=True),
                user_import_select,
                server_import_select,
                import_select_row
            ],
                tight=True),
            actions_alignment=ft.MainAxisAlignment.END,
        )
        self.page.dialog = import_dlg
        import_dlg.open = True
        self.page.update()

    def backup_data(self, e):
        def close_backup_dlg(page):
            backup_dlg.open = False
            self.page.update()

        def open_backups():
            import subprocess
            import platform

            def open_folder(path):
                if platform.system() == "Windows":
                    os.startfile(path)
                elif platform.system() == "Darwin":
                    subprocess.Popen(["open", path])
                else:
                    subprocess.Popen(["xdg-open", path])

            open_folder(backup_dir)

        def backup_user():
            backup_status = api_functions.functions.call_backup_user(self.app_api.url, self.app_api.headers,
                                                                     self.active_user.user_id, backup_dir)
            close_backup_dlg(self.page)
            self.page.update()

            def close_backup_status_win(page):
                backup_stat_dlg.open = False
                self.page.update()

            if backup_status == True:
                backup_status_text = ft.Text(f"Backup Successful! File Saved to: {backup_dir}",
                                             selectable=True)
                folder_location = ft.TextButton("Open Backup Location",
                                                on_click=lambda x: (open_backups()))
            else:
                backup_status_text = ft.Text("Backup was not successful. Try again!")
                folder_location = ft.Text("N/A")

            backup_select_status_row = ft.Row(
                controls=[
                    ft.TextButton("Close", on_click=lambda x: (close_backup_status_win(self.page)))
                ],
                alignment=ft.MainAxisAlignment.END
            )

            backup_stat_dlg = ft.AlertDialog(
                modal=True,
                title=ft.Text(f"Backup Data:"),
                content=ft.Column(controls=[
                    backup_status_text,
                    folder_location,
                    backup_select_status_row
                ],
                    tight=True),
                actions_alignment=ft.MainAxisAlignment.END,
            )
            self.page.dialog = backup_stat_dlg
            backup_stat_dlg.open = True
            self.page.update()

        def backup_server():
            close_backup_dlg(self.page)
            self.page.update()

            def run_database_backup(e):
                backup_status = api_functions.functions.call_backup_server(self.app_api.url, self.app_api.headers,
                                                                           backup_dir,
                                                                           backup_database_pass.value)
                close_backup_pass_win(self.page)
                self.page.update()

                def close_backup_status_win(page):
                    backup_stat_dlg.open = False
                    self.page.update()

                if backup_status["success"]:
                    backup_status_text = ft.Text(f"Backup Successful! File Saved to: {backup_dir}",
                                                 selectable=True)
                    folder_location = ft.TextButton("Open Backup Location",
                                                    on_click=lambda x: (open_backups()))
                else:
                    backup_status_text = ft.Text(
                        f"Backup was not successful. Reason: {backup_status['error_message']}")
                    folder_location = ft.Text("N/A")

                backup_select_status_row = ft.Row(
                    controls=[
                        ft.TextButton("Close", on_click=lambda x: (close_backup_status_win(self.page)))
                    ],
                    alignment=ft.MainAxisAlignment.END
                )

                backup_stat_dlg = ft.AlertDialog(
                    modal=True,
                    title=ft.Text(f"Backup Data:"),
                    content=ft.Column(controls=[
                        backup_status_text,
                        folder_location,
                        backup_select_status_row
                    ], tight=True),
                    actions_alignment=ft.MainAxisAlignment.END,
                )
                self.page.dialog = backup_stat_dlg
                backup_stat_dlg.open = True
                self.page.update()

            def close_backup_pass_win(page):
                backup_pass_dlg.open = False
                self.page.update()

            backup_pass_text = ft.Text(
                f"In order to conduct a server wide backup you must provide your database password set during the creation of your Pinepods server. Please enter that below.",
                selectable=True)

            backup_select_pass_row = ft.Row(
                controls=[
                    ft.TextButton("Submit", on_click=run_database_backup),
                    ft.TextButton("Close", on_click=lambda x: (close_backup_pass_win(self.page)))
                ],
                alignment=ft.MainAxisAlignment.END
            )
            backup_database_pass = ft.TextField(label="Database Password", icon=ft.icons.HANDYMAN,
                                                hint_text='My_Datab@$$_P@SS', password=True,
                                                can_reveal_password=True)

            backup_pass_dlg = ft.AlertDialog(
                modal=True,
                title=ft.Text(f"Backup Data:"),
                content=ft.Column(controls=[
                    backup_pass_text,
                    backup_database_pass,
                    backup_select_pass_row
                ],
                    tight=True),
                actions_alignment=ft.MainAxisAlignment.END,
            )
            self.page.dialog = backup_pass_dlg
            backup_pass_dlg.open = True
            self.page.update()

        user_backup_select = ft.TextButton("Export OPML of Podcasts", on_click=lambda x: (backup_user()))
        server_backup_select = ft.TextButton("Backup Entire Server", on_click=lambda x: (backup_server()))

        if not self.active_user.user_is_admin:
            server_backup_select.visible = False

        backup_select_row = ft.Row(
            controls=[
                ft.TextButton("Close", on_click=lambda x: (close_backup_dlg(self.page)))
            ],
            alignment=ft.MainAxisAlignment.END
        )

        backup_dlg = ft.AlertDialog(
            modal=True,
            title=ft.Text(f"Backup Data:"),
            content=ft.Column(controls=[
                ft.Text(
                    f'Select an option below to backup information.',
                    selectable=True),
                user_backup_select,
                server_backup_select,
                backup_select_row
            ],
                tight=True),
            actions_alignment=ft.MainAxisAlignment.END,
        )
        self.page.dialog = backup_dlg
        backup_dlg.open = True
        self.page.update()

    def guest_user_change(self, e):
        api_functions.functions.call_enable_disable_guest(self.app_api.url, self.app_api.headers)
        self.page.snack_bar = ft.SnackBar(content=ft.Text(f"Guest user modified!"))
        self.page.snack_bar.open = True
        self.guest_status_bool = api_functions.functions.call_guest_status(self.app_api.url, self.app_api.headers)
        if self.guest_status_bool:
            self.guest_info_button.text = 'Disable Guest User'
            self.guest_info_button.on_click = self.guest_user_change
            self.guest_status = 'enabled'
        else:
            self.guest_info_button.text = 'Enable Guest User'
            self.guest_info_button.on_click = self.guest_user_change
            self.guest_status = 'disabled'

        self.disable_guest_notify.visible = False
        self.page.update()

    def self_service_change(self, e):
        api_functions.functions.call_enable_disable_self_service(self.app_api.url, self.app_api.headers)
        self.page.snack_bar = ft.SnackBar(content=ft.Text(f"Self Service Settings Adjusted!"))
        self.page.snack_bar.open = True
        self.self_service_bool = api_functions.functions.call_self_service_status(self.app_api.url,
                                                                                  self.app_api.headers)
        if self.self_service_bool:
            self.self_service_button.text = 'Disable Self Service User Creation'
            self.self_service_button.on_click = self.self_service_change
            self.self_service_status = 'enabled'
        else:
            self.self_service_button.text = 'Enable Self Service User Creation'
            self.self_service_button.on_click = self.self_service_change
            self.self_service_status = 'disabled'

        self.self_service_notify.visible = False
        self.page.update()

    def download_option_change(self, e):
        api_functions.functions.call_enable_disable_downloads(self.app_api.url, self.app_api.headers)
        self.page.snack_bar = ft.SnackBar(content=ft.Text(f"Download Option Modified!"))
        self.page.snack_bar.open = True
        self.download_status_bool = api_functions.functions.call_download_status(self.app_api.url,
                                                                                 self.app_api.headers)
        if self.download_status_bool:
            self.download_info_button.text = 'Disable Podcast Server Downloads'
            self.download_info_button.on_click = self.download_option_change
        else:
            self.download_info_button.text = 'Enable Podcast Server Downloads'
            self.download_info_button.on_click = self.download_option_change

        self.disable_download_notify.visible = False
        self.page.update()

    def mfa_option_change(self, e):
        mfa_setup_check = self.setup_mfa()
        if mfa_setup_check == True:
            self.mfa_check()
            self.page.update()
        else:
            self.page.update()
