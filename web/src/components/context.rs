use crate::requests::login_requests::GetUserDetails;
use crate::requests::login_requests::LoginServerRequest;
use crate::requests::login_requests::GetApiDetails;
use crate::requests::search_pods::PodcastSearchResult;
use yewdux::prelude::*;

#[derive(Default, Clone, PartialEq, Store)]
pub struct AppState {
    pub user_details: Option<GetUserDetails>,
    pub auth_details: Option<LoginServerRequest>,
    pub server_details: Option<GetApiDetails>,
    pub error_message: Option<String>,
    pub search_results: Option<PodcastSearchResult>
}