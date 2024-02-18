use yew::prelude::*;

#[function_component(BackupServer)]
pub fn backup_server() -> Html {
    html! {
        <div class="p-4"> // You can adjust the padding as needed
            <p class="text-lg font-bold mb-4">{"Backup Server:"}</p> // Styled paragraph
            <p class="text-md mb-4">{"You can download a backup of your entire server database here. This will include absolutely everything, users, podcasts, episodes, settings, api keys. Should something happen to your server or you need to migrate to a new server you'll be able to easily restore to this previous backup with the restore option below."}</p> // Styled paragraph

            <button class="mt-4 bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline" type="button">
                {"Download Server Backup"}
            </button>
        </div>
    }
}

