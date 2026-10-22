//! Saved episodes, collections (manual + category auto-add) and smart playlists.

use crate::harness::*;

struct Lib {
    user: TestUser,
    pod: i32,
    ep: Vec<i32>, // Episode 1..=4 (durations 1801..1804s)
}

async fn setup(app: &TestApp) -> Lib {
    let user = app.user("collector").await;
    let url = app.feed("lib.xml", &Feed::with_episodes("Library Show", 4).categories(&["Technology"]));
    let pod = app.subscribe(&user, &url, 4).await;
    let mut ep = Vec::new();
    for n in 1..=4 {
        ep.push(app.episode_id(&user, pod, &format!("Episode {n}")).await);
    }
    Lib { user, pod, ep }
}

async fn save(app: &TestApp, u: &TestUser, ep: i32) {
    app.post("/api/data/save_episode")
        .key(u)
        .json(&json!({ "episode_id": ep, "user_id": u.id, "is_youtube": false }))
        .send()
        .await
        .assert_ok();
}

async fn saved_ids(app: &TestApp, u: &TestUser) -> Vec<i32> {
    let body = app.get(&format!("/api/data/saved_episode_list/{}", u.id)).key(u).send().await.ok_json();
    ids(&body["saved_episodes"])
}

fn ids(list: &Value) -> Vec<i32> {
    let mut v: Vec<i32> = list
        .as_array()
        .unwrap_or_else(|| panic!("not a list: {list}"))
        .iter()
        .map(|e| e["episodeid"].as_i64().or(e["Episodeid"].as_i64()).unwrap() as i32)
        .collect();
    v.sort();
    v
}

async fn collections(app: &TestApp, u: &TestUser) -> Vec<Value> {
    let body = app.get(&format!("/api/data/collections/user/{}", u.id)).key(u).send().await.ok_json();
    body["collections"].as_array().cloned().unwrap()
}

async fn collection_episode_ids(app: &TestApp, u: &TestUser, collection: i32) -> Vec<i32> {
    let body = app
        .get(&format!("/api/data/collections/{collection}/episodes"))
        .key(u)
        .query("limit", 100)
        .query("offset", 0)
        .query("sort_by", "date")
        .query("sort_order", "desc")
        .query("filter", "all")
        .send()
        .await
        .ok_json();
    ids(&body["saved_episodes"])
}

async fn create_collection(app: &TestApp, u: &TestUser, body: Value) -> i32 {
    let mut body = body;
    body["user_id"] = json!(u.id);
    let r = app.post("/api/data/collections/create").key(u).json(&body).send().await.ok_json();
    r["collection_id"].as_i64().unwrap() as i32
}

async fn add_to_collection(app: &TestApp, u: &TestUser, collection: i32, ep: i32) {
    app.post(&format!("/api/data/collections/{collection}/add_episode"))
        .key(u)
        .json(&json!({ "user_id": u.id, "episode_id": ep, "is_youtube": false }))
        .send()
        .await
        .assert_ok();
}

// ---- saved -------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn saving_and_unsaving_episodes() {
    let app = TestApp::new().await;
    let l = setup(&app).await;
    save(&app, &l.user, l.ep[0]).await;
    save(&app, &l.user, l.ep[2]).await;
    save(&app, &l.user, l.ep[2]).await; // idempotent
    let mut expect = vec![l.ep[0], l.ep[2]];
    expect.sort();
    assert_eq!(saved_ids(&app, &l.user).await, expect);

    let eps = app.episodes(&l.user, l.pod).await;
    let flag = |id: i32| eps.iter().find(|e| e["Episodeid"] == id).unwrap()["saved"].clone();
    assert_eq!(flag(l.ep[0]), true);
    assert_eq!(flag(l.ep[1]), false);

    app.post("/api/data/remove_saved_episode")
        .key(&l.user)
        .json(&json!({ "episode_id": l.ep[0], "user_id": l.user.id, "is_youtube": false }))
        .send()
        .await
        .assert_ok();
    assert_eq!(saved_ids(&app, &l.user).await, [l.ep[2]]);
}

