use yew::prelude::*;

#[function_component(SelfServiceSettings)]
pub fn self_service_settings() -> Html {
    html! {
        <div class="p-4"> // You can adjust the padding as needed
            <p class="text-lg font-bold mb-4">{"User Self Service Settings:"}</p> // Styled paragraph
            <p class="text-md mb-4">{"You can enable or disable user self service setup here. That is as it sounds. Once enabled there's a button on the login screen that allows users to set themselves up. It's highly recommended that if you enable this option you disable server downloads and setup the email settings so users can do self service password resets. If you'd rather not enable this you can just set new users up manually using User Settings above."}</p> // Styled paragraph

            <label class="relative inline-flex items-center cursor-pointer">
                <input type="checkbox" value="" class="sr-only peer" />
                <div class="w-11 h-6 bg-gray-200 peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-blue-300 dark:peer-focus:ring-blue-800 rounded-full peer dark:bg-gray-700 peer-checked:after:translate-x-full rtl:peer-checked:after:-translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:start-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all dark:border-gray-600 peer-checked:bg-blue-600"></div>
                <span class="ms-3 text-sm font-medium text-gray-900 dark:text-gray-300">{"Enable User Self Service"}</span>
            </label>
        </div>
    }
}

