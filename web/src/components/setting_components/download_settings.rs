use yew::prelude::*;

#[function_component(DownloadSettings)]
pub fn download_settings() -> Html {
    html! {
        <div class="p-4"> // You can adjust the padding as needed
            <p class="text-lg font-bold mb-4">{"Server Download Settings:"}</p> // Styled paragraph
            <p class="text-md mb-4">{"You can choose to enable or disable server downloads here. This does not effect local downloads. There's two types of downloads in Pinepods. Local and Server. Local downloads would be where a user clicks download and it downloads the podcast to their local machine. A server download is when a user downloads the podcast to the server specifically. This is meant as an archival option. If you're concerned the podcast may not be always available you may want to archive it using this option. See the Pinepods documentation for mapping a specific location (like a NAS) as the location server downloads download to. You might want to turn this option off if you have self service enabled or your Pinepods server accessible to the internet. You wouldn't want any random user filling up your server."}</p> // Styled paragraph

            <label class="relative inline-flex items-center cursor-pointer">
                <input type="checkbox" value="" class="sr-only peer" />
                <div class="w-11 h-6 bg-gray-200 peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-blue-300 dark:peer-focus:ring-blue-800 rounded-full peer dark:bg-gray-700 peer-checked:after:translate-x-full rtl:peer-checked:after:-translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:start-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all dark:border-gray-600 peer-checked:bg-blue-600"></div>
                <span class="ms-3 text-sm font-medium text-gray-900 dark:text-gray-300">{"Enable Server Downloads"}</span>
            </label>
        </div>
    }
}

