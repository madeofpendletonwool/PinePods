use crate::components::context::{AppState, NotificationState};
use crate::components::gen_funcs::format_error_message;
use crate::requests::setting_reqs::{call_create_custom_theme, CreateCustomThemeRequest};
use i18nrs::yew::use_translation;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use yewdux::prelude::*;

struct ColorField {
    label: &'static str,
    key: &'static str,
}

const COLOR_FIELDS: &[ColorField] = &[
    ColorField { label: "Background",           key: "background_color" },
    ColorField { label: "Secondary Background", key: "secondary_background" },
    ColorField { label: "Container Background", key: "container_background" },
    ColorField { label: "Button",               key: "button_color" },
    ColorField { label: "Container Button",     key: "container_button_color" },
    ColorField { label: "Button Text",          key: "button_text_color" },
    ColorField { label: "Text",                 key: "text_color" },
    ColorField { label: "Secondary Text",       key: "text_secondary_color" },
    ColorField { label: "Border",               key: "border_color" },
    ColorField { label: "Accent",               key: "accent_color" },
    ColorField { label: "Progress Bar",         key: "prog_bar_color" },
    ColorField { label: "Standout",             key: "standout_color" },
    ColorField { label: "Hover",                key: "hover_color" },
    ColorField { label: "Link",                 key: "link_color" },
    ColorField { label: "Thumb / Scrollbar",    key: "thumb_color" },
    ColorField { label: "Unfilled",             key: "unfilled_color" },
    ColorField { label: "Checkbox",             key: "check_box_color" },
    ColorField { label: "Bonus",                key: "bonus_color" },
    ColorField { label: "Error",                key: "error_color" },
];

// Nordic defaults
const DEFAULTS: &[(&str, &str)] = &[
    ("background_color",      "#3C4252"),
    ("secondary_background",  "#2e3440"),
    ("container_background",  "#2b2f3a"),
    ("button_color",          "#3e4555"),
    ("container_button_color","#3C4252"),
    ("button_text_color",     "#f6f5f4"),
    ("text_color",            "#f6f5f4"),
    ("text_secondary_color",  "#f6f5f4"),
    ("border_color",          "#000000"),
    ("accent_color",          "#6d747f"),
    ("prog_bar_color",        "#3550af"),
    ("standout_color",        "#6e8e92"),
    ("hover_color",           "#5d80aa"),
    ("link_color",            "#5d80aa"),
    ("thumb_color",           "#3550af"),
    ("unfilled_color",        "#d4d6d7"),
    ("check_box_color",       "#ffffff"),
    ("bonus_color",           "#000000"),
    ("error_color",           "#ff0000"),
];

fn default_for(key: &str) -> &'static str {
    DEFAULTS.iter().find(|(k, _)| *k == key).map(|(_, v)| *v).unwrap_or("#000000")
}

#[derive(Properties, PartialEq)]
pub struct CustomThemeCreatorProps {
    pub on_created: Callback<()>,
}

