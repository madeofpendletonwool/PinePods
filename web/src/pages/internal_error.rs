use i18nrs::yew::use_translation;
use yew::prelude::*;
use yew_router::history::{BrowserHistory, History};

#[derive(Properties, PartialEq)]
pub struct InternalErrorProps {
    #[prop_or_default]
    pub message: String,
}

#[function_component(InternalError)]
pub fn internal_error(props: &InternalErrorProps) -> Html {
    let (i18n, _) = use_translation();
    let i18n_you_broke_pinepods = i18n.t("internal_error.you_broke_pinepods").to_string();
    let i18n_clever_quip = i18n.t("internal_error.clever_quip").to_string();
    let i18n_head_back_home = i18n.t("internal_error.head_back_home").to_string();
    let on_home_click = Callback::from(|e: MouseEvent| {
        e.prevent_default();
        let history = BrowserHistory::new();
        history.push("/home");
    });
    html! {
        <div class="flex flex-col items-center justify-center min-h-screen p-8">
            <div class="flex flex-col items-center text-center max-w-md space-y-6">
                <div class="flex items-center gap-4 mb-4">
                    <i class="ph ph-heart-break text-8xl item_container-text opacity-80" />
                    <span class="text-8xl font-bold item_container-text opacity-80">{"500"}</span>
                </div>

                <h1 class="text-3xl font-bold item_container-text">
                    { &i18n_you_broke_pinepods }
                </h1>

                <p class="text-lg item_container-text opacity-80">
                    { &i18n_clever_quip }
                </p>

                {
                    if !props.message.is_empty() {
                        html! {
                            { props.message.clone() }

                        }

                    } else {
                        html! { }
                    }
                }

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
