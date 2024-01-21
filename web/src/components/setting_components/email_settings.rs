use yew::prelude::*;

#[function_component(EmailSettings)]
pub fn email_settings() -> Html {
    let auth_required = use_state(|| false);

    let toggle_auth_required = {
        let auth_required = auth_required.clone();
        Callback::from(move |_| auth_required.set(!*auth_required))
    };

    html! {
        <div class="p-4">
            <p class="text-lg font-bold mb-4">{"Email Setup:"}</p>
            <p class="text-md mb-4">{"You can setup server Email settings here. Email is mostly used for self service password resets. The server will require that you verify your email settings setup before it will allow you to submit the settings you've entered."}</p>

            <button class="mt-4 bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline" type="button">
                {"Download/Export OPML"}
            </button>

            <div class="flex mt-4">
                <input type="text" placeholder="Server Name" class="border p-2 mr-2 rounded"/>
                <span>{":"}</span>
                <input type="text" placeholder="Port" class="border p-2 ml-2 rounded"/>
            </div>

            <div class="mt-4">
                <p class="font-medium">{"Send Mode:"}</p>
                <select class="border p-2 rounded mr-2">
                    <option>{"SMTP"}</option>
                </select>
            </div>
            <div class="mt-4">
                <p class="font-medium">{"Encryption:"}</p>
                <select class="border p-2 rounded">
                    <option value="none" selected=true>{"None"}</option>
                    <option>{"SSL/TLS"}</option>
                    <option>{"StartTLS"}</option>
                </select>
            </div>

            <input type="text" placeholder="From Address" class="border p-2 mt-4 rounded"/>

            <div class="flex items-center mt-4">
                <input type="checkbox" id="auth_required" checked={*auth_required} onclick={toggle_auth_required}/>
                <label for="auth_required" class="ml-2">{"Authentication Required"}</label>
            </div>
            {
                if *auth_required {
                    html! {
                                    <>
                                        <input type="text" placeholder="Username" class="border p-2 mt-4 rounded"/>
                                        <input type="password" placeholder="Password" class="border p-2 mt-4 rounded"/>
                                    </>
                                }
                } else {
                    html! {}
                }
            }
            <div class="flex mt-4">
                <button class="bg-green-500 hover:bg-green-700 text-white font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline mr-2" type="button">
                    {"Test & Submit"}
                </button>
            </div>
        </div>
    }
}
