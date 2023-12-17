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
from helpers import navigation

# Other Imports
import os
import appdirs
import re
import base64
import shutil


class User:
    email_regex = r"^[a-zA-Z0-9_.+-]+@[a-zA-Z0-9-]+\.[a-zA-Z0-9-.]+$"

    def __init__(self, page, app_api, nav, login_screen, pod_controls):
        self.valid_username = None
        self.pod_controls = pod_controls
        self.login_screen = login_screen
        self.nav = nav
        self.from_email = None
        self.server_port = None
        self.server_name = None
        self.app_api = app_api
        self.username = None
        self.password = None
        self.email = None
        self.main_color = 'colors.BLUE_GREY'
        self.bgcolor = 'colors.BLUE_GREY'
        self.accent_color = 'colors.BLUE_GREY'
        self.tertiary_color = 'colors.BLUE_GREY'
        self.font_color = 'colors.BLUE_GREY'
        self.user_id = None
        self.page = page
        self.pr_instance = navigation.PR(self.page)
        self.fullname = 'Login First'
        self.isadmin = None
        self.navbar_stack = None
        self.new_user_valid = False
        self.invalid_value = False
        self.api_id = 0
        self.mfa_secret = None
        self.downloading = []
        self.downloading_name = []
        self.auth_enabled = 0
        self.timezone = 'UTC'
        self.hour_pref = 24
        self.first_login_finished = 0
        self.first_start = 0
        self.search_term = ""
        self.feed_url = None
        self.import_file = None
        self.user_is_admin = None
        self.show_audio_container = True
        # global current_pod_view
        self.current_pod_view = None  # This global variable will hold the current active Pod_View instance
        self.retain_session = ft.Switch(label="Stay Signed in", value=False)
        self.nextcloud_endpoint = None
        self.nexcloud_token = None

    # New User Stuff ----------------------------

    def speed_login(self, e):
        self.login(self.login_username, self.login_password, self.retain_session.value)


    # ---Flet Various Elements----------------------------------------------------------------
    def close_dlg(self, e):
        self.user_dlg.open = False
        self.page.update()
        navigation.go_home

    def close_invalid_dlg(self, e):
        self.username_invalid_dlg.open = False
        self.password_invalid_dlg.open = False
        self.email_invalid_dlg.open = False
        self.username_exists_dlg.open = False
        self.page.update()
        # Define User Creation Dialog

    user_dlg = ft.AlertDialog(
        modal=True,
        title=ft.Text("New User Created!"),
        content=ft.Text("You can now log in as this user"),
        actions=[
            ft.TextButton("Okay", on_click=close_dlg),
        ],
        actions_alignment=ft.MainAxisAlignment.END,
        on_dismiss=lambda e: navigation.go_home
    )
    username_invalid_dlg = ft.AlertDialog(
        modal=True,
        title=ft.Text("Username Invalid!"),
        content=ft.Text("Usernames must be unique and require at least 3 characters!"),
        actions=[
            ft.TextButton("Okay", on_click=close_invalid_dlg),
        ],
        actions_alignment=ft.MainAxisAlignment.END
    )
    password_invalid_dlg = ft.AlertDialog(
        modal=True,
        title=ft.Text("Password Invalid!"),
        content=ft.Text("Passwords require at least 8 characters, a number, a capital letter and a special character!"),
        actions=[
            ft.TextButton("Okay", on_click=close_invalid_dlg),
        ],
        actions_alignment=ft.MainAxisAlignment.END
    )
    email_invalid_dlg = ft.AlertDialog(
        modal=True,
        title=ft.Text("Invalid Email!"),
        content=ft.Text("Email appears to be non-standard email layout!"),
        actions=[
            ft.TextButton("Okay", on_click=close_invalid_dlg),
        ],
        actions_alignment=ft.MainAxisAlignment.END
    )
    username_exists_dlg = ft.AlertDialog(
        modal=True,
        title=ft.Text("Username already exists"),
        content=ft.Text("This username is already in use. Please try another."),
        actions=[
            ft.TextButton("Okay", on_click=close_invalid_dlg),
        ],
        actions_alignment=ft.MainAxisAlignment.END
    )

    login_username = ft.TextField(
        label="Username",
        border="underline",
        width=320,
        text_size=14,
        on_submit=speed_login
    )

    login_password = ft.TextField(
        label="Password",
        border="underline",
        width=320,
        text_size=14,
        password=True,
        can_reveal_password=True,
        on_submit=speed_login
    )

    server_name = ft.TextField(
        label="Server Name",
        border="underline",
        hint_text="ex. https://api.pinepods.online",
        width=320,
        text_size=14,
    )

    app_api_key = ft.TextField(
        label="API Key",
        border="underline",
        width=320,
        text_size=14,
        hint_text='Generate this from settings in PinePods',
        password=True,
        can_reveal_password=True,
    )

    mfa_prompt = ft.TextField(
        label="MFA code",
        border="underline",
        hint_text="ex. 123456",
        width=320,
        text_size=14,
    )


    def set_username(self, new_username):
        if new_username is None or not new_username.strip():
            self.username = None
        else:
            self.username = new_username

    def set_password(self, new_password):
        if new_password is None or not new_password.strip():
            self.password = None
        else:
            self.password = new_password

    def set_email(self, new_email):
        if new_email is None or not new_email.strip():
            self.email = None
        else:
            self.email = new_email

    def set_name(self, new_name):
        if new_name is None or not new_name.strip():
            self.fullname = None
        else:
            self.fullname = new_name

    def set_admin(self, new_admin):
        self.isadmin = new_admin

    def verify_user_values(self):
        self.valid_username = self.username is not None and len(self.username) >= 3
        self.valid_password = self.password is not None and len(self.password) >= 8 and any(
            c.isupper() for c in self.password) and any(c.isdigit() for c in self.password)
        regex = r"^[a-zA-Z0-9_.+-]+@[a-zA-Z0-9-]+\.[a-zA-Z0-9-.]+$"
        self.valid_email = self.email is not None and re.match(self.email_regex, self.email) is not None
        invalid_value = False
        if not self.valid_username:
            self.page.dialog = self.username_invalid_dlg
            self.username_invalid_dlg.open = True
            self.page.update()
            invalid_value = True
        elif not self.valid_password:
            self.page.dialog = self.password_invalid_dlg
            self.password_invalid_dlg.open = True
            self.page.update()
            invalid_value = True
        elif not self.valid_email:
            self.page.dialog = self.email_invalid_dlg
            self.email_invalid_dlg.open = True
            self.page.update()
            invalid_value = True
        elif api_functions.functions.call_check_usernames(self.app_api.url, self.app_api.headers, self.username):
            self.page.dialog = self.username_exists_dlg
            self.username_exists_dlg.open = True
            self.page.update()
            invalid_value = True
        self.new_user_valid = not invalid_value

    def verify_user_values_snack(self):
        self.valid_username = self.username is not None and len(self.username) >= 6
        self.valid_password = self.password is not None and len(self.password) >= 8 and any(
            c.isupper() for c in self.password) and any(c.isdigit() for c in self.password)
        regex = r"^[a-zA-Z0-9_.+-]+@[a-zA-Z0-9-]+\.[a-zA-Z0-9-.]+$"
        self.valid_email = self.email is not None and re.match(self.email_regex, self.email) is not None
        invalid_value = False
        if not self.valid_username:
            self.page.snack_bar = ft.SnackBar(
                content=ft.Text(f"Usernames must be unique and require at least 6 characters"))
            self.page.snack_bar.open = True
            self.page.update()
            self.invalid_value = True
        elif not self.valid_password:
            self.page.snack_bar = ft.SnackBar(content=ft.Text(
                f"Passwords require at least 8 characters, a number, a capital letter and a special character!"))
            self.page.snack_bar.open = True
            self.page.update()
            self.invalid_value = True
        elif not self.valid_email:
            self.page.snack_bar = ft.SnackBar(content=ft.Text(f"Email appears to be non-standard email layout!"))
            self.page.snack_bar.open = True
            self.page.update()
            self.invalid_value = True
        elif api_functions.functions.call_check_usernames(self.app_api.url, self.app_api.headers, self.username):
            self.page.snack_bar = ft.SnackBar(content=ft.Text(f"This username appears to be already taken"))
            self.page.snack_bar.open = True
            self.page.update()
            self.invalid_value = True
        if self.invalid_value:
            self.new_user_valid = False
        else:
            self.new_user_valid = not invalid_value

    def user_created_prompt(self):
        if self.new_user_valid:
            self.page.dialog = self.user_dlg
            self.user_dlg.open = True
            self.page.update()

    def user_created_snack(self):
        if self.new_user_valid:
            self.page.snack_bar = ft.SnackBar(content=ft.Text(
                f"New user created successfully. You may now login and begin using Pinepods. Enjoy!"))
            self.page.snack_bar.open = True
            self.page.update()

    def popup_user_values(self, e):
        pass

    def create_user(self):
        if self.new_user_valid:
            salt, hash_pw = Auth.Passfunctions.hash_password(self.password)
            hash_pw_str = base64.b64encode(hash_pw).decode()
            salt_str = base64.b64encode(salt).decode()
            api_functions.functions.call_add_user(self.app_api.url, self.app_api.headers, self.fullname, self.username,
                                                  self.email, hash_pw_str, salt_str)

    def test_email_settings(self, server_name, server_port, from_email, send_mode, encryption, auth_required,
                            username=None, password=None):
        def close_email_dlg(e):
            send_email_dlg.open = False
            self.page.update()

        self.pr_instance.touch_stack()
        self.page.update()

        def save_email_settings(e):
            encryption_key = api_functions.functions.call_get_encryption_key(self.app_api.url, self.app_api.headers)
            encryption_key_bytes = base64.b64decode(encryption_key)
            api_functions.functions.call_save_email_settings(
                self.app_api.url,
                self.app_api.headers,
                self.server_name,
                self.server_port,
                self.from_email,
                self.send_mode,
                self.encryption,
                self.auth_required,
                self.email_username,
                self.email_password,
                encryption_key_bytes
            )
            send_email_dlg.open = False
            self.page.update()

        self.server_name = server_name
        self.server_port = int(server_port)
        self.from_email = from_email
        self.send_mode = send_mode
        self.encryption = encryption
        self.auth_required = auth_required
        self.email_username = username
        self.email_password = password

        subject = "Test email from pinepods"
        body = "If you got this your email settings are working! Great Job! Don't forget to hit save."
        to_email = self.email
        email_result = app_functions.functions.send_email(server_name, server_port, from_email, to_email, send_mode,
                                                          encryption, auth_required, username, password, subject,
                                                          body)

        self.pr_instance.rm_stack()
        send_email_dlg = ft.AlertDialog(
            modal=True,
            title=ft.Text(f"Email Send Test"),
            content=ft.Column(controls=[
                ft.Text(f"Test email send result: {email_result}", selectable=True),
                ft.Text(
                    f'If the email sent successfully be sure to hit save. This will save your settings to the database for later use with resetting passwords.',
                    selectable=True),
            ], tight=True),
            actions=[
                ft.TextButton("Save", on_click=save_email_settings),
                ft.TextButton("Close", on_click=close_email_dlg)
            ],
            actions_alignment=ft.MainAxisAlignment.END
        )
        self.page.dialog = send_email_dlg
        send_email_dlg.open = True
        self.page.update()

    def adjust_email_settings(self, server_name, server_port, from_email, send_mode, encryption, auth_required,
                              username, password):
        self.server_name = server_name
        self.server_port = server_port
        self.from_email = from_email
        self.send_mode = send_mode
        self.encryption = encryption
        self.auth_required = auth_required
        self.email_username = username
        self.email_password = password
        api_functions.functions.call_save_email_settings(self.app_api.url, self.app_api.headers, self.server_name,
                                                         self.server_port, self.from_email, self.send_mode,
                                                         self.encryption, self.auth_required, self.email_username,
                                                         self.email_password)

    # Modify User Stuff---------------------------
    def open_edit_user(self, username, admin, fullname, email, user_id):
        def close_modify_dlg():
            modify_user_dlg.open = False
            self.page.update()

        def close_modify_dlg_auto(e):
            modify_user_dlg.open = False
            self.page.update()

        if username == 'guest':
            modify_user_dlg = ft.AlertDialog(
                modal=True,
                title=ft.Text(f"Guest user cannot be changed"),
                actions=[
                    ft.TextButton("Cancel", on_click=close_modify_dlg_auto)
                ],
                actions_alignment=ft.MainAxisAlignment.END
            )
            self.page.dialog = modify_user_dlg
            modify_user_dlg.open = True
            self.page.update()
        else:
            self.user_id = user_id
            if admin == 1:
                admin_box = True
            else:
                admin_box = False

            self.username = username
            user_modify_name = ft.TextField(label="Full Name", icon=ft.icons.CARD_MEMBERSHIP,
                                            hint_text='John PinePods')
            user_modify_email = ft.TextField(label="Email", icon=ft.icons.EMAIL,
                                             hint_text='ilovepinepods@pinepods.com')
            user_modify_username = ft.TextField(label="Username", icon=ft.icons.PERSON,
                                                hint_text='pinepods_user1999')
            user_modify_password = ft.TextField(label="Password", icon=ft.icons.PASSWORD, password=True,
                                                can_reveal_password=True, hint_text='mY_SuPeR_S3CrEt!')
            user_modify_admin = ft.Checkbox(label="Set User as Admin", value=admin_box)
            modify_user_dlg = ft.AlertDialog(
                modal=True,
                title=ft.Text(f"Modify User: {self.username}"),
                content=ft.Column(controls=[
                    user_modify_name,
                    user_modify_email,
                    user_modify_username,
                    user_modify_password,
                    user_modify_admin
                ], tight=True),
                actions=[
                    ft.TextButton(content=ft.Text("Delete User", color=ft.colors.RED_400), on_click=lambda x: (
                        close_modify_dlg(),
                        self.page.update(),
                        self.delete_user(user_id)
                    )),
                    ft.TextButton("Confirm Changes", on_click=lambda x: (
                        self.set_username(user_modify_username.value),
                        self.set_password(user_modify_password.value),
                        self.set_email(user_modify_email.value),
                        self.set_name(user_modify_name.value),
                        self.set_admin(user_modify_admin.value),
                        self.change_user_attributes(),
                        close_modify_dlg()
                    )),

                    ft.TextButton("Cancel", on_click=close_modify_dlg_auto)
                ],
                actions_alignment=ft.MainAxisAlignment.SPACE_EVENLY
            )
            self.page.dialog = modify_user_dlg
            modify_user_dlg.open = True
            self.page.update()

    def change_user_attributes(self):
        if self.fullname is not None:
            api_functions.functions.call_set_fullname(self.app_api.url, self.app_api.headers, self.user_id, self.fullname)

        if self.password is not None:
            if len(self.password) < 8 or not any(c.isupper() for c in self.password) or not any(
                    c.isdigit() for c in self.password):
                self.page.snack_bar = ft.SnackBar(
                    content=ft.Text(f"Passwords must contain a number, a capital letter and a special character"))
                self.page.snack_bar.open = True
                self.page.update()
            else:
                salt, hash_pw = Auth.Passfunctions.hash_password(self.password)
                api_functions.functions.call_set_password(self.app_api.url, self.app_api.headers, self.user_id, salt, hash_pw)

        if self.email is not None:
            if not re.match(self.email_regex, self.email):
                self.page.snack_bar = ft.SnackBar(
                    content=ft.Text(f"This does not appear to be a properly formatted email"))
                self.page.snack_bar.open = True
                self.page.update()
            else:
                api_functions.functions.call_set_email(self.app_api.url, self.app_api.headers, self.user_id, self.email)

        if self.username is not None:
            if len(self.username) < 6:
                self.page.snack_bar = ft.SnackBar(content=ft.Text(f"Username must be at least 6 characters"))
                self.page.snack_bar.open = True
                self.page.update()
            else:
                api_functions.functions.call_set_username(self.app_api.url, self.app_api.headers, self.user_id, self.username)

        api_functions.functions.call_set_isadmin(self.app_api.url, self.app_api.headers, self.user_id, self.isadmin)
        user_changed = True

        if user_changed == True:
            self.page.snack_bar = ft.SnackBar(content=ft.Text(f"User Changed!"))
            self.page.snack_bar.open = True
            self.page.update()

    def delete_user(self, user_id):
        admin_check = api_functions.functions.call_final_admin(self.app_api.url, self.app_api.headers, user_id)

        def show_snack_bar(message):
            if self.page.snack_bar:
                self.page.remove(self.page.snack_bar)

            self.page.snack_bar = ft.SnackBar(content=ft.Text(message))
            self.page.snack_bar.open = True

        if user_id == self.user_id:
            show_snack_bar("Cannot delete your own user")
        elif admin_check:
            show_snack_bar("Cannot delete the final admin user")
        else:
            # Confirmation dialog
            dlg_modal = ft.AlertDialog(
                modal=True,
                title=ft.Text("Please confirm"),
                content=ft.Text("Do you really want to delete User?"),
                actions=[
                    ft.TextButton("Yes", on_click=lambda e: perform_delete(user_id)),
                    ft.TextButton("No", on_click=lambda e: close_dialog()),
                ],
                actions_alignment=ft.MainAxisAlignment.END,
                on_dismiss=lambda e: print("Modal dialog dismissed!"),
            )

            # Show the confirmation dialog
            self.page.dialog = dlg_modal
            dlg_modal.open = True
            self.page.update()

        def close_dialog():
            # Close the confirmation dialog
            self.page.dialog.open = False
            self.page.update()

        def perform_delete(user_id):

            api_functions.functions.call_delete_user(self.app_api.url, self.app_api.headers, user_id)
            self.page.snack_bar = ft.SnackBar(content=ft.Text(f"User Deleted!"))
            self.page.snack_bar.open = True
            self.page.update()

            close_dialog()

    # Active User Stuff --------------------------

    def setup_timezone(self, tz, hour_pref):
        if hour_pref == '12-hour':
            self.hour_pref = 12
        else:
            self.hour_pref = 24
        self.timezone = tz
        api_functions.functions.call_setup_time_info(self.app_api.url, self.app_api.headers, self.user_id, self.timezone,
                                                     self.hour_pref)
        if self.user_id == 1:
            self.nav.new_nav.navbar.border = ft.border.only(right=ft.border.BorderSide(2, self.tertiary_color))
            self.nav.new_nav.navbar_stack = ft.Stack([self.nav.new_nav.navbar], expand=True)
            self.page.overlay.append(self.nav.new_nav.navbar_stack)
            self.page.update()
            self.page.go("/")
        else:
            navigation.go_homelogin(self.page, self)

    def get_timezone(self):
        self.timezone, self.hour_pref = api_functions.functions.call_get_time_info(self.app_api.url, self.app_api.headers,
                                                                                   self.user_id)

    def first_login_done(self):
        self.first_login_finished = api_functions.functions.call_first_login_done(self.app_api.url, self.app_api.headers,
                                                                                  self.user_id)

    def get_initials(self):
        # split the full name into separate words
        words = self.fullname.split()

        # extract the first letter of each word and combine them
        initials_lower = "".join(word[0] for word in words)

        # return the initials as uppercase
        self.initials = initials_lower.upper()

    def on_click_wronguser(self, page):
        self.page.snack_bar = ft.SnackBar(ft.Text(f"Wrong username or password. Please try again!"))
        self.page.snack_bar.open = True
        self.page.update()

    def on_click_novalues(self, page):
        self.page.snack_bar = ft.SnackBar(ft.Text(f"Please enter a username and a password before selecting Login"))
        self.page.snack_bar.open = True
        self.page.update()

    def login(self, username_field, password_field, retain_session):
        username = username_field.value
        password = password_field.value
        username_field.value = ''
        password_field.value = ''
        username_field.update()
        password_field.update()
        if not username or not password:
            self.on_click_novalues(self.page)
            return
        pass_correct = api_functions.functions.call_verify_password(self.app_api.url, self.app_api.headers, username,
                                                                    password)
        if pass_correct == True:
            login_details = api_functions.functions.call_get_user_details(self.app_api.url, self.app_api.headers, username)
            self.user_id = login_details['UserID']
            self.fullname = login_details['Fullname']
            self.username = login_details['Username']
            self.email = login_details['Email']

            check_mfa_status = api_functions.functions.call_check_mfa_enabled(self.app_api.url, self.app_api.headers,
                                                                              self.user_id)
            if check_mfa_status:
                self.retain_session = retain_session
                navigation.open_mfa_login(self.page)

            else:
                if retain_session:
                    session_token = api_functions.functions.call_create_session(self.app_api.url, self.app_api.headers,
                                                                                self.user_id)
                    if session_token:
                        self.app_api.save_session_id_to_file(session_token)
                self.first_login_done()
                if self.first_login_finished == 1:
                    self.get_timezone()
                    navigation.go_homelogin(self.page, self)
                else:
                    navigation.first_time_config(self.page)
        else:
            self.on_click_wronguser(self.page)

    def mfa_login(self, mfa_prompt):
        mfa_secret = mfa_prompt.value

        mfa_verify = api_functions.functions.call_verify_mfa(self.app_api.url, self.app_api.headers, self.user_id, mfa_secret)

        if mfa_verify:
            if self.retain_session:
                session_token = api_functions.functions.call_create_session(self.app_api.url, self.app_api.headers,
                                                                            self.user_id)
                if session_token:
                    self.app_api.save_session_id_to_file(session_token)
            self.first_login_done()
            if self.first_login_finished == 1:
                self.get_timezone()
                navigation.go_homelogin(self.page, self)
            else:
                navigation.first_time_config(self.page)
        else:
            self.page.snack_bar = ft.SnackBar(content=ft.Text(f"MFA Code incorrect"))
            self.page.snack_bar.open = True
            self.page.update()

    def saved_login(self, user_id):
        login_details = api_functions.functions.call_get_user_details_id(self.app_api.url, self.app_api.headers, user_id)
        self.user_id = login_details['UserID']
        self.fullname = login_details['Fullname']
        self.username = login_details['Username']
        self.email = login_details['Email']
        self.first_login_done()
        if self.first_login_finished == 1:
            self.get_timezone()
            navigation.go_homelogin(self.page, self)
        else:
            navigation.first_time_config(self.page)

    def logout_pinepods(self, e):
        active_user = User(self.page)
        self.pr_instance.rm_stack()
        self.login_username.visible = True
        self.login_password.visible = True
        if self.login_screen:

            navigation.start_login(self.page)
            self.nav.new_nav.navbar.border = ft.border.only(right=ft.border.BorderSide(2, active_user.tertiary_color))
            self.nav.new_nav.navbar_stack = ft.Stack([self.nav.new_nav.navbar], expand=True)
            self.page.overlay.append(self.nav.new_nav.navbar_stack)
            self.nav.new_nav.navbar.visible = False
            self.page.update()
        else:
            active_user.user_id = 1
            active_user.fullname = 'Guest User'
            self.nav.go_homelogin(self.page, self)

    def logout_pinepods_clear_local(self, e):
        active_user = User(self.page, self.app_api, self.nav, self.login_screen, self.pod_controls)
        self.pr_instance.rm_stack()
        self.login_username.visible = True
        self.login_password.visible = True
        if self.login_screen:
            app_name = 'pinepods'
            data_dir = appdirs.user_data_dir(app_name)
            for filename in os.listdir(data_dir):
                file_path = os.path.join(data_dir, filename)
                try:
                    if os.path.isfile(file_path) or os.path.islink(file_path):
                        os.unlink(file_path)
                    elif os.path.isdir(file_path):
                        shutil.rmtree(file_path)
                except Exception as e:
                    print(f'Failed to delete {file_path}. Reason: {e}')

            navigation.start_config(self.page)
        else:
            active_user.user_id = 1
            active_user.fullname = 'Guest User'
            navigation.go_homelogin(self.page, self)

    def clear_guest(self, e):
        if self.user_id == 1:
            api_functions.functions.call_clear_guest_data(self.app_api.url, self.app_api.headers)

    # Setup Theming-------------------------------------------------------
    def theme_select(self):
        active_theme = api_functions.functions.call_get_theme(self.app_api.url, self.app_api.headers, self.user_id)
        if active_theme == 'light':
            self.page.theme_mode = "light"
            self.main_color = '#E1E1E1'
            self.accent_color = colors.BLACK
            self.tertiary_color = '#C7C7C7'
            self.font_color = colors.BLACK
            self.bonus_color = colors.BLACK
            self.nav_color1 = colors.BLACK
            self.nav_color2 = colors.BLACK
            self.bgcolor = '#ECECEC'
            self.page.bgcolor = '#3C4252'
            self.page.window_bgcolor = '#ECECEC'
        elif active_theme == 'dark':
            self.page.theme_mode = "dark"
            self.main_color = '#010409'
            self.accent_color = '#8B949E'
            self.tertiary_color = '#8B949E'
            self.font_color = '#F5F5F5'
            self.bonus_color = colors.BLACK
            self.nav_color1 = colors.BLACK
            self.nav_color2 = colors.BLACK
            self.bgcolor = '#0D1117'
            self.page.bgcolor = '#3C4252'
            self.page.window_bgcolor = '#3C4252'
        elif active_theme == 'nordic':
            self.page.theme_mode = "dark"
            self.main_color = '#323542'
            self.accent_color = colors.WHITE
            self.tertiary_color = colors.WHITE
            self.font_color = colors.WHITE
            self.bonus_color = colors.BLACK
            self.nav_color1 = colors.BLACK
            self.nav_color2 = colors.BLACK
            self.bgcolor = '#3C4252'
            self.page.bgcolor = '#3C4252'
            self.page.window_bgcolor = '#3C4252'
        elif active_theme == 'abyss':
            self.page.theme_mode = "dark"
            self.main_color = '#051336'
            self.accent_color = '#FFFFFF'
            self.tertiary_color = '#13326A'
            self.font_color = '#42A5F5'
            self.bonus_color = colors.BLACK
            self.nav_color1 = colors.BLACK
            self.nav_color2 = colors.WHITE
            self.bgcolor = '#000C18'
            self.page.bgcolor = '#3C4252'
            self.page.window_bgcolor = '#3C4252'
        elif active_theme == 'dracula':
            self.page.theme_mode = "dark"
            self.main_color = '#262626'
            self.accent_color = '#5196B2'
            self.tertiary_color = '#5196B2'
            self.font_color = colors.WHITE
            self.bonus_color = '#D5BC5C'
            self.nav_color1 = '#D5BC5C'
            self.nav_color2 = colors.BLACK
            self.bgcolor = '#282A36'
            self.page.bgcolor = '#282A36'
            self.page.window_bgcolor = '#3C4252'
        elif active_theme == 'kimbie':
            self.page.theme_mode = "dark"
            self.main_color = '#362712'
            self.accent_color = '#B23958'
            self.tertiary_color = '#AC8E2F'
            self.font_color = '#B1AD86'
            self.bonus_color = '#221A1F'
            self.nav_color1 = '#221A1F'
            self.nav_color2 = '#B1AD86'
            self.bgcolor = '#221A0F'
            self.page.bgcolor = '#282A36'
            self.page.window_bgcolor = '#3C4252'
        elif active_theme == 'hotdogstand - MY EYES':
            self.page.theme_mode = "dark"
            self.main_color = '#EEB911'
            self.accent_color = '#C3590D'
            self.tertiary_color = '#730B1B'
            self.font_color = colors.WHITE
            self.bonus_color = '#D5BC5C'
            self.nav_color1 = '#D5BC5C'
            self.nav_color2 = colors.BLACK
            self.bgcolor = '#E31836'
            self.page.bgcolor = '#282A36'
            self.page.window_bgcolor = '#3C4252'
        elif active_theme == 'neon':
            self.page.theme_mode = "dark"
            self.main_color = '#161C26'
            self.accent_color = '#7000FF'
            self.tertiary_color = '#5196B2'
            self.font_color = '#9F9DA1'
            self.bonus_color = '##01FFF4'
            self.nav_color1 = '#FF1178'
            self.nav_color2 = '#3544BD'
            self.bgcolor = '#120E16'
            self.page.bgcolor = '#282A36'
            self.page.window_bgcolor = '#3C4252'
        elif active_theme == 'wildberries':
            self.page.theme_mode = "dark"
            self.main_color = '#19002E'
            self.accent_color = '#F55385'
            self.tertiary_color = '#5196B2'
            self.font_color = '#CF8B3E'
            self.bonus_color = '#C79BFF'
            self.nav_color1 = '#00FFB7'
            self.nav_color2 = '#44433A'
            self.bgcolor = '#282A36'
            self.page.bgcolor = '#240041'
            self.page.window_bgcolor = '#3C4252'
        elif active_theme == 'greenie meanie':
            self.page.theme_mode = "dark"
            self.main_color = '#292A2E'
            self.accent_color = '#737373'
            self.tertiary_color = '#489D50'
            self.font_color = '#489D50'
            self.bonus_color = '#849CA0'
            self.nav_color1 = '#446448'
            self.nav_color2 = '#43603D'
            self.bgcolor = '#1E1F21'
            self.page.bgcolor = '#3C4252'
            self.page.window_bgcolor = '#3C4252'

    def set_theme(self, theme, navbar):
        api_functions.functions.call_set_theme(self.app_api.url, self.app_api.headers, self.user_id, theme)
        self.theme_select()
        navigation.go_theme_rebuild(self.page, self, self.pod_controls, navbar)
        self.page.update()
