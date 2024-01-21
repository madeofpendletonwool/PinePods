use yew::prelude::*;

#[function_component(ImportOptions)]
pub fn import_options() -> Html {
    html! {
        <div class="p-4"> // You can adjust the padding as needed
            <p class="text-lg font-bold mb-4">{"Import Options:"}</p> // Styled paragraph
            <p class="text-md mb-4">{"You can Import an OPML of podcasts here. If you're migrating from a different podcast app this is probably the solution you want. Most podcast apps allow you to export a backup of your saved podcasts to an OPML file and this option can easily import them into Pinepods."}</p> // Styled paragraph

            <button class="mt-4 bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline" type="button">
                {"Import OPML"}
            </button>
        </div>
    }
}

