use crate::requests::login_requests::GetUserDetails;
use crate::requests::login_requests::LoginServerRequest;
use yewdux::prelude::*;

#[derive(Default, Clone, PartialEq, Store)]
pub struct AppState {
    pub user_details: Option<GetUserDetails>,
    pub auth_details: Option<LoginServerRequest>,
}