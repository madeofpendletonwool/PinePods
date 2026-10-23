//! A sync app that knows a show under another address (it followed a feed move, or writes the
//! address differently) never makes Pinepods subscribe to it twice: the address is remembered
//! as an alias of the podcast, and removing it under that address unsubscribes the podcast.

use crate::harness::gpodder::*;
use crate::harness::*;

async fn scene() -> GpodderScene {
    let app = TestApp::new().await;
    let user = app.user("moved").await;
    let server = app.rust_gpodder_server().await;
    let password = app.enable_gpodder(&server, &user).await;
    GpodderScene { server, app, user, password }
}

async fn feed_urls(s: &GpodderScene) -> Vec<String> {
    s.app.podcasts(&s.user).await.iter().filter_map(|p| p["feedurl"].as_str().map(str::to_string)).collect()
}

async fn aliases(s: &GpodderScene) -> i64 {
    int(&s.app, &format!(r#"SELECT COUNT(*) FROM "PodcastFeedAliases" WHERE UserID = {}"#, s.user.id)).await.unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_moved_feed_is_not_subscribed_twice_and_unsubscribes_under_its_new_address() {
    let s = scene().await;
    let old = s.app.feed("moved-old.xml", &Feed::with_episodes("Moved Show", 3));
    s.app.subscribe(&s.user, &old, 3).await;
    // The new home has the same episodes and one more.
    let new = s.app.feed("moved-new.xml", &Feed::with_episodes("Moved Show", 4));

    let phone = s.device("phone").await;
    phone.upload_subscription_changes(&[&new], &[]).await.assert_ok();
    eventually("the new address remembered", || async { (aliases(&s).await == 1).then_some(()) }).await;
    assert_eq!(feed_urls(&s).await, vec![old.clone()], "still one podcast, at its own address");

    phone.upload_subscription_changes(&[], &[&new]).await.assert_ok();
    eventually("unsubscribed through the new address", || async { feed_urls(&s).await.is_empty().then_some(()) }).await;
    assert_eq!(aliases(&s).await, 0, "aliases go with the podcast");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_same_address_written_differently_is_not_imported() {
    let s = scene().await;
    let feed = s.app.feed("written.xml", &Feed::with_episodes("Written", 1));
    s.app.subscribe(&s.user, &feed, 1).await;
    let other = s.app.feed("other.xml", &Feed::with_episodes("Other", 1));

    let phone = s.device("phone").await;
    let variant = format!("{}/", feed.replace("http://", "HTTP://"));
    phone.upload_subscription_changes(&[&variant, &other], &[]).await.assert_ok();
    eventually("the other feed imported", || async { feed_urls(&s).await.contains(&other).then_some(()) }).await;
    assert_eq!(feed_urls(&s).await.len(), 2, "the variant isn't a new podcast");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_feed_with_the_same_podcast_guid_is_the_same_show() {
    let s = scene().await;
    let guid = "<podcast:guid>8d2c3a1e-0000-4000-8000-000000000077</podcast:guid>";
    let old = s.app.feed("guid-old.xml", &Feed::with_episodes("Guid Show", 1).extra(guid));
    let pod = s.app.subscribe(&s.user, &old, 1).await;
    eventually("Podcasting 2.0 data stored", || async { s.app.feed_tags_stored(pod).await.then_some(()) }).await;
    // Renamed, and no episodes in common.
    let mut renamed = Feed::new("Guid Show (new home)").extra(guid);
    renamed = renamed.episode(Episode::new(50));
    let new = s.app.feed("guid-new.xml", &renamed);

    let phone = s.device("phone").await;
    phone.upload_subscription_changes(&[&new], &[]).await.assert_ok();
    eventually("the new address remembered", || async { (aliases(&s).await == 1).then_some(()) }).await;
    assert_eq!(feed_urls(&s).await, vec![old]);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_different_show_with_the_same_title_is_still_added() {
    let s = scene().await;
    let first = s.app.feed("same-title-a.xml", &Feed::with_episodes("Daily News", 2));
    s.app.subscribe(&s.user, &first, 2).await;
    let mut other = Feed::new("Daily News");
    for n in 100..103 {
        other = other.episode(Episode::new(n));
    }
    let second = s.app.feed("same-title-b.xml", &other);

    let phone = s.device("phone").await;
    phone.upload_subscription_changes(&[&second], &[]).await.assert_ok();
    eventually("the second show imported", || async { (feed_urls(&s).await.len() == 2).then_some(()) }).await;
    assert_eq!(aliases(&s).await, 0);
}

/// Upgrading merges podcasts already subscribed twice: one podcast, no duplicate episodes, the
/// copy's listening history kept, the address that published last kept and the other remembered.
#[tokio::test(flavor = "multi_thread")]
async fn podcasts_subscribed_twice_are_merged_on_upgrade() {
    let app = TestApp::new().await;
    let user = app.user("merge").await;
    let old = app.feed("merge-old.xml", &Feed::with_episodes("Merged Show", 3));
    let new = app.feed("merge-new.xml", &Feed::with_episodes("Merged Show", 4));
    let kept = app.subscribe(&user, &old, 3).await;
    let copy = app.subscribe(&user, &new, 4).await;
    let unrelated = app.subscribe(&user, &app.feed("merge-other.xml", &Feed::with_episodes("Other Show", 2)), 2).await;
    let (copy_ep2, copy_ep4) = (app.episode_id(&user, copy, "Episode 2").await, app.episode_id(&user, copy, "Episode 4").await);
    for (ep, secs) in [(copy_ep2, 120), (copy_ep4, 300)] {
        exec(&app, &format!(
            r#"INSERT INTO "UserEpisodeHistory" (UserID, EpisodeID, ListenDate, ListenDuration) VALUES ({}, {ep}, '2026-01-01 00:00:00', {secs})"#,
            user.id
        ))
        .await;
    }

    rerun_migrations(&app, &["077"]).await;

    let ids: Vec<i64> = app.podcasts(&user).await.iter().filter_map(|p| p["podcastid"].as_i64()).collect();
    assert_eq!(ids.len(), 2, "{ids:?}");
    assert!(ids.contains(&(kept as i64)) && ids.contains(&(unrelated as i64)));
    assert_eq!(int(&app, &format!(r#"SELECT COUNT(*) FROM "Episodes" WHERE PodcastID = {kept}"#)).await, Some(4));
    assert_eq!(
        int(&app, &format!(r#"SELECT COUNT(DISTINCT EpisodeGUID) FROM "Episodes" WHERE PodcastID = {kept}"#)).await,
        Some(4),
        "no episode twice"
    );
    let listened = |title: &'static str| {
        let sql = format!(
            r#"SELECT h.ListenDuration FROM "UserEpisodeHistory" h JOIN "Episodes" e ON e.EpisodeID = h.EpisodeID
               WHERE h.UserID = {} AND e.PodcastID = {kept} AND e.EpisodeTitle = '{title}'"#,
            user.id
        );
        let app = &app;
        async move { int(app, &sql).await }
    };
    assert_eq!(listened("Episode 2").await, Some(120), "merged into the original episode");
    assert_eq!(listened("Episode 4").await, Some(300), "moved over with the episode only the copy had");
    // The copy published last, so its address is kept and the old one is the alias.
    assert_eq!(text(&app, &format!(r#"SELECT FeedURL FROM "Podcasts" WHERE PodcastID = {kept}"#)).await, Some(new));
    assert_eq!(text(&app, &format!(r#"SELECT FeedURL FROM "PodcastFeedAliases" WHERE PodcastID = {kept}"#)).await, Some(old));
}
