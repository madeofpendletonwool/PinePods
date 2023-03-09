""" Let's do a little client side UI"""

""" these are the modules needed"""
import flet
from flet import (
    Page,
    Text,
    View,
    Column,
    Container,
    LinearGradient,
    alignment,
    border_radius,
    padding,
    Row,
    Card,
    TextField,
    FilledButton,
    SnackBar,
)

""" We use requests library to send GET/POST requests to our server"""
import requests


def main(page: Page):
    page.title = "Routes Example"

    def GradientGenerator(start, end):
        ColorGradient = LinearGradient(
            begin=alignment.bottom_left,
            end=alignment.top_right,
            colors=[
                start,
                end,
            ],
        )

        return ColorGradient
    
    email = TextField(
    label="Email",
    border="underline",
    width=320,
    text_size=14,
)

    password = TextField(
        label="Password",
        border="underline",
        width=320,
        text_size=14,
        password=True,
        can_reveal_password=True,
    )
    
    
    addlogin = Column(
        alignment="center",
        controls=[
            Card(
                elevation=15,
                content=Container(
                    width=550,
                    height=550,
                    padding=padding.all(30),
                    gradient=GradientGenerator(
                        "#2f2937", "#251867"
                    ),
                    border_radius=border_radius.all(12),
                    content=Column(
                        horizontal_alignment="center",
                        alignment="start",
                        controls=[
                            Text(
                                "Pypods: A podcast app built in python",
                                size=32,
                                weight="w700",
                                text_align="center",
                            ),
                            Text(
                                "Please login with your user account to start listening to podcasts. If you didn't set a default user up please check the docker logs for a default account and credentials",
                                size=14,
                                weight="w700",
                                text_align="center",
                                color="#64748b",
                            ),
                            Container(
                                padding=padding.only(bottom=20)
                            ),
                            email,
                            Container(
                                padding=padding.only(bottom=10)
                            ),
                            password,
                            Container(
                                padding=padding.only(bottom=20)
                            ),
                            Row(
                                alignment="center",
                                spacing=20,
                                controls=[
                                    FilledButton(
                                        content=Text(
                                            "Login",
                                            weight="w700",
                                        ),
                                        width=160,
                                        height=40,
                                        # Now, if we want to login, we also need to send some info back to the server and check if the credentials are correct or if they even exists.
                                        on_click=lambda e: req_login(
                                            e,
                                            email.value,
                                            password.value,
                                        ),
                                    ),
                                    FilledButton(
                                        content=Text(
                                            "Create acount",
                                            weight="w700",
                                        ),
                                        width=160,
                                        height=40,
                                        on_click=lambda __: page.go(
                                            "/register"
                                        ),
                                    ),
                                ],
                            ),
                        ],
                    ),
                ),
            )
        ],
    )

    page.add(addlogin)

# we can now test this using the web browser
# flet.app(target=main, host="localhost", port=9999, view=flet.WEB_BROWSER)
flet.app(target=main, port=8034)