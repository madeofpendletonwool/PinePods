use yew::prelude::*;

#[function_component(ExportOptions)]
pub fn export_options() -> Html {
    html! {
        <div class="p-4"> // You can adjust the padding as needed
            <p class="text-lg font-bold mb-4">{"Export Options:"}</p> // Styled paragraph
            <p class="text-md mb-4">{"You can export an OPML file containing your Podcasts here. This file can then be imported if you want to switch to a different podcast app or simply want a backup of your files just in case. Note, if you are exporting to add your podcasts to AntennaPod the Nextcloud Options below might better suit your needs. If you're an admin a full server backup might be a better solution as well on the Admin Settings Page."}</p> // Styled paragraph

            <button class="mt-4 bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline" type="button">
                {"Download/Export OPML"}
            </button>
        </div>
    }
}

