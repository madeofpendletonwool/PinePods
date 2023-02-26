import flet
from flet import *
from functools import partial
import time


class ModernNavBar(UserControl):
    def __init__(self, func):
        self.func = func
        super().__init__()

    def HighlightContainer(self, e):
        if e.data == "true":
            e.control.bgcolor = "white10"
            e.control.update()

            e.control.content.controls[0].icon_color = "white"
            e.control.content.controls[1].color = "white"
            e.control.content.update()
        else:
            e.control.bgcolor = None
            e.control.update()

            e.control.content.controls[0].icon_color = "white54"
            e.control.content.controls[1].color = "white54"
            e.control.content.update()

    def UserData(self, initials: str, name: str, description: str):
        return Container(
            content=Row(
                controls=[
                    Container(
                        width=42,
                        height=42,
                        border_radius=8,
                        bgcolor="bluegrey900",
                        alignment=alignment.center,
                        content=Text(
                            value=initials,
                            size=20,
                            weight="bold",
                        ),
                    ),
                    Column(
                        spacing=1,
                        alignment=MainAxisAlignment.CENTER,
                        controls=[
                            Text(
                                value=name,
                                size=11,
                                weight="bold",
                                opacity=1,
                                animate_opacity=200,
                            ),
                            Text(
                                value=description,
                                size=9,
                                weight="w400",
                                color="white54",
                                opacity=1,
                                animate_opacity=200,
                            ),
                        ],
                    ),
                ]
            )
        )

    def ContainedIcon(self, icon_name, text):
        return Container(
            width=180,
            height=45,
            border_radius=10,
            on_hover=lambda e: self.HighlightContainer(e),
            ink=True,
            content=Row(
                controls=[
                    IconButton(
                        icon=icon_name,
                        icon_size=18,
                        icon_color="white54",
                        selected=False,
                        style=ButtonStyle(
                            shape={
                                "": RoundedRectangleBorder(radius=7),
                            },
                            overlay_color={"": "transparent"},
                        ),
                    ),
                    Text(
                        value=text,
                        color="white54",
                        size=11,
                        opacity=1,
                        animate_opacity=200,
                    ),
                ],
            ),
        )

    def build(self):
        return Container(
            width=200,
            height=580,
            padding=padding.only(top=10),
            alignment=alignment.center,
            content=Column(
                alignment=MainAxisAlignment.CENTER,
                horizontal_alignment="center",
                controls=[
                    self.UserData("LI", "Line Indent", "Softeware Engineer"),
                    Container(
                        width=24,
                        height=24,
                        bgcolor="bluegrey600",
                        border_radius=8,
                        on_click=partial(self.func),
                    ),
                    Divider(height=5, color="transparent"),
                    self.ContainedIcon(icons.SEARCH, "Search"),
                    self.ContainedIcon(icons.DASHBOARD_ROUNDED, "Dashboard"),
                    self.ContainedIcon(icons.BAR_CHART, "Revenue"),
                    self.ContainedIcon(icons.NOTIFICATIONS, "Notifications"),
                    self.ContainedIcon(icons.PIE_CHART_ROUNDED, "Analytics"),
                    self.ContainedIcon(icons.FAVORITE_ROUNDED, "Likes"),
                    self.ContainedIcon(icons.WALLET_ROUNDED, "Wallet"),
                    Divider(color="white24", height=5),
                    self.ContainedIcon(icons.LOGOUT_ROUNDED, "Logout"),
                ],
            ),
        )


def main(page: Page):
    page.title = "Flet Modern Sidebar"
    page.horizontal_alignment = "end"
    page.vertical_alignment = "center"

    def AnimateNavBar(e):
        print(page.controls[0])
        print(type(page.controls[0]))
        print(type(page.controls[0].content))
        print(type(page.controls[0].content.controls[0]))
        print(type(page.controls[0].content.controls[0].content))
        print(page.controls[0]
                .content.controls[0]
                .content.controls[0]
                .content.controls[1])
        print(page.controls[0]
                .content.controls[0]
                .content.controls[0]
                .content.controls[1]
                .controls[:])
        if page.controls[0].width != 62:
            for item in (
                page.controls[0]
                .content.controls[0]
                .content.controls[0]
                .content.controls[1]
                .controls[:]
            ):
                item.opacity = 0
                item.update()

            for item in page.controls[0].content.controls[0].content.controls[3:]:
                if isinstance(item, Container):

                    item.content.controls[1].opacity = 0
                    item.content.update()

            time.sleep(0.2)

            page.controls[0].width = 62
            page.controls[0].update()

        else:
            page.controls[0].width = 200
            page.controls[0].update()

            time.sleep(0.2)

            for item in (
                page.controls[0]
                .content.controls[0]
                .content.controls[0]
                .content.controls[1]
                .controls[:]
            ):
                item.opacity = 1
                item.update()

            for item in page.controls[0].content.controls[0].content.controls[3:]:
                if isinstance(item, Container):

                    item.content.controls[1].opacity = 1
                    item.content.update()

    page.add(
        Container(
            width=200,
            height=580,
            animate=animation.Animation(500, "decelerate"),
            bgcolor="black",
            border_radius=10,
            padding=10,
            content=ModernNavBar(AnimateNavBar),
        )
    )

    page.bgcolor = "deeppurple200"
    page.padding = padding.only(right=120)

    page.update()


if __name__ == "__main__":
    flet.app(target=main)