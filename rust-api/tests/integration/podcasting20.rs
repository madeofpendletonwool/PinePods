//! Podcasting 2.0 tags: chapters, transcripts and people, however the feed is laid out.

use crate::harness::*;
use crate::harness::rss;

const CHAPTERS_JSON: &str = r#"{"version":"1.2.0","chapters":[{"startTime":0,"title":"Intro"},{"startTime":95,"title":"Main topic"}]}"#;

/// Item-level tags for one episode; `chapters` is the chapters JSON URL.
fn item_tags(chapters: &str) -> String {
    format!(
        "      <podcast:transcript url=\"https://cdn.example.invalid/t/ep.vtt\" type=\"text/vtt\" language=\"en\" rel=\"captions\"/>\n\
         \x20     <podcast:transcript url=\"https://cdn.example.invalid/t/ep.json\" type=\"application/json\"/>\n\
         \x20     <podcast:chapters url=\"{}\" type=\"application/json+chapters\"/>\n\
         \x20     <podcast:person role=\"guest\" img=\"https://example.invalid/g.jpg\" href=\"https://example.invalid/g\">Ana &amp; Bo</podcast:person>\n",
        rss::esc(chapters)
    )
}

const CHANNEL_PERSON: &str = "    <podcast:person role=\"host\">Hosty McHost</podcast:person>\n";

async fn p20(app: &TestApp, u: &TestUser, ep: i32) -> Value {
    app.get("/api/data/fetch_podcasting_2_data")
        .key(u)
        .query("episode_id", ep)
        .query("user_id", u.id)
        .send()
        .await
        .ok_json()
}

fn assert_full(data: &Value) {
    let chapter_titles: Vec<&str> =
        data["chapters"].as_array().unwrap().iter().map(|c| c["title"].as_str().unwrap()).collect();
    assert_eq!(chapter_titles, ["Intro", "Main topic"], "{data:#}");

    let transcripts = data["transcripts"].as_array().unwrap();
    assert_eq!(transcripts.len(), 2, "{data:#}");
    assert_eq!(transcripts[0]["url"], "https://cdn.example.invalid/t/ep.vtt");
    assert_eq!(transcripts[0]["mime_type"], "text/vtt");
    assert_eq!(transcripts[0]["language"], "en");
    assert_eq!(transcripts[0]["rel"], "captions");
    assert_eq!(transcripts[1]["mime_type"], "application/json");
    assert!(transcripts[1]["language"].is_null());

    let mut people: Vec<(String, String)> = data["people"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| (p["name"].as_str().unwrap().to_string(), p["role"].as_str().unwrap_or_default().to_string()))
        .collect();
    people.sort();
    assert_eq!(
        people,
        [("Ana & Bo".to_string(), "guest".to_string()), ("Hosty McHost".to_string(), "host".to_string())],
        "channel people plus the episode's own, entities decoded: {data:#}"
    );
}

/// Subscribe to `xml` and return (user, id of "Episode 2", id of "Episode 1").
async fn subscribe_raw(app: &TestApp, name: &str, xml: String) -> (TestUser, i32, i32) {
    let user = app.user(name).await;
    let url = app.fixtures().serve_feed(&format!("{name}.xml"), xml);
    let pod = app.subscribe(&user, &url, 2).await;
    let two = app.episode_id(&user, pod, "Episode 2").await;
    let one = app.episode_id(&user, pod, "Episode 1").await;
    (user, two, one)
}

#[tokio::test(flavor = "multi_thread")]
async fn chapters_transcripts_and_people_are_read_for_the_right_episode() {
    let app = TestApp::new().await;
    let chapters = app.fixtures().serve("chapters.json", Fixture::new("application/json", CHAPTERS_JSON));
    let feed = Feed::new("P20 Show")
        .extra(CHANNEL_PERSON)
        .episode(Episode::new(1))
        .episode(Episode::new(2).extra(&item_tags(&chapters)));
    let (user, two, one) = subscribe_raw(&app, "listener", feed.to_xml()).await;

    assert_full(&p20(&app, &user, two).await);

    // Episode 1 has no tags of its own: only the channel-level host shows up.
    let data = p20(&app, &user, one).await;
    assert_eq!(data["chapters"], json!([]), "{data:#}");
    assert_eq!(data["transcripts"], json!([]));
    let names: Vec<&str> = data["people"].as_array().unwrap().iter().map(|p| p["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["Hosty McHost"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn tags_are_found_in_minified_feeds_and_before_the_enclosure() {
    let app = TestApp::new().await;
    let chapters = app.fixtures().serve("chapters.json", Fixture::new("application/json", CHAPTERS_JSON));

    // Tags ahead of <enclosure>, which many publishers emit.
    let feed = Feed::new("Early Tags").extra(CHANNEL_PERSON).episode(Episode::new(1)).episode(Episode::new(2));
    let early = feed.to_xml().replacen("<item>\n", &format!("<item>\n{}", item_tags(&chapters)), 1);
    let (user, two, _) = subscribe_raw(&app, "early", early.clone()).await;
    assert_full(&p20(&app, &user, two).await);

    // The same feed with every line break removed.
    let minified: String = early.lines().map(str::trim).collect();
    let (user, two, _) = subscribe_raw(&app, "minified", minified).await;
    assert_full(&p20(&app, &user, two).await);
}

#[tokio::test(flavor = "multi_thread")]
async fn episodes_whose_audio_url_has_a_query_string_still_match() {
    let app = TestApp::new().await;
    let chapters = app.fixtures().serve("chapters.json", Fixture::new("application/json", CHAPTERS_JSON));
    let feed = Feed::new("Tracked")
        .extra(CHANNEL_PERSON)
        .episode(Episode::new(1))
        .episode(
            Episode::new(2)
                .enclosure("https://track.example.invalid/redirect?u=ep2.mp3&src=rss")
                .extra(&item_tags(&format!("{chapters}?v=1&fmt=json"))),
        );
    let (user, two, _) = subscribe_raw(&app, "tracked", feed.to_xml()).await;
    assert_full(&p20(&app, &user, two).await);
}
