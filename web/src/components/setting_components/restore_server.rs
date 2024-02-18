use yew::prelude::*;

#[function_component(RestoreServer)]
pub fn restore_server() -> Html {
    html! {
        <div class="p-4"> // You can adjust the padding as needed
            <p class="text-lg font-bold mb-4">{"Restore Server:"}</p> // Styled paragraph
            <p class="text-md mb-4">{"With this option you can restore your entire server with all it's previous settings, users and data from a backup. Take a backup above to restore here. WARNING: This will delete everything on your server now and restore to the point that the backup contains."}</p> // Styled paragraph

            <button class="mt-4 bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline" type="button">
                {"Restore Server from Backup"}
            </button>
        </div>
    }
}

