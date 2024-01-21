use yew::prelude::*;

#[function_component(NextcloudOptions)]
pub fn nextcloud_options() -> Html {
    html! {
        <div class="p-4"> // You can adjust the padding as needed
            <p class="text-lg font-bold mb-4">{"Nextcloud Podcast Sync:"}</p> // Styled paragraph
            <p class="text-md mb-4">{"With this option you can authenticate with a Nextcloud server to use as a podcast sync client. This option works great with AntennaPod on Android so you can have the same exact feed there while on mobile. In addition, if you're already using AntennaPod with Nextcloud Podcast sync you can connect your existing sync feed to quickly import everything right into Pinepods! Clicking the Authenticate Button will prompt you to externally import your Nextcloud Server."}</p> // Styled paragraph

            <button class="mt-4 bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline" type="button">
                {"Authenticate Nextcloud Server"}
            </button>
        </div>
    }
}

