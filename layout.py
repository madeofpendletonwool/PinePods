import flet as ft
from math import pi

rotate_pos = False
def main(page):

    def button_press(e):
        button.text = "testafter"
        page.update()

    button = ft.ElevatedButton("testing", on_click=button_press)

    hidden_text = ft.Text('test')
    hidden_cont = ft.Container(content=hidden_text,
        offset=ft.transform.Offset(0, -5),
        animate_offset=ft.animation.Animation(100),)
    # hidden_cont.visible = False
    double_cont = ft.Container(content=hidden_cont)
    double_cont.clip_behavior = ft.ClipBehavior.HARD_EDGE

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
            # hidden_cont.visible = True
            rotate_button.rotate.angle += pi / 2
            hidden_cont.offset = ft.transform.Offset(0, 0)
            # hidden_cont.update()
            page.update()
        else:
            # hidden_cont.visible = False
            rotate_button.rotate.angle -= pi / 2
            rotate_pos = False
            hidden_cont.offset = ft.transform.Offset(0, 0)
            page.update()

    rotate_button.on_click = animate

    rotate_row = ft.Row(
        [rotate_button]
    )

    page.add(button, rotate_row, double_cont
             )

ft.app(target=main)