use yew_router::Routable;

#[derive(Clone, Routable, PartialEq)]
pub enum Route {
    #[at("/")]
    Login,
    #[at("/home")]
    Home,
    #[not_found]
    #[at("/404")]
    NotFound,
    #[at("/change_server")]
    ChangeServer,
    #[at("/queue")]
    Queue,
    #[at("/saved")]
    Saved,
    #[at("/settings")]
    Settings,
    #[at("/history")]
    PodHistory,
    #[at("/downloads")]
    Downloads,
    #[at("/search")]
    Search,
    #[at("/user_stats")]
    UserStats,
    #[at("/sign_out")]
    LogOut,
}
