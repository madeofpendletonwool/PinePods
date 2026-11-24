//! The sign-in backdrop shared by the web, change-server and desktop login pages: a forest
//! drifting forest photo with fog, the brand column and the footer controls.

use crate::components::i18n::use_translation;
use yew::prelude::*;

const SCENES: u32 = 9;

fn scene_url(scene: u32) -> String {
    format!("static/assets/backgrounds/{}.webp", scene)
}

#[derive(Properties, PartialEq)]
pub struct LoginShellProps {
    pub children: Html,
    /// Speeds up the logo's signal rings while a sign-in request is in flight.
    #[prop_or_default]
    pub tuning: bool,
    /// The server being signed in to, shown under the wordmark.
    #[prop_or_default]
    pub server: Option<String>,
    /// Extra controls for the bottom-right corner.
    #[prop_or_default]
    pub corner: Html,
}

#[function_component(LoginShell)]
pub fn login_shell(props: &LoginShellProps) -> Html {
    let (i18n, _) = use_translation();
    // (current scene, scene fading out beneath it)
    let scene = use_state(|| (rand::random_range(1..=SCENES), 0u32));

    let next_scene = {
        let scene = scene.clone();
        Callback::from(move |_: MouseEvent| {
            let (current, _) = *scene;
            scene.set((current % SCENES + 1, current));
        })
    };

    let (current, previous) = *scene;

    html! {
        <div class={classes!("lp-root", props.tuning.then_some("is-tuning"))}>
            if previous != 0 {
                <div class="lp-photo" style={format!("background-image:url('{}')", scene_url(previous))}></div>
            }
            <div key={current} class="lp-photo lp-photo-in" style={format!("background-image:url('{}')", scene_url(current))}></div>
            <div class="lp-scrim"></div>
            <div class="lp-fog lp-fog-a"></div>
            <div class="lp-fog lp-fog-b"></div>

            <div class="lp-stage">
                <section class="lp-hero">
                    <LoginSignal />
                    <p class="lp-eyebrow">{ i18n.t("login.eyebrow") }</p>
                    <h1 class="lp-wordmark">{"PinePods"}</h1>
                    <p class="lp-tagline">{ i18n.t("login.tagline") }</p>
                    if let Some(server) = props.server.as_ref().filter(|s| !s.is_empty()) {
                        <p class="lp-server">
                            <span class="lp-live-dot"></span>
                            { i18n.t("login.tuned_to") }
                            <strong>{ server.clone() }</strong>
                        </p>
                    }
                </section>
                <div class="lp-card-wrap">
                    { props.children.clone() }
                </div>
            </div>

            <footer class="lp-footer">
                <button type="button" class="lp-scene" onclick={next_scene} title={i18n.t("login.next_scene")}>
                    <span class="lp-eq" aria-hidden="true"><i></i><i></i><i></i><i></i></span>
                    <span class="lp-scene-text">
                        <span class="lp-scene-label">{ i18n.t("login.now_playing") }</span>
                        <span class="lp-scene-name">{ i18n.t(&format!("login.scene_{}", current)) }</span>
                    </span>
                    <i class="ph ph-skip-forward lp-scene-skip" aria-hidden="true"></i>
                </button>
                <div class="lp-corner">{ props.corner.clone() }</div>
            </footer>
        </div>
    }
}

/// The logo with broadcast rings pulsing out of it.
#[function_component(LoginSignal)]
pub fn login_signal() -> Html {
    html! {
        <div class="lp-signal" aria-hidden="true">
            <span class="lp-ring"></span>
            <span class="lp-ring"></span>
            <span class="lp-ring"></span>
            <img src="static/assets/favicon.png" alt="" />
        </div>
    }
}

/// Logo, wordmark and a heading for the card; the logo and wordmark only show on narrow
/// screens, where the hero column is hidden.
#[derive(Properties, PartialEq)]
pub struct CardHeadProps {
    pub title: AttrValue,
    #[prop_or_default]
    pub subtitle: Option<AttrValue>,
    /// Shows a back arrow that returns to the main sign-in view.
    #[prop_or_default]
    pub on_back: Option<Callback<MouseEvent>>,
}

