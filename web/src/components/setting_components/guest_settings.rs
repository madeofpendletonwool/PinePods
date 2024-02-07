use yew::prelude::*;

#[function_component(GuestSettings)]
pub fn guest_settings() -> Html {
    html! {
        <div class="p-4"> // You can adjust the padding as needed
            <p class="text-lg font-bold mb-4">{"Guest User Settings:"}</p> // Styled paragraph
            <p class="text-md mb-4">{"You can choose to enable or disable the Guest user here. It's always disabled by default. Basically, enabling the guest user enables a button on the login page to login as guest. This guest user essentially has access to add podcasts and listen to them in an ephemeral sense. Once logged out, the session is deleted along with any podcasts the Guest saved. If your Pinepods server is exposed to the internet you probably want to disable this option. It's meant more for demos or if you want to allow people to quickly listen to a podcast using your server."}</p> // Styled paragraph

            <label class="relative inline-flex items-center cursor-pointer">
                <input type="checkbox" value="" class="sr-only peer" />
                <div class="w-11 h-6 bg-gray-200 peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-blue-300 dark:peer-focus:ring-blue-800 rounded-full peer dark:bg-gray-700 peer-checked:after:translate-x-full rtl:peer-checked:after:-translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:start-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all dark:border-gray-600 peer-checked:bg-blue-600"></div>
                <span class="ms-3 text-sm font-medium text-gray-900 dark:text-gray-300">{"Enable Guest User"}</span>
            </label>
        </div>
    }
}