#[function_component(CustomThemeCreator)]
pub fn custom_theme_creator(props: &CustomThemeCreatorProps) -> Html {
    let (i18n, _) = use_translation();
    let i18n_preview = i18n.t("custom_theme_creator.preview").to_string();
    let i18n_my_theme = i18n.t("custom_theme_creator.my_theme").to_string();
    let i18n_theme_name = i18n.t("custom_theme_creator.theme_name").to_string();
    let i18n_create_theme = i18n.t("custom_theme_creator.create_theme").to_string();
    let (state, dispatch) = use_store::<AppState>();

    let theme_name = use_state(|| "".to_string());
    let saving = use_state(|| false);

    // One state handle per color field
    let background_color      = use_state(|| default_for("background_color").to_string());
    let secondary_background  = use_state(|| default_for("secondary_background").to_string());
    let container_background  = use_state(|| default_for("container_background").to_string());
    let button_color          = use_state(|| default_for("button_color").to_string());
    let container_button_color= use_state(|| default_for("container_button_color").to_string());
    let button_text_color     = use_state(|| default_for("button_text_color").to_string());
    let text_color            = use_state(|| default_for("text_color").to_string());
    let text_secondary_color  = use_state(|| default_for("text_secondary_color").to_string());
    let border_color          = use_state(|| default_for("border_color").to_string());
    let accent_color          = use_state(|| default_for("accent_color").to_string());
    let prog_bar_color        = use_state(|| default_for("prog_bar_color").to_string());
    let standout_color        = use_state(|| default_for("standout_color").to_string());
    let hover_color           = use_state(|| default_for("hover_color").to_string());
    let link_color            = use_state(|| default_for("link_color").to_string());
    let thumb_color           = use_state(|| default_for("thumb_color").to_string());
    let unfilled_color        = use_state(|| default_for("unfilled_color").to_string());
    let check_box_color       = use_state(|| default_for("check_box_color").to_string());
    let bonus_color           = use_state(|| default_for("bonus_color").to_string());
    let error_color           = use_state(|| default_for("error_color").to_string());

    let get_color = |key: &str| -> String {
        match key {
            "background_color"       => (*background_color).clone(),
            "secondary_background"   => (*secondary_background).clone(),
            "container_background"   => (*container_background).clone(),
            "button_color"           => (*button_color).clone(),
            "container_button_color" => (*container_button_color).clone(),
            "button_text_color"      => (*button_text_color).clone(),
            "text_color"             => (*text_color).clone(),
            "text_secondary_color"   => (*text_secondary_color).clone(),
            "border_color"           => (*border_color).clone(),
            "accent_color"           => (*accent_color).clone(),
            "prog_bar_color"         => (*prog_bar_color).clone(),
            "standout_color"         => (*standout_color).clone(),
            "hover_color"            => (*hover_color).clone(),
            "link_color"             => (*link_color).clone(),
            "thumb_color"            => (*thumb_color).clone(),
            "unfilled_color"         => (*unfilled_color).clone(),
            "check_box_color"        => (*check_box_color).clone(),
            "bonus_color"            => (*bonus_color).clone(),
            "error_color"            => (*error_color).clone(),
            _ => "#000000".to_string(),
        }
    };

    let make_color_input = |key: &'static str, label: &'static str| {
        let bg_h = background_color.clone();
        let sec_bg_h = secondary_background.clone();
        let con_bg_h = container_background.clone();
        let btn_h = button_color.clone();
        let con_btn_h = container_button_color.clone();
        let btn_txt_h = button_text_color.clone();
        let txt_h = text_color.clone();
        let txt_sec_h = text_secondary_color.clone();
        let brd_h = border_color.clone();
        let acc_h = accent_color.clone();
        let prg_h = prog_bar_color.clone();
        let std_h = standout_color.clone();
        let hov_h = hover_color.clone();
        let lnk_h = link_color.clone();
        let thm_h = thumb_color.clone();
        let unf_h = unfilled_color.clone();
        let chk_h = check_box_color.clone();
        let bon_h = bonus_color.clone();
        let err_h = error_color.clone();

        let current_val = match key {
            "background_color"       => (*bg_h).clone(),
            "secondary_background"   => (*sec_bg_h).clone(),
            "container_background"   => (*con_bg_h).clone(),
            "button_color"           => (*btn_h).clone(),
            "container_button_color" => (*con_btn_h).clone(),
            "button_text_color"      => (*btn_txt_h).clone(),
            "text_color"             => (*txt_h).clone(),
            "text_secondary_color"   => (*txt_sec_h).clone(),
            "border_color"           => (*brd_h).clone(),
            "accent_color"           => (*acc_h).clone(),
            "prog_bar_color"         => (*prg_h).clone(),
            "standout_color"         => (*std_h).clone(),
            "hover_color"            => (*hov_h).clone(),
            "link_color"             => (*lnk_h).clone(),
            "thumb_color"            => (*thm_h).clone(),
            "unfilled_color"         => (*unf_h).clone(),
            "check_box_color"        => (*chk_h).clone(),
            "bonus_color"            => (*bon_h).clone(),
            "error_color"            => (*err_h).clone(),
            _ => "#000000".to_string(),
        };

        let on_change = Callback::from(move |e: Event| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            let val = input.value();
            match key {
                "background_color"       => bg_h.set(val),
                "secondary_background"   => sec_bg_h.set(val),
                "container_background"   => con_bg_h.set(val),
                "button_color"           => btn_h.set(val),
                "container_button_color" => con_btn_h.set(val),
                "button_text_color"      => btn_txt_h.set(val),
                "text_color"             => txt_h.set(val),
                "text_secondary_color"   => txt_sec_h.set(val),
                "border_color"           => brd_h.set(val),
                "accent_color"           => acc_h.set(val),
                "prog_bar_color"         => prg_h.set(val),
                "standout_color"         => std_h.set(val),
                "hover_color"            => hov_h.set(val),
                "link_color"             => lnk_h.set(val),
                "thumb_color"            => thm_h.set(val),
                "unfilled_color"         => unf_h.set(val),
                "check_box_color"        => chk_h.set(val),
                "bonus_color"            => bon_h.set(val),
                "error_color"            => err_h.set(val),
                _ => {}
            }
        });

        html! {
            <div class="custom-theme-color-row">
                <label class="custom-theme-color-label">{label}</label>
                <div class="custom-theme-color-input-wrap">
                    <input
                        type="color"
                        value={current_val.clone()}
                        onchange={on_change}
                        class="custom-theme-color-picker"
                    />
                    <span class="custom-theme-color-hex">{current_val}</span>
                </div>
            </div>
        }
    };

    let on_name_input = {
        let theme_name = theme_name.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            theme_name.set(input.value());
        })
    };

    let on_submit = {
        let theme_name = theme_name.clone();
        let saving = saving.clone();
        let on_created = props.on_created.clone();
        let dispatch = dispatch.clone();
        let state = state.clone();

        let bg = (*background_color).clone();
        let sec_bg = (*secondary_background).clone();
        let con_bg = (*container_background).clone();
        let btn = (*button_color).clone();
        let con_btn = (*container_button_color).clone();
        let btn_txt = (*button_text_color).clone();
        let txt = (*text_color).clone();
        let txt_sec = (*text_secondary_color).clone();
        let brd = (*border_color).clone();
        let acc = (*accent_color).clone();
        let prg = (*prog_bar_color).clone();
        let std = (*standout_color).clone();
        let hov = (*hover_color).clone();
        let lnk = (*link_color).clone();
        let thm = (*thumb_color).clone();
        let unf = (*unfilled_color).clone();
        let chk = (*check_box_color).clone();
        let bon = (*bonus_color).clone();
        let err = (*error_color).clone();

        // Reset handles for after submit
        let bg_h = background_color.clone();
        let sec_bg_h = secondary_background.clone();
        let con_bg_h = container_background.clone();
        let btn_h = button_color.clone();
        let con_btn_h = container_button_color.clone();
        let btn_txt_h = button_text_color.clone();
        let txt_h = text_color.clone();
        let txt_sec_h = text_secondary_color.clone();
        let brd_h = border_color.clone();
        let acc_h = accent_color.clone();
        let prg_h = prog_bar_color.clone();
        let std_h = standout_color.clone();
        let hov_h = hover_color.clone();
        let lnk_h = link_color.clone();
        let thm_h = thumb_color.clone();
        let unf_h = unfilled_color.clone();
        let chk_h = check_box_color.clone();
        let bon_h = bonus_color.clone();
        let err_h = error_color.clone();

        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            let name = (*theme_name).trim().to_string();
            if name.is_empty() {
                return;
            }

            if let (Some(api_key), Some(user_id), Some(server_name)) = (
                state.auth_details.as_ref().and_then(|d| d.api_key.clone()),
                state.user_details.as_ref().map(|d| d.UserID),
                state.auth_details.as_ref().map(|d| d.server_name.clone()),
            ) {
                saving.set(true);
                let saving = saving.clone();
                let theme_name = theme_name.clone();
                let on_created = on_created.clone();
                let dispatch = dispatch.clone();

                let req = CreateCustomThemeRequest {
                    user_id,
                    name: name.clone(),
                    background_color: bg.clone(),
                    secondary_background: sec_bg.clone(),
                    container_background: con_bg.clone(),
                    button_color: btn.clone(),
                    container_button_color: con_btn.clone(),
                    button_text_color: btn_txt.clone(),
                    text_color: txt.clone(),
                    text_secondary_color: txt_sec.clone(),
                    border_color: brd.clone(),
                    accent_color: acc.clone(),
                    prog_bar_color: prg.clone(),
                    standout_color: std.clone(),
                    hover_color: hov.clone(),
                    link_color: lnk.clone(),
                    thumb_color: thm.clone(),
                    unfilled_color: unf.clone(),
                    check_box_color: chk.clone(),
                    bonus_color: bon.clone(),
                    error_color: err.clone(),
                };

                let bg_h = bg_h.clone();
                let sec_bg_h = sec_bg_h.clone();
                let con_bg_h = con_bg_h.clone();
                let btn_h = btn_h.clone();
                let con_btn_h = con_btn_h.clone();
                let btn_txt_h = btn_txt_h.clone();
                let txt_h = txt_h.clone();
                let txt_sec_h = txt_sec_h.clone();
                let brd_h = brd_h.clone();
                let acc_h = acc_h.clone();
                let prg_h = prg_h.clone();
                let std_h = std_h.clone();
                let hov_h = hov_h.clone();
                let lnk_h = lnk_h.clone();
                let thm_h = thm_h.clone();
                let unf_h = unf_h.clone();
                let chk_h = chk_h.clone();
                let bon_h = bon_h.clone();
                let err_h = err_h.clone();

                spawn_local(async move {
                    match call_create_custom_theme(&server_name, &api_key, &req).await {
                        Ok(_) => {
                            // Reset form
                            theme_name.set("".to_string());
                            bg_h.set(default_for("background_color").to_string());
                            sec_bg_h.set(default_for("secondary_background").to_string());
                            con_bg_h.set(default_for("container_background").to_string());
                            btn_h.set(default_for("button_color").to_string());
                            con_btn_h.set(default_for("container_button_color").to_string());
                            btn_txt_h.set(default_for("button_text_color").to_string());
                            txt_h.set(default_for("text_color").to_string());
                            txt_sec_h.set(default_for("text_secondary_color").to_string());
                            brd_h.set(default_for("border_color").to_string());
                            acc_h.set(default_for("accent_color").to_string());
                            prg_h.set(default_for("prog_bar_color").to_string());
                            std_h.set(default_for("standout_color").to_string());
                            hov_h.set(default_for("hover_color").to_string());
                            lnk_h.set(default_for("link_color").to_string());
                            thm_h.set(default_for("thumb_color").to_string());
                            unf_h.set(default_for("unfilled_color").to_string());
                            chk_h.set(default_for("check_box_color").to_string());
                            bon_h.set(default_for("bonus_color").to_string());
                            err_h.set(default_for("error_color").to_string());
                            saving.set(false);
                            on_created.emit(());
                        }
                        Err(e) => {
                            let formatted = format_error_message(&e.to_string());
                            Dispatch::<NotificationState>::global().reduce_mut(|s| {
                                s.error_message =
                                    Some(format!("Failed to create theme: {}", formatted));
                            });
                            saving.set(false);
                        }
                    }
                });
            }
        })
    };

    let preview_bg = get_color("background_color");
    let preview_text = get_color("text_color");
    let preview_swatch1 = get_color("prog_bar_color");
    let preview_swatch2 = get_color("standout_color");
    let name_val = (*theme_name).clone();
    let is_disabled = *saving || (*theme_name).trim().is_empty();

    html! {
        <div class="custom-theme-creator p-6 space-y-6">
            // Live preview card
            <div class="custom-theme-preview-wrap">
                <span class="custom-theme-preview-label">{ &i18n_preview }</span>
                <div class="custom-theme-preview-card" style={format!(
                    "background-color:{};border-radius:10px;padding:12px;min-height:74px;box-shadow:0 1px 4px rgba(0,0,0,.25);display:inline-block;min-width:160px;",
                    preview_bg
                )}>
                    <div style={format!("color:{};font-size:13px;font-weight:600;line-height:1.2;margin-bottom:6px;", preview_text)}>
                        if name_val.is_empty() { { &i18n_my_theme } } else { {name_val.clone()} }
                    </div>
                    <div style="display:flex;gap:4px;">
                        <span style={format!("display:inline-block;width:18px;height:18px;border-radius:4px;background-color:{};", preview_swatch1)}></span>
                        <span style={format!("display:inline-block;width:18px;height:18px;border-radius:4px;background-color:{};opacity:0.6;", preview_swatch2)}></span>
                    </div>
                </div>
            </div>

            <form onsubmit={on_submit} class="space-y-4">
                // Theme name input
                <div class="custom-theme-name-row">
                    <label class="custom-theme-name-label">{ &i18n_theme_name }</label>
                    <input
                        type="text"
                        placeholder="My Custom Theme"
                        value={(*theme_name).clone()}
                        oninput={on_name_input}
                        class="custom-theme-name-input"
                        maxlength="255"
                    />
                </div>

                // Color pickers grid
                <div class="custom-theme-colors-grid">
                    { COLOR_FIELDS.iter().map(|f| make_color_input(f.key, f.label)).collect::<Html>() }
                </div>

                <button
                    type="submit"
                    class="custom-theme-submit-btn"
                    disabled={is_disabled}
                >
                    if *saving {
                        <span class="animate-spin inline-block mr-2">{"⟳"}</span>
                        {"Saving..."}
                    } else {
                        <i class="ph ph-plus-circle mr-1"></i>
                        { &i18n_create_theme }
                    }
                </button>
            </form>
        </div>
    }
}
