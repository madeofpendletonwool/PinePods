import flet as ft
from math import pi

rotate_pos = False
def main(page):
    def button_clicked(e):
        t.value = (
            f"Switch values are:  {c1.value}, {c2.value}, {c3.value}, {c4.value}."
        )
        page.update()

    t = ft.Text()
    c1 = ft.Switch(label="Unchecked switch", value=False)
    c2 = ft.Switch(label="Checked switch", value=True)
    c3 = ft.Switch(label="Disabled switch", disabled=True)
    c4 = ft.Switch(
        label="Switch with rendered label_position='left'", label_position=ft.LabelPosition.LEFT
    )
    b = ft.ElevatedButton(text="Submit", on_click=button_clicked)

    rotate_button = ft.IconButton(
        icon=ft.icons.ARROW_FORWARD_IOS,
        icon_color="blue400",
        tooltip="Pause record",
        rotate=ft.transform.Rotate(0, alignment=ft.alignment.center),
        animate_rotation=ft.animation.Animation(300, ft.AnimationCurve.BOUNCE_OUT),
    )

    def animate(e):
        global rotate_pos
        if not rotate_pos:
            rotate_pos = True
            rotate_button.rotate.angle += pi / 2
            page.update()
        else:
            rotate_button.rotate.angle -= pi / 2
            rotate_pos = False
            page.update()

    rotate_button.on_click = animate

    rotate_row = ft.Row(
        [rotate_button]
    )

    page.add(c1, c2, c3, c4, b, t, rotate_row
             )

ft.app(target=main)