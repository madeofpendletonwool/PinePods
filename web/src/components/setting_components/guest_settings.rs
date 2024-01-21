use yew::prelude::*;

#[function_component(GuestSettings)]
pub fn guest_settings() -> Html {
    html! {
        <div class="p-4"> // You can adjust the padding as needed
            <p class="text-lg font-bold mb-4">{"Guest User Settings:"}</p> // Styled paragraph
            <p class="text-md mb-4">{"You can choose to enable or disable the Guest user here. It's always disabled by default. Basically, enabling the guest user enables a button on the login page to login as guest. This guest user essentially has access to add podcasts and listen to them in an ephemeral sense. Once logged out, the session is deleted along with any podcasts the Guest saved. If your Pinepods server is exposed to the internet you probably want to disable this option. It's meant more for demos or if you want to allow people to quickly listen to a podcast using your server."}</p> // Styled paragraph

            <button class="mt-4 bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline" type="button">
                {"Enable Guest User"}
            </button>
        </div>
    }
}

