//! Links the app is registered for: the OIDC sign-in return, and podcast subscribe links.

use url::Url;

/// Schemes registered with the system; a test keeps this in step with tauri.conf.json.
#[cfg_attr(not(test), allow(dead_code))]
pub const SCHEMES: [&str; 4] = ["pinepods", "pinepods-subscribe", "podcast", "itpc"];
/// The ones only Pinepods uses, safe to claim again at every start.
#[cfg_attr(not(any(windows, target_os = "linux")), allow(dead_code))]
pub const OWN_SCHEMES: [&str; 2] = ["pinepods", "pinepods-subscribe"];

/// The link the server sends after an OIDC sign-in: `pinepods://auth/callback?...`.
pub fn is_sign_in_link(url: &Url) -> bool {
    url.scheme() == "pinepods" && url.host_str() == Some("auth") && url.path() == "/callback"
}

/// The feed a subscribe link points at:
/// - `pinepods-subscribe://?url=<feed>` (the mobile app's format, also `uri=`)
/// - `podcast://host/feed.xml` → `https://host/feed.xml`, `itpc://host/feed` → `http://…`
/// - `podcast://https://host/feed.xml` and `podcast:https://…` as some sites write them
pub fn feed_from_link(url: &Url) -> Option<String> {
    let feed = match url.scheme() {
        "pinepods-subscribe" => url
            .query_pairs()
            .find(|(k, _)| k == "url" || k == "uri")
            .map(|(_, v)| v.into_owned())?,
        scheme @ ("podcast" | "itpc") => {
            let rest = url.as_str()[scheme.len() + 1..].trim_start_matches('/');
            if rest.starts_with("http://") || rest.starts_with("https://") {
                rest.to_string()
            } else if let Some(r) = rest.strip_prefix("https//").or_else(|| rest.strip_prefix("https/")) {
                format!("https://{r}")
            } else if let Some(r) = rest.strip_prefix("http//").or_else(|| rest.strip_prefix("http/")) {
                format!("http://{r}")
            } else {
                let web = if scheme == "itpc" { "http" } else { "https" };
                format!("{web}://{rest}")
            }
        }
        _ => return None,
    };
    let feed = Url::parse(&feed).ok()?;
    (matches!(feed.scheme(), "http" | "https") && feed.host_str().is_some()).then(|| feed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(link: &str) -> Option<String> {
        feed_from_link(&link.parse().unwrap())
    }

    #[test]
    fn sign_in_link_is_exact() {
        let link = |s: &str| is_sign_in_link(&s.parse().unwrap());
        assert!(link("pinepods://auth/callback?api_key=k&state=s"));
        assert!(link("pinepods://auth/callback?error=no_access"));
        assert!(!link("pinepods://auth/callbackevil?api_key=k"));
        assert!(!link("pinepods://other/callback?api_key=k"));
        assert!(!link("https://auth/callback?api_key=k"));
    }

    #[test]
    fn schemes_match_the_config() {
        let config: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let schemes = &config["plugins"]["deep-link"]["desktop"]["schemes"];
        assert_eq!(schemes, &serde_json::json!(SCHEMES));
    }

    #[test]
    fn subscribe_links() {
        let want = Some("https://example.com/feed.xml".to_string());
        assert_eq!(feed("pinepods-subscribe://?url=https://example.com/feed.xml"), want);
        assert_eq!(feed("pinepods-subscribe://?url=https%3A%2F%2Fexample.com%2Ffeed.xml"), want);
        assert_eq!(feed("podcast://example.com/feed.xml"), want);
        assert_eq!(feed("podcast://https://example.com/feed.xml"), want);
        assert_eq!(feed("podcast:https://example.com/feed.xml"), want);
        assert_eq!(feed("podcast://https//example.com/feed.xml"), want);
        assert_eq!(feed("itpc://example.com/feed.xml"), Some("http://example.com/feed.xml".into()));
        assert_eq!(feed("pinepods-subscribe://?url=file:///etc/passwd"), None);
        assert_eq!(feed("pinepods-subscribe://?url=javascript:alert(1)"), None);
        assert_eq!(feed("pinepods://auth/callback?url=https://example.com"), None);
        assert_eq!(feed("podcast://"), None);
    }
}
