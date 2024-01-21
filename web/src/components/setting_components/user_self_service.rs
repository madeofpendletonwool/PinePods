use yew::prelude::*;

#[function_component(SelfServiceSettings)]
pub fn self_service_settings() -> Html {
    html! {
        <div class="p-4"> // You can adjust the padding as needed
            <p class="text-lg font-bold mb-4">{"User Self Service Settings:"}</p> // Styled paragraph
            <p class="text-md mb-4">{"You can enable or disable user self service setup here. That is as it sounds. Once enabled there's a button on the login screen that allows users to set themselves up. It's highly recommended that if you enable this option you disable server downloads and setup the email settings so users can do self service password resets. If you'd rather not enable this you can just set new users up manually using User Settings above."}</p> // Styled paragraph

            <button class="mt-4 bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline" type="button">
                {"Enable User Self Service"}
            </button>
        </div>
    }
}

