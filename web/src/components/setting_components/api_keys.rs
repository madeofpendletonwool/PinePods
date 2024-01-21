use yew::prelude::*;

#[function_component(APIKeys)]
pub fn api_keys() -> Html {
    html! {
        <>
            <div class="p-4">
                <p class="text-lg font-bold mb-4">{"API Keys:"}</p>
                <p class="text-md mb-4">{"You can request a Pinepods API Key here. These keys can then be used in conjunction with other Pinepods apps (like Pinepods Firewood) to connect them to the Pinepods server. In addition, you can also use an API Key to authenticate to this server from any other Pinepods server. Sort of like using a different server as a client for this one."}</p>
                <button class="mt-4 bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline" type="button">
                    {"Request API Key"}
                </button>
            </div>
            <div class="relative overflow-x-auto">
                <table class="w-full text-sm text-left rtl:text-right text-gray-500 dark:text-gray-400">
                    <thead class="text-xs uppercase table-header">
                        <tr>
                            <th scope="col" class="px-6 py-3">{"API ID"}</th>
                            <th scope="col" class="px-6 py-3">{"Last 4 Digits"}</th>
                            <th scope="col" class="px-6 py-3">{"Date Created"}</th>
                            <th scope="col" class="px-6 py-3">{"User"}</th>
                        </tr>
                    </thead>
                    <tbody>
                        // { /* Replace with dynamic data in future */ }
                        <tr class="table-row border-b cursor-pointer">
                            <td class="px-6 py-4">{"1234"}</td>
                            <td class="px-6 py-4">{"6789"}</td>
                            <td class="px-6 py-4">{"2023-03-01"}</td>
                            <td class="px-6 py-4">{"User A"}</td>
                        </tr>
                        <tr class="table-row border-b cursor-pointer">
                            <td class="px-6 py-4">{"5678"}</td>
                            <td class="px-6 py-4">{"1234"}</td>
                            <td class="px-6 py-4">{"2023-03-02"}</td>
                            <td class="px-6 py-4">{"User B"}</td>
                        </tr>
                        // { /* Add more rows as needed */ }
                    </tbody>
                </table>
            </div>
        </>
    }
}