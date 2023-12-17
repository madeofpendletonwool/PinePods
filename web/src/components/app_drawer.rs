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
        <div class="flex justify-center items-center h-screen">
            <div class="flex">
                <input
                    type="checkbox"
                    id="drawer-toggle"
                    class="relative sr-only peer"
                    checked={*is_drawer_open}
                    onclick={toggle_drawer}
                />
                <label for="drawer-toggle" class="absolute top-0 left-0 inline-block p-4 transition-all duration-500 bg-indigo-500 rounded-lg peer-checked:rotate-180 peer-checked:left-64">
                    <div class="w-6 h-1 mb-3 -rotate-45 bg-white rounded-lg"></div>
                    <div class="w-6 h-1 rotate-45 bg-white rounded-lg"></div>
                </label>
                <div class="fixed top-0 left-0 z-20 w-64 h-full transition-all duration-500 transform -translate-x-full bg-white shadow-lg peer-checked:translate-x-0">
                    <div class="px-6 py-4">
                        <h2 class="text-lg font-semibold">{"Drawer"}</h2>
                        <p class="text-gray-500">{"This is a drawer."}</p>
                    </div>
                </div>
            </div>
        </div>
    }

}