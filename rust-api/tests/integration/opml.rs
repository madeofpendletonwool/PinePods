//! OPML: importing a subscription list and exporting it back.

use crate::harness::*;

async fn import(app: &TestApp, u: &TestUser, urls: &[String]) {
    let body = app
        .post("/api/data/import_opml")
        .key(u)
        .json(&json!({ "user_id": u.id, "podcasts": urls }))
        .send()
        .await
        .ok_json();
    assert_eq!(body["success"], true, "{body}");
}

async fn wait_for_podcasts(app: &TestApp, u: &TestUser, n: usize) -> Vec<Value> {
    eventually(&format!("{n} imported podcasts"), || async {
        let pods = app.podcasts(u).await;
        (pods.len() >= n).then_some(pods)
    })
    .await
}

fn xml_urls(opml: &str) -> Vec<String> {
    let mut urls: Vec<String> = opml
        .split("xmlUrl=\"")
        .skip(1)
        .map(|rest| rest.split('"').next().unwrap().replace("&amp;", "&"))
        .collect();
    urls.sort();
    urls
}

#[tokio::test(flavor = "multi_thread")]
async fn importing_an_opml_list_subscribes_to_every_working_feed() {
    let app = TestApp::new().await;
    let user = app.user("importer").await;
    let a = app.feed("one.xml", &Feed::with_episodes("Imported One", 2));
    let b = app.feed("two.xml", &Feed::with_episodes("Imported Two", 3));
    let broken = app.fixtures().serve("gone.xml", Fixture::new("text/plain", "nope").status(404));

    import(&app, &user, &[a.clone(), broken, b.clone()]).await;
    let pods = wait_for_podcasts(&app, &user, 2).await;
    let mut names: Vec<&str> = pods.iter().map(|p| p["podcastname"].as_str().unwrap()).collect();
    names.sort();
    assert_eq!(names, ["Imported One", "Imported Two"], "the broken feed is skipped, the rest still import");

    // Episodes arrive with the subscription.
    let two = pods.iter().find(|p| p["podcastname"] == "Imported Two").unwrap()["podcastid"].as_i64().unwrap() as i32;
    eventually("imported episodes", || async { (app.episodes(&user, two).await.len() == 3).then_some(()) }).await;

    // Progress is cleared once the import is done.
    let progress = eventually("import finished", || async {
        let p = app.get(&format!("/api/data/import_progress/{}", user.id)).key(&user).send().await.ok_json();
        (p["total"] == 0).then_some(p)
    })
    .await;
    assert_eq!(progress["current"], 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn re_importing_a_feed_you_already_have_does_not_duplicate_it() {
    let app = TestApp::new().await;
    let user = app.user("importer").await;
    let a = app.feed("dupe.xml", &Feed::with_episodes("Already Here", 2));
    let pod = app.subscribe(&user, &a, 2).await;

    let b = app.feed("new.xml", &Feed::with_episodes("Brand New", 1));
    import(&app, &user, &[a, b]).await;
    wait_for_podcasts(&app, &user, 2).await;
    eventually("import finished", || async {
        let p = app.get(&format!("/api/data/import_progress/{}", user.id)).key(&user).send().await.ok_json();
        (p["total"] == 0).then_some(())
    })
    .await;

    let pods = app.podcasts(&user).await;
    assert_eq!(pods.len(), 2, "{pods:#?}");
    assert_eq!(app.episodes(&user, pod).await.len(), 2, "existing subscription untouched");
}

#[tokio::test(flavor = "multi_thread")]
async fn exported_opml_lists_every_subscription_and_round_trips() {
    let app = TestApp::new().await;
    let alice = app.user("alice").await;
    let a = app.feed("a.xml", &Feed::with_episodes("Rock & Roll <Live>", 1));
    // The fixture server routes on the path, so the query string just has to survive escaping.
    let b = format!("{}?x=1&y=2", app.feed("b.xml", &Feed::with_episodes("Query Feed", 1)));
    app.subscribe(&alice, &a, 1).await;
    app.subscribe(&alice, &b, 1).await;

    let export = |u: &TestUser| app.post("/api/data/backup_user").key(u).json(&json!({ "user_id": u.id }));
    let opml = export(&alice).send().await.text();
    assert!(opml.starts_with("<?xml") && opml.contains("<opml"), "{opml}");
    assert!(opml.contains("Rock &amp; Roll &lt;Live&gt;"), "names are XML-escaped:\n{opml}");
    let mut expected = vec![a.clone(), b.clone()];
    expected.sort();
    assert_eq!(xml_urls(&opml), expected);

    // Someone else can't export alice's list.
    let bob = app.user("bob").await;
    app.post("/api/data/backup_user").key(&bob).json(&json!({ "user_id": alice.id })).send().await.assert_status(403);

    // Importing the export into a fresh account reproduces the subscriptions.
    import(&app, &bob, &xml_urls(&opml)).await;
    let pods = wait_for_podcasts(&app, &bob, 2).await;
    let mut feeds: Vec<String> = pods.iter().map(|p| p["feedurl"].as_str().unwrap().to_string()).collect();
    feeds.sort();
    assert_eq!(feeds, expected);
}

#[tokio::test(flavor = "multi_thread")]
async fn users_cannot_import_into_someone_elses_account() {
    let app = TestApp::new().await;
    let alice = app.user("alice").await;
    let mallory = app.user("mallory").await;
    let url = app.feed("spam.xml", &Feed::with_episodes("Spam", 1));
    app.post("/api/data/import_opml")
        .key(&mallory)
        .json(&json!({ "user_id": alice.id, "podcasts": [url] }))
        .send()
        .await
        .assert_status(403);
    app.get(&format!("/api/data/import_progress/{}", alice.id)).key(&mallory).send().await.assert_status(403);
    assert!(app.podcasts(&alice).await.is_empty());
}
