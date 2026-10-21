use anyhow::Error;
use gloo::net::http::{Request, Response};
use serde::{Deserialize, Serialize};

use crate::requests::episode::Episode;

// --- People credited on podcasts and episodes (/api/data/people/*) ---

/// A person as shown on a podcast or episode page.
#[derive(Deserialize, Serialize, Clone, PartialEq, Debug, Default)]
#[serde(default)]
pub struct CreditedPerson {
    /// PinePods person id; use it for the person page and follows.
    pub id: i32,
    pub podpeople_id: Option<i32>,
    pub name: String,
    pub img: Option<String>,
    pub href: Option<String>,
    /// Most significant role ("host", "co-host", "guest", ...).
    pub role: String,
    pub roles: Vec<String>,
    pub group: Option<String>,
    /// "podcast", "episodes" (some episodes of this show) or "episode" (this episode).
    pub scope: String,
    pub episode_count: i64,
    pub sources: Vec<String>,
    /// False while every credit is an unreviewed community submission.
    pub verified: bool,
    pub followed: bool,
}

#[derive(Deserialize, Clone, PartialEq, Debug, Default)]
#[serde(default)]
pub struct PodcastCredits {
    pub people: Vec<CreditedPerson>,
    pub guests: Vec<CreditedPerson>,
}

impl PodcastCredits {
    /// Split the flat list `people/contribute` returns for a podcast back into regulars and guests.
    pub fn from_flat(list: Vec<CreditedPerson>) -> Self {
        let (people, guests) = list.into_iter().partition(|p| p.scope == "podcast");
        PodcastCredits { people, guests }
    }
}

#[derive(Deserialize, Clone, PartialEq, Debug, Default)]
#[serde(default)]
pub struct PersonInfo {
    pub id: i32,
    pub podpeople_id: Option<i32>,
    pub name: String,
    pub img: Option<String>,
    pub href: Option<String>,
    pub followed: bool,
}

/// A podcast a person is credited on.
#[derive(Deserialize, Clone, PartialEq, Debug, Default)]
#[serde(default)]
pub struct PersonPodcast {
    /// The user's podcast id, or None for a show outside their library.
    pub podcastid: Option<i32>,
    pub podcastname: String,
    pub artworkurl: Option<String>,
    pub feedurl: String,
    pub podcastindexid: Option<i64>,
    pub roles: Vec<String>,
    pub podcast_level: bool,
    pub episode_count: i64,
}

#[derive(Deserialize, Clone, PartialEq, Debug, Default)]
#[serde(default)]
pub struct ExternalEpisode {
    pub podcastname: String,
    pub feedurl: String,
    pub episodetitle: String,
    pub episodepubdate: String,
    pub episodedescription: String,
    pub episodeartwork: String,
    pub episodeurl: String,
    pub episodeduration: i32,
    pub role: Option<String>,
}

impl ExternalEpisode {
    /// The shape episode lists render. Episode id 0 marks it as outside the library, like a
    /// search result, so playing it goes through the same add-on-play path.
    pub fn into_episode(self) -> Episode {
        Episode {
            podcastname: self.podcastname,
            feedurl: self.feedurl,
            episodetitle: self.episodetitle,
            episodepubdate: self.episodepubdate,
            episodedescription: self.episodedescription,
            artworkurl: self.episodeartwork.clone(),
            episodeartwork: self.episodeartwork,
            episodeurl: self.episodeurl,
            episodeduration: self.episodeduration,
            ..Default::default()
        }
    }
}

#[derive(Deserialize, Clone, PartialEq, Debug, Default)]
#[serde(default)]
pub struct PersonProfile {
    pub person: PersonInfo,
    pub podcasts: Vec<PersonPodcast>,
    pub episodes: Vec<Episode>,
    pub external_episodes: Vec<ExternalEpisode>,
    /// True while the server refreshes the outside-library episodes in the background.
    pub refreshing: bool,
}

