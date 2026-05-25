use i18nrs::yew::use_translation;
use yew::prelude::*;
use yew_router::history::{BrowserHistory, History};

#[function_component(NotFound)]
pub fn not_found() -> Html {
    let (i18n, _) = use_translation();
    let i18n_page_not_found = i18n.t("not_found.page_not_found").to_string();
    let i18n_uncharted_territory = i18n.t("not_found.uncharted_territory").to_string();
    let i18n_grab_coffee = i18n.t("not_found.grab_coffee").to_string();
    let i18n_head_back_home = i18n.t("not_found.head_back_home").to_string();
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
                    { &i18n_page_not_found }
                </h1>

                <p class="text-lg item_container-text opacity-80">
                    { &i18n_uncharted_territory }
                </p>

                <div class="flex items-center gap-2 text-lg item_container-text opacity-70">
                    <i class="ph ph-coffee-bean text-2xl" />
                    <span>{ &i18n_grab_coffee }</span>
                    <i class="ph ph-coffee text-2xl" />
                </div>

                <button
                    onclick={on_home_click}
                    class="flex items-center gap-2 px-6 py-3 mt-4 rounded-lg transition-all
                        item_container-text border-2 border-current hover:opacity-80
                        active:scale-95 text-lg font-medium"
                >
                    <i class="ph ph-house-line text-xl" />
                    { &i18n_head_back_home }
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
