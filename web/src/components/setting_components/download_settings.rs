use yew::prelude::*;

#[function_component(DownloadSettings)]
pub fn download_settings() -> Html {
    html! {
        <div class="p-4"> // You can adjust the padding as needed
            <p class="text-lg font-bold mb-4">{"Server Download Settings:"}</p> // Styled paragraph
            <p class="text-md mb-4">{"You can choose to enable or disable server downloads here. This does not effect local downloads. There's two types of downloads in Pinepods. Local and Server. Local downloads would be where a user clicks download and it downloads the podcast to their local machine. A server download is when a user downloads the podcast to the server specifically. This is meant as an archival option. If you're concerned the podcast may not be always available you may want to archive it using this option. See the Pinepods documentation for mapping a specific location (like a NAS) as the location server downloads download to. You might want to turn this option off if you have self service enabled or your Pinepods server accessible to the internet. You wouldn't want any random user filling up your server."}</p> // Styled paragraph

            <button class="mt-4 bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline" type="button">
                {"Enable Server Downloads"}
            </button>
        </div>
    }
}