/// A people search result, from this server or PodPeopleDB.
#[derive(Deserialize, Clone, PartialEq, Debug, Default)]
#[serde(default)]
pub struct PersonHit {
    pub person_id: Option<i32>,
    pub podpeople_id: Option<i64>,
    pub name: String,
    pub img: Option<String>,
    /// A few podcasts they're known for, to tell namesakes apart.
    pub known_for: Option<String>,
    pub followed: bool,
}

#[derive(Serialize, Clone, PartialEq, Debug, Default)]
pub struct ContributePerson {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub person_id: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub podpeople_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
}

#[derive(Serialize, Clone, PartialEq, Debug)]
pub struct ContributeRequest {
    pub podcast_id: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub episode_id: Option<i32>,
    pub people: Vec<ContributePerson>,
}

fn people_url(server_name: &str, path: &str) -> String {
    format!("{}/api/data/people/{}", server_name.trim_end_matches('/'), path)
}

/// The server's error message ("Bad request: PodPeopleDB didn't accept that: ...") without the
/// status prefix, falling back to the status text.
async fn error_from(response: Response, what: &str) -> Error {
    let status = response.status();
    let message = response
        .json::<serde_json::Value>()
        .await
        .ok()
        .and_then(|v| v.get("message").and_then(|m| m.as_str()).map(str::to_string))
        .map(|m| {
            ["Bad request: ", "Not found: ", "Internal server error: "]
                .iter()
                .find_map(|prefix| m.strip_prefix(prefix))
                .map(str::to_string)
                .unwrap_or(m)
        });
    Error::msg(message.unwrap_or_else(|| format!("{} ({})", what, status)))
}

async fn get_json<T: for<'de> Deserialize<'de>>(url: &str, api_key: &str, what: &str) -> Result<T, Error> {
    let response = Request::get(url).header("Api-Key", api_key).send().await?;
    if !response.ok() {
        return Err(error_from(response, what).await);
    }
    Ok(response.json::<T>().await?)
}

/// Hosts and regulars (`people`) and guests of one of the user's podcasts.
pub async fn call_get_podcast_people(server_name: &str, api_key: &str, podcast_id: i32) -> Result<PodcastCredits, Error> {
    get_json(&people_url(server_name, &format!("podcast/{}", podcast_id)), api_key, "Failed to load people").await
}

/// People on one of the user's episodes: its own credits plus the show's regulars.
pub async fn call_get_episode_people(server_name: &str, api_key: &str, episode_id: i32) -> Result<Vec<CreditedPerson>, Error> {
    get_json(&people_url(server_name, &format!("episode/{}", episode_id)), api_key, "Failed to load people").await
}

pub async fn call_get_person_profile(
    server_name: &str,
    api_key: &str,
    person_id: i32,
    limit: i64,
    offset: i64,
) -> Result<PersonProfile, Error> {
    let url = people_url(server_name, &format!("{}?limit={}&offset={}", person_id, limit, offset));
    get_json(&url, api_key, "Failed to load person").await
}

#[derive(Deserialize)]
struct LookupResponse {
    person_id: i32,
}

/// Resolve a PodPeopleDB id or a name to a PinePods person id.
pub async fn call_lookup_person(
    server_name: &str,
    api_key: &str,
    podpeople_id: Option<i64>,
    name: Option<&str>,
) -> Result<i32, Error> {
    let query = match (podpeople_id, name) {
        (Some(id), _) => format!("podpeople_id={}", id),
        (None, Some(name)) => format!("name={}", urlencoding::encode(name)),
        (None, None) => return Err(Error::msg("Nothing to look up")),
    };
    let found: LookupResponse =
        get_json(&people_url(server_name, &format!("lookup?{}", query)), api_key, "Person not found").await?;
    Ok(found.person_id)
}

pub async fn call_search_people(server_name: &str, api_key: &str, query: &str) -> Result<Vec<PersonHit>, Error> {
    let url = people_url(server_name, &format!("search?q={}", urlencoding::encode(query)));
    get_json(&url, api_key, "Search failed").await
}

pub async fn call_get_followed_people(server_name: &str, api_key: &str) -> Result<Vec<PersonInfo>, Error> {
    get_json(&people_url(server_name, "followed"), api_key, "Failed to load followed people").await
}

