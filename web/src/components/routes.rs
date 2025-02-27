use yew_router::Routable;

#[derive(Clone, Routable, PartialEq)]
pub enum Route {
    #[at("/")]
    Login,
    #[at("/home")]
    Home,
    #[at("/feed")]
    Feed,
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
    #[at("/local_downloads")]
    LocalDownloads,
    #[at("/search")]
    Search,
    #[at("/user_stats")]
    UserStats,
    #[at("/sign_out")]
    LogOut,
    #[at("/person/:name")]
    Person { name: String },
    #[at("/pod_layout")]
    PodLayout,
    #[at("/people_subs")]
    SubscribedPeople,
    #[at("/search_new")]
    SearchNew,
    #[at("/podcasts")]
    Podcasts,
    #[at("/episode_layout")]
    EpisodeLayout,
    #[at("/episode")]
    Episode,
    #[at("/youtube_layout")]
    YoutubeLayout,
    #[at("/playlists")]
    Playlists,
    #[at("/shared_episode/:url_key")]
    SharedEpisode { url_key: String },
    #[at("/oauth/callback")]
    OAuthCallback,
    #[at("/playlist/:id")]
    PlaylistDetail { id: i32 },
}
