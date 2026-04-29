use yew::prelude::*;
use yew_router::history::{BrowserHistory, History};

#[function_component(NotFound)]
pub fn not_found() -> Html {
    let on_home_click = Callback::from(|e: MouseEvent| {
        e.prevent_default();
        let history = BrowserHistory::new();
        history.push("/home");
    });
    html! {
        <div class="flex flex-col items-center justify-center min-h-screen p-8">
            <div class="flex flex-col items-center text-center max-w-md space-y-6">
                <div class="flex items-center gap-4 mb-4">
                    <i class="ph ph-warning-circle text-8xl item_container-text opacity-80" />
                    <span class="text-8xl font-bold item_container-text opacity-80">{"404"}</span>
                </div>

                <h1 class="text-3xl font-bold item_container-text">
                    {"Page Not Found"}
                </h1>

                <p class="text-lg item_container-text opacity-80">
                    {"Looks like we've wandered into uncharted territory!"}
                </p>

                <div class="flex items-center gap-2 text-lg item_container-text opacity-70">
                    <i class="ph ph-coffee-bean text-2xl" />
                    <span>{"Grab some coffee and try again"}</span>
                    <i class="ph ph-coffee text-2xl" />
                </div>

                <button
                    onclick={on_home_click}
                    class="flex items-center gap-2 px-6 py-3 mt-4 rounded-lg transition-all
                        item_container-text border-2 border-current hover:opacity-80
                        active:scale-95 text-lg font-medium"
                >
                    <i class="ph ph-house-line text-xl" />
                    {"Head back home"}
                </button>

                <img
                    src="static/assets/favicon.png"
                    alt="Pinepods Logo"
                    class="w-16 h-16 mt-8 opacity-60"
                />
            </div>
        </div>
    }
}
