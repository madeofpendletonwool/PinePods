use yew::prelude::*;

#[function_component(UserSettings)]
pub fn user_settings() -> Html {
    html! {
        <>
            <div class="p-4">
                <p class="text-lg font-bold mb-4">{"User Management:"}</p>
                <p class="text-md mb-4">{"You can manage users here. Click a user in the table to manage settings for that existing user or click 'Create New' to add a new user. Note that the guest user will always show regardless of whether it's enabled or not. View the Guest Settings Area to properly manage that."}</p>
                <button class="mt-4 bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline" type="button">
                    {"Create New User"}
                </button>
            </div>
            <div class="relative overflow-x-auto">
                <table class="w-full text-sm text-left rtl:text-right text-gray-500 dark:text-gray-400">
                    <thead class="text-xs uppercase table-header">
                        <tr>
                            <th scope="col" class="px-6 py-3">{"User ID"}</th>
                            <th scope="col" class="px-6 py-3">{"Fullname"}</th>
                            <th scope="col" class="px-6 py-3">{"Email"}</th>
                            <th scope="col" class="px-6 py-3">{"Username"}</th>
                            <th scope="col" class="px-6 py-3">{"Admin Status"}</th>
                        </tr>
                    </thead>
                    <tbody>
                        // { /* Replace with dynamic data in future */ }
                        <tr class="table-row border-b cursor-pointer">
                            <td class="px-6 py-4">{"1234"}</td>
                            <td class="px-6 py-4">{"6789"}</td>
                            <td class="px-6 py-4">{"2023-03-01"}</td>
                            <td class="px-6 py-4">{"User A"}</td>
                            <td class="px-6 py-4">{"User A"}</td>
                        </tr>
                        <tr class="table-row border-b cursor-pointer">
                            <td class="px-6 py-4">{"5678"}</td>
                            <td class="px-6 py-4">{"1234"}</td>
                            <td class="px-6 py-4">{"2023-03-02"}</td>
                            <td class="px-6 py-4">{"User B"}</td>
                            <td class="px-6 py-4">{"User A"}</td>
                        </tr>
                        // { /* Add more rows as needed */ }
                    </tbody>
                </table>
            </div>
        </>
    }
}