// ---- collections -------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn every_user_has_an_undeletable_default_saved_collection_backed_by_saves() {
    let app = TestApp::new().await;
    let l = setup(&app).await;
    let cols = collections(&app, &l.user).await;
    let default: Vec<&Value> = cols.iter().filter(|c| c["is_default"] == true).collect();
    assert_eq!(default.len(), 1, "exactly one default collection: {cols:#?}");
    let default_id = default[0]["collection_id"].as_i64().unwrap() as i32;

    save(&app, &l.user, l.ep[1]).await;
    assert_eq!(collection_episode_ids(&app, &l.user, default_id).await, [l.ep[1]], "default collection = saved episodes");

    let resp = app.delete(&format!("/api/data/collections/{default_id}")).key(&l.user).send().await;
    assert!(!resp.is_success(), "the default collection must not be deletable: {}", resp.text());
    assert_eq!(collections(&app, &l.user).await.iter().filter(|c| c["is_default"] == true).count(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn custom_collection_lifecycle() {
    let app = TestApp::new().await;
    let l = setup(&app).await;
    let c = create_collection(&app, &l.user, json!({ "name": "Road trip", "description": "long drives" })).await;
    add_to_collection(&app, &l.user, c, l.ep[0]).await;
    add_to_collection(&app, &l.user, c, l.ep[3]).await;
    add_to_collection(&app, &l.user, c, l.ep[3]).await; // no duplicates

    let mut both = vec![l.ep[0], l.ep[3]];
    both.sort();
    assert_eq!(collection_episode_ids(&app, &l.user, c).await, both);
    let col = collections(&app, &l.user).await.into_iter().find(|x| x["collection_id"] == c).unwrap();
    assert_eq!(col["name"], "Road trip");
    assert_eq!(col["episode_count"], 2);
    assert_eq!(col["is_default"], false);

    // Membership lookup used by the "add to collection" picker.
    let m = app
        .get(&format!("/api/data/episode_collections/{}/{}", l.user.id, l.ep[0]))
        .key(&l.user)
        .query("is_youtube", false)
        .send()
        .await
        .ok_json();
    assert!(m["collection_ids"].as_array().unwrap().contains(&json!(c)), "{m}");

    // An episode can live in several collections, and saving is independent.
    let c2 = create_collection(&app, &l.user, json!({ "name": "Favourites" })).await;
    add_to_collection(&app, &l.user, c2, l.ep[0]).await;
    assert!(saved_ids(&app, &l.user).await.is_empty(), "collections are not the saved list");

    app.post(&format!("/api/data/collections/{c}/remove_episode"))
        .key(&l.user)
        .json(&json!({ "user_id": l.user.id, "episode_id": l.ep[0], "is_youtube": false }))
        .send()
        .await
        .assert_ok();
    assert_eq!(collection_episode_ids(&app, &l.user, c).await, [l.ep[3]]);
    assert_eq!(collection_episode_ids(&app, &l.user, c2).await, [l.ep[0]], "other collection untouched");

    app.request(axum::http::Method::PATCH, &format!("/api/data/collections/{c}"))
        .key(&l.user)
        .json(&json!({ "name": "Long drives" }))
        .send()
        .await
        .assert_ok();
    let col = collections(&app, &l.user).await.into_iter().find(|x| x["collection_id"] == c).unwrap();
    assert_eq!(col["name"], "Long drives");
    assert_eq!(col["description"], "long drives", "PATCH keeps fields it wasn't given");

    app.delete(&format!("/api/data/collections/{c}")).key(&l.user).send().await.assert_ok();
    assert!(collections(&app, &l.user).await.iter().all(|x| x["collection_id"] != c));
    assert_eq!(app.episodes(&l.user, l.pod).await.len(), 4, "deleting a collection never deletes episodes");
}

#[tokio::test(flavor = "multi_thread")]
async fn duplicate_collection_names_are_a_clean_client_error() {
    let app = TestApp::new().await;
    let l = setup(&app).await;
    create_collection(&app, &l.user, json!({ "name": "Dup" })).await;
    let resp = app
        .post("/api/data/collections/create")
        .key(&l.user)
        .json(&json!({ "user_id": l.user.id, "name": "Dup" }))
        .send()
        .await;
    let s = resp.status.as_u16();
    assert!((400..500).contains(&s), "duplicate name should be 4xx, got {s}: {}", resp.text());
}

#[tokio::test(flavor = "multi_thread")]
async fn category_collections_backfill_and_auto_add_after_refresh() {
    let app = TestApp::new().await;
    let l = setup(&app).await; // "Library Show" is Technology
    let other_url = app.feed("cooking.xml", &Feed::with_episodes("Cooking Show", 2).categories(&["Food"]));
    let cooking = app.subscribe(&l.user, &other_url, 2).await;
    // Someone else's Technology show stays out of this user's collection.
    let neighbour = app.user("neighbour").await;
    let theirs_url = app.feed("theirs.xml", &Feed::with_episodes("Their Tech Show", 1).categories(&["Technology"]));
    let theirs = app.subscribe(&neighbour, &theirs_url, 1).await;

    let cats = app.get(&format!("/api/data/collections/categories/{}", l.user.id)).key(&l.user).send().await.ok_json();
    let cats = cats["categories"].to_string();
    assert!(cats.contains("Technology") && cats.contains("Food"), "user categories: {cats}");

    let c = create_collection(&app, &l.user, json!({ "name": "Tech", "auto_add_categories": ["Technology"], "backfill": true })).await;
    let mut tech = l.ep.clone();
    tech.sort();
    assert_eq!(collection_episode_ids(&app, &l.user, c).await, tech, "backfill pulls in existing matching episodes");

    // New episodes on both shows; only the Technology one should be auto-added.
    app.feed("lib.xml", &Feed::with_episodes("Library Show", 5).categories(&["Technology"]));
    app.feed("cooking.xml", &Feed::with_episodes("Cooking Show", 3).categories(&["Food"]));
    app.feed("theirs.xml", &Feed::with_episodes("Their Tech Show", 2).categories(&["Technology"]));
    app.refresh().await;

    let new_tech = app.episode_id(&l.user, l.pod, "Episode 5").await;
    let in_col = collection_episode_ids(&app, &l.user, c).await;
    assert!(in_col.contains(&new_tech), "new Technology episode auto-added by the refresh: {in_col:?}");
    let new_food = app.episode_id(&l.user, cooking, "Episode 3").await;
    assert!(!in_col.contains(&new_food), "other categories stay out");
    let stored = int(&app, &format!(r#"SELECT COUNT(*) FROM "CollectionEpisodes" ce JOIN "Episodes" e ON e.EpisodeID = ce.EpisodeID WHERE ce.CollectionID = {c} AND e.PodcastID = {theirs}"#)).await;
    assert_eq!(stored, Some(0), "another user's episodes are never added");
}

// ---- playlists ---------------------------------------------------------------------------

async fn playlists(app: &TestApp, u: &TestUser) -> Value {
    app.get("/api/data/get_playlists").key(u).query("user_id", u.id).send().await.ok_json()
}

fn playlist_list(v: &Value) -> Vec<Value> {
    v["playlists"].as_array().cloned().unwrap_or_else(|| panic!("playlists shape: {v}"))
}

async fn playlist_episode_ids(app: &TestApp, u: &TestUser, playlist: i32) -> Vec<i32> {
    let body = app
        .get("/api/data/get_playlist_episodes")
        .key(u)
        .query("user_id", u.id)
        .query("playlist_id", playlist)
        .query("limit", 100)
        .send()
        .await
        .ok_json();
    ids(&body["episodes"])
}

async fn create_playlist(app: &TestApp, u: &TestUser, overrides: Value) -> i32 {
    let mut body = json!({
        "user_id": u.id,
        "name": "Mine",
        "include_unplayed": true,
        "include_partially_played": true,
        "include_played": false,
        "sort_order": "date_desc",
        "group_by_podcast": false,
        "icon_name": "ph-playlist",
    });
    for (k, v) in overrides.as_object().unwrap() {
        body[k] = v.clone();
    }
    let r = app.post("/api/data/create_playlist").key(u).json(&body).send().await.ok_json();
    r["playlist_id"].as_i64().unwrap() as i32
}

#[tokio::test(flavor = "multi_thread")]
async fn new_users_get_the_default_smart_playlists_which_they_own() {
    let app = TestApp::new().await;
    let l = setup(&app).await;
    let all = playlist_list(&playlists(&app, &l.user).await);
    let mut names: Vec<&str> = all.iter().map(|p| p["name"].as_str().unwrap()).collect();
    names.sort();
    assert_eq!(
        names,
        ["Commuter Mix", "Currently Listening", "Fresh Releases", "Longform", "Quick Listens", "Weekend Marathon"]
    );
    // Defaults are ordinary per-user playlists: the user may delete them.
    let id = all[0]["playlist_id"].as_i64().unwrap();
    app.delete("/api/data/delete_playlist")
        .key(&l.user)
        .json(&json!({ "user_id": l.user.id, "playlist_id": id }))
        .send()
        .await
        .assert_ok();
    assert_eq!(playlist_list(&playlists(&app, &l.user).await).len(), 5);
}

#[tokio::test(flavor = "multi_thread")]
async fn smart_playlist_rules_select_episodes_live() {
    let app = TestApp::new().await;
    let l = setup(&app).await;
    let other_url = app.feed("other.xml", &Feed::with_episodes("Other Show", 2));
    let other = app.subscribe(&l.user, &other_url, 2).await;

    let p = create_playlist(&app, &l.user, json!({ "name": "Unplayed library", "podcast_ids": [l.pod] })).await;
    let mut all4 = l.ep.clone();
    all4.sort();
    assert_eq!(playlist_episode_ids(&app, &l.user, p).await, all4, "only the chosen podcast");

    app.post("/api/data/mark_episode_completed")
        .key(&l.user)
        .json(&json!({ "episode_id": l.ep[0], "user_id": l.user.id, "is_youtube": false }))
        .send()
        .await
        .assert_ok();
    let after = playlist_episode_ids(&app, &l.user, p).await;
    assert!(!after.contains(&l.ep[0]), "played episodes drop out when include_played=false: {after:?}");

    // Durations: min/max_duration are given in *minutes*.
    let lengths = Feed::new("Lengths")
        .episode(Episode::new(1).title("Ten minutes").duration(Some(600)))
        .episode(Episode::new(2).title("One hour").duration(Some(3600)));
    let lengths_url = app.feed("lengths.xml", &lengths);
    let lengths_pod = app.subscribe(&l.user, &lengths_url, 2).await;
    let ten = app.episode_id(&l.user, lengths_pod, "Ten minutes").await;
    let hour = app.episode_id(&l.user, lengths_pod, "One hour").await;
    let long = create_playlist(&app, &l.user, json!({ "name": "Long ones", "min_duration": 45 })).await;
    let got = playlist_episode_ids(&app, &l.user, long).await;
    assert!(got.contains(&hour), "60-minute episode qualifies: {got:?}");
    assert!(!got.contains(&ten), "10-minute episode is too short: {got:?}");
    let other_ep = app.episode_id(&l.user, other, "Episode 1").await;
    assert!(!got.contains(&other_ep), "30-minute episode is too short");

    app.delete("/api/data/delete_playlist")
        .key(&l.user)
        .json(&json!({ "user_id": l.user.id, "playlist_id": p }))
        .send()
        .await
        .assert_ok();
    let left = playlist_list(&playlists(&app, &l.user).await);
    assert!(left.iter().all(|x| x["playlist_id"] != p));
}