#[function_component(CardHead)]
pub fn card_head(props: &CardHeadProps) -> Html {
    let (i18n, _) = use_translation();
    html! {
        <header class="lp-card-head">
            if let Some(on_back) = props.on_back.clone() {
                <button type="button" class="lp-back" onclick={on_back} aria-label={i18n.t("common.back")}>
                    <i class="ph ph-arrow-left"></i>
                </button>
            } else {
                <div class="lp-card-brand">
                    <LoginSignal />
                    <span class="lp-card-wordmark">{"PinePods"}</span>
                </div>
            }
            <h2 class="lp-card-title">{ props.title.clone() }</h2>
            if let Some(subtitle) = props.subtitle.clone() {
                <p class="lp-card-sub">{ subtitle }</p>
            }
        </header>
    }
}

#[derive(Properties, PartialEq)]
pub struct LoginFieldProps {
    pub id: AttrValue,
    pub label: AttrValue,
    pub icon: AttrValue,
    pub oninput: Callback<InputEvent>,
    #[prop_or(AttrValue::Static("text"))]
    pub kind: AttrValue,
    #[prop_or_default]
    pub autocomplete: Option<AttrValue>,
    #[prop_or_default]
    pub inputmode: Option<AttrValue>,
    #[prop_or_default]
    pub onkeypress: Option<Callback<KeyboardEvent>>,
    #[prop_or_default]
    pub autofocus: bool,
    #[prop_or_default]
    pub error: Option<AttrValue>,
}

/// A text field with an icon and a label that floats up once it has focus or a value.
/// Password fields get a show/hide toggle.
#[function_component(LoginField)]
pub fn login_field(props: &LoginFieldProps) -> Html {
    let (i18n, _) = use_translation();
    let revealed = use_state(|| false);
    let is_password = props.kind == "password";
    let input_type = if is_password && *revealed {
        AttrValue::Static("text")
    } else {
        props.kind.clone()
    };
    let toggle = {
        let revealed = revealed.clone();
        Callback::from(move |_: MouseEvent| revealed.set(!*revealed))
    };
    html! {
        <div class={classes!("lp-field", props.error.is_some().then_some("has-error"))}>
            <i class={classes!("ph", props.icon.to_string(), "lp-field-icon")} aria-hidden="true"></i>
            <input
                id={props.id.clone()}
                name={props.id.clone()}
                type={input_type}
                placeholder=" "
                autocomplete={props.autocomplete.clone()}
                inputmode={props.inputmode.clone()}
                autofocus={props.autofocus}
                oninput={props.oninput.clone()}
                onkeypress={props.onkeypress.clone()}
                spellcheck="false"
                autocapitalize="off"
            />
            <label for={props.id.clone()}>{ props.label.clone() }</label>
            if is_password {
                <button
                    type="button"
                    class="lp-reveal"
                    onclick={toggle}
                    aria-label={if *revealed { i18n.t("login.hide_password") } else { i18n.t("login.show_password") }}
                >
                    <i class={classes!("ph", if *revealed { "ph-eye-slash" } else { "ph-eye" })}></i>
                </button>
            }
            if let Some(error) = props.error.clone() {
                <p class="lp-field-error">{ error }</p>
            }
        </div>
    }
}

#[derive(Properties, PartialEq)]
pub struct PlayButtonProps {
    pub label: AttrValue,
    #[prop_or_default]
    pub busy: bool,
    #[prop_or_default]
    pub onclick: Option<Callback<MouseEvent>>,
    #[prop_or(AttrValue::Static("ph-play"))]
    pub icon: AttrValue,
}

/// The primary action: a pill with a round "play" badge, which turns into bouncing bars
/// while busy.
#[function_component(PlayButton)]
pub fn play_button(props: &PlayButtonProps) -> Html {
    html! {
        <button type="submit" class={classes!("lp-primary", props.busy.then_some("is-busy"))} onclick={props.onclick.clone()} disabled={props.busy}>
            <span class="lp-primary-badge" aria-hidden="true">
                if props.busy {
                    <span class="lp-eq"><i></i><i></i><i></i><i></i></span>
                } else {
                    <i class={classes!("ph", props.icon.to_string())}></i>
                }
            </span>
            <span class="lp-primary-label">{ props.label.clone() }</span>
        </button>
    }
}