/// Follow (`true`) or unfollow a person.
pub async fn call_set_following(server_name: &str, api_key: &str, person_id: i32, follow: bool) -> Result<(), Error> {
    let url = people_url(server_name, &format!("{}/follow", person_id));
    let request = if follow { Request::post(&url) } else { Request::delete(&url) };
    let response = request.header("Api-Key", api_key).send().await?;
    if !response.ok() {
        return Err(error_from(response, "Couldn't update follow").await);
    }
    Ok(())
}

/// Add people to a podcast (or one of its episodes). Returns everyone on it afterwards.
pub async fn call_contribute_people(
    server_name: &str,
    api_key: &str,
    request: &ContributeRequest,
) -> Result<Vec<CreditedPerson>, Error> {
    let response = Request::post(&people_url(server_name, "contribute"))
        .header("Api-Key", api_key)
        .header("Content-Type", "application/json")
        .body(serde_json::to_string(request)?)?
        .send()
        .await?;
    if !response.ok() {
        return Err(error_from(response, "Couldn't add people").await);
    }
    Ok(response.json::<Vec<CreditedPerson>>().await?)
}

// --- Discovery (PodPeopleDB, proxied through the PinePods backend) ---

#[derive(Deserialize, Clone, PartialEq, Debug, Default)]
pub struct DiscoverHost {
    #[serde(default)]
    pub id: i32,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub img: String,
    #[serde(default, alias = "podcastCount")]
    pub podcast_count: i32,
}

#[derive(Deserialize, Clone, PartialEq, Debug, Default)]
pub struct DiscoverPodcast {
    #[serde(default)]
    pub podcast_id: i32,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub host_count: i32,
    #[serde(default)]
    pub description: String,
}

#[derive(Deserialize, Clone, PartialEq, Debug, Default)]
pub struct DiscoverStats {
    #[serde(default)]
    pub total_hosts: i32,
    #[serde(default)]
    pub total_podcasts: i32,
    #[serde(default)]
    pub approved_hosts: i32,
    #[serde(default)]
    pub episodes_with_guests: i32,
}

async fn discover_get(server_name: &str, api_key: &str, kind: &str, limit: Option<i32>) -> Result<String, Error> {
    let mut url = format!(
        "{}/api/data/podpeople/discover?kind={}",
        server_name, kind
    );
    if let Some(l) = limit {
        url.push_str(&format!("&limit={}", l));
    }
    let response = Request::get(&url).header("Api-Key", api_key).send().await?;
    if !response.ok() {
        return Err(Error::msg(format!(
            "Failed to fetch discover {}: {}",
            kind,
            response.status_text()
        )));
    }
    Ok(response.text().await?)
}

#[allow(dead_code)]
pub async fn call_discover_top_hosts(server_name: &str, api_key: &str, limit: i32) -> Result<Vec<DiscoverHost>, Error> {
    let text = discover_get(server_name, api_key, "top-hosts", Some(limit)).await?;
    Ok(serde_json::from_str(&text)?)
}

#[allow(dead_code)]
pub async fn call_discover_recent_hosts(server_name: &str, api_key: &str, limit: i32) -> Result<Vec<DiscoverHost>, Error> {
    let text = discover_get(server_name, api_key, "recent-hosts", Some(limit)).await?;
    Ok(serde_json::from_str(&text)?)
}

#[allow(dead_code)]
pub async fn call_discover_popular_podcasts(server_name: &str, api_key: &str, limit: i32) -> Result<Vec<DiscoverPodcast>, Error> {
    let text = discover_get(server_name, api_key, "popular-podcasts", Some(limit)).await?;
    Ok(serde_json::from_str(&text)?)
}

#[allow(dead_code)]
pub async fn call_discover_stats(server_name: &str, api_key: &str) -> Result<DiscoverStats, Error> {
    let text = discover_get(server_name, api_key, "stats", None).await?;
    Ok(serde_json::from_str(&text)?)
}
