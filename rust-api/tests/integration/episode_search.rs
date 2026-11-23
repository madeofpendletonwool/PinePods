//! Searching one podcast's episodes: every term must match, title matches come first, and
//! descriptions are matched on their visible words, not their links or markup.

use crate::harness::*;

/// A show whose every description links the same pages, plus the one episode about them.
fn cube_show() -> Feed {
    let footer = r#"<p>Our cubes: <a href="https://cubecobra.com/cube/overview/100-ornithopters">list</a>
        https://cubecobra.com/cube/list/vintage-ornithopters</p>"#;
    let mut feed = Feed::new("Cube Talk");
    for n in 1..=12 {
        feed = feed.episode(Episode::new(n).description(&format!("<p>Episode {n} chat.</p>{footer}")));
    }
    feed.episode(
        Episode::new(13).title("Drafting the 100 Ornithopters cube").description(&format!("<p>All about it.</p>{footer}")),
    )
    .episode(Episode::new(14).title("Mailbag").description("<p>We revisit the <b>100 Ornithopters</b> list and 50% off_sale!</p>"))
    .episode(Episode::new(15).title("Ornithopters everywhere").description("<p>Episode 100 is next.</p>"))
}

async fn search(app: &TestApp, user: &TestUser, pod: i32, term: &str, limit: i32, offset: i32) -> Value {
    app.get("/api/data/podcast_episodes")
        .key(user)
        .query("user_id", user.id)
        .query("podcast_id", pod)
        .query("limit", limit)
        .query("offset", offset)
        .query("search", term)
        .send()
        .await
        .ok_json()
}

fn titles(body: &Value) -> Vec<String> {
    body["episodes"].as_array().unwrap().iter().map(|e| e["Episodetitle"].as_str().unwrap().to_string()).collect()
}

fn ranks(body: &Value) -> Vec<i64> {
    body["episodes"].as_array().unwrap().iter().map(|e| e["match_rank"].as_i64().unwrap()).collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn title_matches_come_first_and_links_are_not_searched() {
    let app = TestApp::new().await;
    let user = app.user("cuber").await;
    let url = app.feed("cube.xml", &cube_show());
    let pod = app.subscribe(&user, &url, 15).await;

    // Links in every description would match all 15 episodes; only the visible words count.
    let body = search(&app, &user, pod, "100 ornithopters", 50, 0).await;
    assert_eq!(body["total"], 3);
    assert_eq!(titles(&body), ["Drafting the 100 Ornithopters cube", "Ornithopters everywhere", "Mailbag"]);
    assert_eq!(ranks(&body), [0, 1, 2]);

    // A quoted phrase stays together; words don't have to be adjacent otherwise.
    assert_eq!(titles(&search(&app, &user, pod, "\"ornithopters cube\"", 50, 0).await), ["Drafting the 100 Ornithopters cube"]);
    assert_eq!(search(&app, &user, pod, "drafting cube", 50, 0).await["total"], 1);
    assert_eq!(search(&app, &user, pod, "cubecobra", 50, 0).await["total"], 0);
    assert_eq!(search(&app, &user, pod, "href", 50, 0).await["total"], 0);

    // Every term must match somewhere.
    assert_eq!(search(&app, &user, pod, "ornithopters nonsense", 50, 0).await["total"], 0);

    // LIKE wildcards are literal.
    assert_eq!(titles(&search(&app, &user, pod, "50%", 50, 0).await), ["Mailbag"]);
    assert_eq!(titles(&search(&app, &user, pod, "off_sale", 50, 0).await), ["Mailbag"]);
    assert_eq!(search(&app, &user, pod, "o_f", 50, 0).await["total"], 0);

    // No search: no rank.
    let all = search(&app, &user, pod, "", 50, 0).await;
    assert_eq!(all["total"], 15);
    assert!(all["episodes"][0].get("match_rank").is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn ranked_results_page_without_repeats() {
    let app = TestApp::new().await;
    let user = app.user("pager").await;
    let mut feed = Feed::new("Paged");
    for n in 1..=30 {
        let ep = Episode::new(n);
        feed = feed.episode(if n % 3 == 0 { ep.title(&format!("Topic {n}")) } else { ep.description("<p>mentions topic</p>") });
    }
    let url = app.feed("paged.xml", &feed);
    let pod = app.subscribe(&user, &url, 30).await;

    let mut seen = Vec::new();
    let mut all_ranks = Vec::new();
    for offset in (0..30).step_by(7) {
        let body = search(&app, &user, pod, "topic", 7, offset).await;
        assert_eq!(body["total"], 30);
        seen.extend(titles(&body));
        all_ranks.extend(ranks(&body));
    }
    let mut unique = seen.clone();
    unique.sort();
    unique.dedup();
    assert_eq!((seen.len(), unique.len()), (30, 30));
    assert_eq!(all_ranks.iter().filter(|&&r| r == 0).count(), 10);
    assert!(all_ranks.windows(2).all(|w| w[0] <= w[1]), "title matches first: {all_ranks:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn episodes_stored_before_the_search_text_get_it_filled() {
    let app = TestApp::new().await;
    let user = app.user("upgrader").await;
    let url = app.feed("cube.xml", &cube_show());
    let pod = app.subscribe(&user, &url, 15).await;

    assert_eq!(
        text(&app, r#"SELECT descriptiontext FROM "Episodes" WHERE episodetitle = 'Mailbag'"#).await.as_deref(),
        Some("We revisit the 100 Ornithopters list and 50% off_sale!")
    );

    // Rows from before the column existed: search falls back to the raw description, links and all.
    exec(&app, r#"UPDATE "Episodes" SET descriptiontext = NULL"#).await;
    assert_eq!(search(&app, &user, pod, "cubecobra", 50, 0).await["total"], 13);

    let filled = app.db().backfill_description_text().await.unwrap();
    assert!(filled >= 15, "filled {filled}");
    assert_eq!(int(&app, r#"SELECT COUNT(*) FROM "Episodes" WHERE descriptiontext IS NULL"#).await, Some(0));
    assert_eq!(search(&app, &user, pod, "cubecobra", 50, 0).await["total"], 0);
    assert_eq!(search(&app, &user, pod, "100 ornithopters", 50, 0).await["total"], 3);
}
