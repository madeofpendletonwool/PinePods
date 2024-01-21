use yew::prelude::*;

#[function_component(MFAOptions)]
pub fn mfa_options() -> Html {
    html! {
        <div class="p-4"> // You can adjust the padding as needed
            <p class="text-lg font-bold mb-4">{"MFA Options:"}</p> // Styled paragraph
            <p class="text-md mb-4">{"You can setup edit, or remove MFA for your account here. MFA will only be prompted when new authentication is needed."}</p> // Styled paragraph

            <button class="mt-4 bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline" type="button">
                {"Setup MFA"}
            </button>
        </div>
    }
}

