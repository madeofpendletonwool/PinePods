// A gpodder.net-compatible server (gpodder.net, oPodSync, another Pinepods, ...). Pinepods is
// one device there, named by SyncTargets.DeviceName. Logs in once per run and rides the session
// cookie; servers without the login endpoint, or that set no cookie, get Basic on every request.

use async_trait::async_trait;
use reqwest::{Method, StatusCode};
use serde_json::{json, Value};

use super::{pulled_actions, sub_changes, Http, PulledActions, SubChanges, SyncTarget};
use crate::error::{AppError, AppResult};
use crate::services::sync::model::OutboundAction;

pub const PUSH_BATCH: usize = 200;
const DEVICE_CAPTION: &str = "PinePods";
const DEVICE_TYPE: &str = "server";
const DEFAULT_DEVICE: &str = "pinepods";

pub struct GpodderTarget {
    http: Http,
    device: Option<String>,
}

impl GpodderTarget {
    pub fn new(url: &str, username: &str, password: &str, device: Option<String>) -> AppResult<Self> {
        Ok(GpodderTarget { http: Http::new(url, username, password)?, device })
    }

    fn device(&self) -> AppResult<&str> {
        self.device.as_deref().ok_or_else(|| AppError::internal("no device configured for the gpodder server"))
    }

    fn user_file(&self) -> String {
        format!("{}.json", self.http.username)
    }

    /// Check credentials: log in with Basic. Used by the settings flow before saving.
    pub async fn verify(url: &str, username: &str, password: &str) -> AppResult<()> {
        GpodderTarget::new(url, username, password, None)?.login().await
    }
}

/// The device Pinepods should use: `current` unless another client owns it (a device of a type
/// other than "server" with that id), then the first free `pinepods`, `pinepods-2`, ...
fn choose_device(current: Option<&str>, remote: &[(String, String)]) -> String {
    let taken = |name: &str| remote.iter().any(|(id, kind)| id == name && kind != DEVICE_TYPE);
    if let Some(name) = current.filter(|n| !taken(n)) {
        return name.to_string();
    }
    std::iter::once(DEFAULT_DEVICE.to_string())
        .chain((2..).map(|n| format!("{DEFAULT_DEVICE}-{n}")))
        .find(|name| !taken(name))
        .unwrap_or_else(|| DEFAULT_DEVICE.to_string())
}

#[async_trait]
impl SyncTarget for GpodderTarget {
    async fn login(&mut self) -> AppResult<()> {
        let url = self.http.url(&["api", "2", "auth", &self.http.username, "login.json"], &[]);
        let (status, headers, _) = self.http.send_raw(Method::POST, url.clone(), None, true).await?;
        match status {
            s if s.is_success() => {
                // The client's cookie jar keeps the session for the rest of the run.
                let has_session = headers
                    .get_all(reqwest::header::SET_COOKIE)
                    .iter()
                    .any(|v| v.to_str().is_ok_and(|v| v.trim_start().starts_with("sessionid=")));
                self.http.basic = !has_session;
                Ok(())
            }
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                Err(AppError::unauthorized(format!("The gpodder server refused the username or password (HTTP {})", status.as_u16())))
            }
            // No login endpoint: authenticate every request instead.
            StatusCode::NOT_FOUND | StatusCode::METHOD_NOT_ALLOWED => {
                self.http.basic = true;
                Ok(())
            }
            s => Err(AppError::internal(format!("POST {}: HTTP {}", url.path(), s.as_u16()))),
        }
    }

    async fn prepare_device(&mut self, full: bool) -> AppResult<Option<String>> {
        if !full && self.device.is_some() {
            return Ok(None);
        }
        let list = self.http.send(Method::GET, self.http.url(&["api", "2", "devices", &self.user_file()], &[]), None).await?;
        let remote: Vec<(String, String)> = list
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|d| Some((d.get("id")?.as_str()?.to_string(), d.get("type").and_then(Value::as_str).unwrap_or("").to_string())))
            .collect();
        let chosen = choose_device(self.device.as_deref(), &remote);
        if let Some(old) = self.device.as_deref().filter(|old| *old != chosen) {
            tracing::warn!("Device {:?} on the gpodder server belongs to another client; Pinepods now syncs as {:?}", old, chosen);
        }
        let url = self.http.url(&["api", "2", "devices", &self.http.username, &format!("{chosen}.json")], &[]);
        self.http.send(Method::POST, url, Some(&json!({ "caption": DEVICE_CAPTION, "type": DEVICE_TYPE }))).await?;
        let changed = self.device.as_deref() != Some(chosen.as_str());
        self.device = Some(chosen.clone());
        Ok(changed.then_some(chosen))
    }

    async fn pull_subscriptions(&self, since: i64) -> AppResult<SubChanges> {
        let url = self.http.url(&["api", "2", "subscriptions", &self.http.username, &format!("{}.json", self.device()?)], &[("since", since.to_string())]);
        let what = format!("GET {}", url.path());
        sub_changes(&self.http.send(Method::GET, url, None).await?, &what)
    }

    async fn push_subscriptions(&self, add: &[String], remove: &[String]) -> AppResult<()> {
        let url = self.http.url(&["api", "2", "subscriptions", &self.http.username, &format!("{}.json", self.device()?)], &[]);
        self.http.send(Method::POST, url, Some(&json!({ "add": add, "remove": remove }))).await?;
        Ok(())
    }

    async fn pull_actions(&self, since: i64) -> AppResult<PulledActions> {
        let url = self.http.url(&["api", "2", "episodes", &self.user_file()], &[("since", since.to_string())]);
        let what = format!("GET {}", url.path());
        pulled_actions(&self.http.send(Method::GET, url, None).await?, &what)
    }

    async fn push_actions(&self, batch: &[OutboundAction]) -> AppResult<()> {
        let device = self.device()?.to_string();
        let body: Vec<OutboundAction> = batch.iter().cloned().map(|a| OutboundAction { device: Some(device.clone()), ..a }).collect();
        let url = self.http.url(&["api", "2", "episodes", &self.user_file()], &[]);
        self.http.send(Method::POST, url, Some(&serde_json::to_value(body)?)).await?;
        Ok(())
    }

    fn push_batch_size(&self) -> usize {
        PUSH_BATCH
    }

    fn own_device(&self) -> Option<&str> {
        self.device.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::choose_device;

    fn devices(v: &[(&str, &str)]) -> Vec<(String, String)> {
        v.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
    }

    #[test]
    fn never_takes_another_clients_device() {
        let remote = devices(&[("phone", "mobile"), ("pinepods", "server")]);
        assert_eq!(choose_device(Some("phone"), &remote), "pinepods");
        assert_eq!(choose_device(Some("pinepods"), &remote), "pinepods");
        assert_eq!(choose_device(Some("pinepods-home"), &remote), "pinepods-home");
        assert_eq!(choose_device(None, &devices(&[("pinepods", "mobile"), ("pinepods-2", "desktop")])), "pinepods-3");
    }
}
