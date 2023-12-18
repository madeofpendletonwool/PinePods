use yew::prelude::*;
use web_sys::HtmlInputElement;
use web_sys::console;


#[function_component(App_drawer)]
pub fn app_drawer() -> Html {
    let selection = use_state(|| "".to_string());

    let is_drawer_open = use_state(|| false);

    let toggle_drawer = {
        let is_drawer_open = is_drawer_open.clone();
        move |_| {
            is_drawer_open.set(!*is_drawer_open);
        }
    };

    let on_selection_change = {
        let selection = selection.clone();
        Callback::from(move |e: InputEvent| {
            selection.set(e.target_unchecked_into::<web_sys::HtmlInputElement>().value());
        })
    };

    // let on_select = {
    //     let selection = selection.clone();
    //     Callback::from(move |_| {
    //         // Handle the login logic here
    //         // For example, send the username and password to a server
    //         let message = format!("Selected: {}", *selection);
    //         console::log_1(&message.into());
    //     })
    // };

    html! {
    <div class="relative min-h-screen">
        // Drawer
        <div class={classes!("fixed", "drawer-background", "top-0", "left-0", "z-20", "h-full", "transition-all", "duration-500", "transform", "shadow-lg", "md:w-64", "w-full", (*is_drawer_open).then(|| "translate-x-0").unwrap_or("-translate-x-full"))}>
            <div class="px-6 py-4 mt-16">
                <h2 class="text-lg font-semibold">{"Drawer"}</h2>
                <p class="text-gray-500">{"This is a drawer."}</p>
            </div>
        </div>


        // Toggle button - Fixed Position
        <div class="fixed top-0 left-0 z-30 p-4">
            <label for="drawer-toggle" class="bg-indigo-500 rounded-lg cursor-pointer">
                <div class="flex flex-col items-center">
                    <div class="w-6 h-1 mb-1 bg-white rounded-lg"></div>
                    <div class="w-6 h-1 mb-1 bg-white rounded-lg"></div>
                    <div class="w-6 h-1 bg-white rounded-lg"></div>
                </div>
            </label>
        </div>

        <input
            type="checkbox"
            id="drawer-toggle"
            class="sr-only"
            checked={*is_drawer_open}
            onclick={toggle_drawer.clone()}
        />
    </div>
    }

}