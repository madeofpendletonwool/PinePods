//! Now-playing sessions over the real WebSocket: who is listening where, remote commands, and
//! what happens as devices come and go.

use futures::{SinkExt, StreamExt};
use std::time::Duration;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::Message;

use crate::harness::*;

type Socket = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

struct Device {
    id: String,
    socket: Socket,
}

fn url(port: u16, user: &TestUser, device: &str) -> String {
    format!(
        "ws://127.0.0.1:{port}/ws/api/nowplaying/{}?api_key={}&device_id={device}&device_name={device}-name&device_type=Test",
        user.id, user.api_key
    )
}

impl Device {
    async fn connect(port: u16, user: &TestUser, id: &str) -> Device {
        let (socket, _) = tokio_tungstenite::connect_async(url(port, user, id)).await.expect("connect");
        Device { id: id.to_string(), socket }
    }

    async fn send(&mut self, message: Value) {
        self.socket.send(Message::text(message.to_string())).await.unwrap();
    }

    async fn report(&mut self, episode: i32, position: f64, playing: bool) {
        self.send(json!({
            "type": "report",
            "episode_id": episode,
            "is_youtube": false,
            "title": format!("Episode {episode}"),
            "position_sec": position,
            "duration_sec": 3600,
            "playing": playing,
            "speed": 1.0,
        }))
        .await;
    }

    /// The next server message of `kind` that satisfies `check`, skipping anything else.
    async fn expect(&mut self, kind: &str, check: impl Fn(&Value) -> bool) -> Value {
        let wait = async {
            loop {
                match self.socket.next().await {
                    Some(Ok(Message::Text(text))) => {
                        let value: Value = serde_json::from_str(text.as_str()).unwrap();
                        if value["type"] == kind && check(&value) {
                            return value;
                        }
                    }
                    Some(Ok(_)) => {}
                    other => panic!("{}: socket ended waiting for {kind}: {other:?}", self.id),
                }
            }
        };
        tokio::time::timeout(Duration::from_secs(10), wait)
            .await
            .unwrap_or_else(|_| panic!("{}: timed out waiting for {kind}", self.id))
    }

    async fn expect_devices(&mut self, check: impl Fn(&[Value]) -> bool) -> Vec<Value> {
        let msg = self.expect("devices", |m| check(m["devices"].as_array().unwrap())).await;
        msg["devices"].as_array().unwrap().clone()
    }

    /// The close code the server ended this socket with.
    async fn expect_close(&mut self) -> Option<CloseCode> {
        let wait = async {
            loop {
                match self.socket.next().await {
                    Some(Ok(Message::Close(frame))) => return frame.map(|f| f.code),
                    Some(Ok(_)) => {}
                    Some(Err(e)) => panic!("{}: socket error: {e}", self.id),
                    None => return None,
                }
            }
        };
        tokio::time::timeout(Duration::from_secs(10), wait).await.expect("socket closed")
    }
}

fn has(devices: &[Value], id: &str) -> bool {
    devices.iter().any(|d| d["device_id"] == id)
}

fn find<'a>(devices: &'a [Value], id: &str) -> &'a Value {
    devices.iter().find(|d| d["device_id"] == id).unwrap_or_else(|| panic!("{id} not in {devices:?}"))
}

async fn listed(app: &TestApp, user: &TestUser) -> Vec<Value> {
    let body = app.get("/api/data/now_playing_devices").key(user).query("user_id", user.id).send().await.ok_json();
    body["devices"].as_array().unwrap().clone()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_new_device_is_listed_as_idle_for_the_users_other_devices() {
    let app = TestApp::new().await;
    let port = app.listen().await;
    let user = app.user("listener").await;

    let mut phone = Device::connect(port, &user, "phone").await;
    phone.expect_devices(|d| has(d, "phone")).await;
    let _laptop = Device::connect(port, &user, "laptop").await;

    let devices = phone.expect_devices(|d| has(d, "phone") && has(d, "laptop")).await;
    let laptop = find(&devices, "laptop");
    assert_eq!(laptop["episode_id"], 0, "{laptop}");
    assert_eq!(laptop["device_name"], "laptop-name", "{laptop}");
    assert_eq!(laptop["device_type"], "Test", "{laptop}");
}

#[tokio::test(flavor = "multi_thread")]
async fn what_a_device_plays_reaches_the_others_and_the_http_list() {
    let app = TestApp::new().await;
    let port = app.listen().await;
    let user = app.user("listener").await;
    let mut phone = Device::connect(port, &user, "phone").await;
    let mut laptop = Device::connect(port, &user, "laptop").await;
    laptop.expect_devices(|d| has(d, "phone") && has(d, "laptop")).await;

    phone.report(42, 812.5, true).await;

    let devices = laptop.expect_devices(|d| d.iter().any(|x| x["device_id"] == "phone" && x["episode_id"] == 42)).await;
    let playing = find(&devices, "phone");
    assert_eq!(playing["position_sec"], 812.5, "{playing}");
    assert_eq!(playing["playing"], true, "{playing}");
    assert_eq!(playing["title"], "Episode 42", "{playing}");

    let over_http = listed(&app, &user).await;
    assert_eq!(find(&over_http, "phone")["episode_id"], 42, "{over_http:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn commands_reach_the_target_device_and_are_acknowledged() {
    let app = TestApp::new().await;
    let port = app.listen().await;
    let user = app.user("listener").await;
    let mut phone = Device::connect(port, &user, "phone").await;
    let mut laptop = Device::connect(port, &user, "laptop").await;
    laptop.expect_devices(|d| has(d, "phone") && has(d, "laptop")).await;

    laptop.send(json!({
        "type": "command",
        "target_device_id": "phone",
        "action": "seek",
        "args": { "position_sec": 120 },
        "cmd_id": "c-1",
    }))
    .await;

    let command = phone.expect("command", |_| true).await;
    assert_eq!(command["from_device_id"], "laptop", "{command}");
    assert_eq!(command["action"], "seek", "{command}");
    assert_eq!(command["args"]["position_sec"], 120, "{command}");

    let ack = laptop.expect("ack", |_| true).await;
    assert_eq!(ack["ok"], true, "{ack}");
    assert_eq!(ack["cmd_id"], "c-1", "{ack}");
    assert_eq!(ack["target_device_id"], "phone", "{ack}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_command_for_a_missing_device_is_reported_as_failed() {
    let app = TestApp::new().await;
    let port = app.listen().await;
    let user = app.user("listener").await;
    let mut laptop = Device::connect(port, &user, "laptop").await;

    laptop.send(json!({ "type": "command", "target_device_id": "gone", "action": "pause", "args": {}, "cmd_id": "c-2" })).await;

    let ack = laptop.expect("ack", |_| true).await;
    assert_eq!(ack["ok"], false, "{ack}");
    assert_eq!(ack["cmd_id"], "c-2", "{ack}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_device_that_disconnects_drops_off_the_list() {
    let app = TestApp::new().await;
    let port = app.listen().await;
    let user = app.user("listener").await;
    let mut laptop = Device::connect(port, &user, "laptop").await;
    let phone = Device::connect(port, &user, "phone").await;
    laptop.expect_devices(|d| has(d, "phone")).await;

    drop(phone);

    laptop.expect_devices(|d| !has(d, "phone") && has(d, "laptop")).await;
    eventually("gone from the HTTP list too", || async {
        (!has(&listed(&app, &user).await, "phone")).then_some(())
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn reconnecting_with_the_same_id_replaces_the_old_socket() {
    let app = TestApp::new().await;
    let port = app.listen().await;
    let user = app.user("listener").await;
    let mut old = Device::connect(port, &user, "tab").await;
    old.expect_devices(|d| has(d, "tab")).await;

    let mut new = Device::connect(port, &user, "tab").await;

    assert_eq!(old.expect_close().await, Some(CloseCode::from(4000)));
    new.expect_devices(|d| has(d, "tab")).await;
    let devices = listed(&app, &user).await;
    assert_eq!(devices.iter().filter(|d| d["device_id"] == "tab").count(), 1, "{devices:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn devices_of_different_users_never_see_each_other() {
    let app = TestApp::new().await;
    let port = app.listen().await;
    let alice = app.user("alice").await;
    let bob = app.user("bob").await;
    let mut alices = Device::connect(port, &alice, "alice-phone").await;
    alices.expect_devices(|d| has(d, "alice-phone")).await;
    let mut bobs = Device::connect(port, &bob, "bob-phone").await;
    bobs.expect_devices(|d| has(d, "bob-phone")).await;
    bobs.report(7, 1.0, true).await;

    alices.send(json!({ "type": "command", "target_device_id": "bob-phone", "action": "pause", "args": {} })).await;

    let ack = alices.expect("ack", |_| true).await;
    assert_eq!(ack["ok"], false, "another user's device can't be targeted: {ack}");
    let devices = listed(&app, &alice).await;
    assert!(!has(&devices, "bob-phone"), "{devices:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_socket_needs_the_users_own_key() {
    let app = TestApp::new().await;
    let port = app.listen().await;
    let alice = app.user("alice").await;
    let mut bob = app.user("bob").await;

    bob.id = alice.id;
    assert!(tokio_tungstenite::connect_async(url(port, &bob, "spy")).await.is_err(), "someone else's user id");

    bob.api_key = "not-a-real-key".to_string();
    assert!(tokio_tungstenite::connect_async(url(port, &bob, "spy")).await.is_err(), "an invalid key");
}

#[tokio::test(flavor = "multi_thread")]
async fn adding_to_the_queue_lands_under_what_another_device_is_playing() {
    let app = TestApp::new().await;
    let port = app.listen().await;
    let user = app.user("listener").await;
    let pod = app.subscribe(&user, &app.feed("show.xml", &Feed::with_episodes("Show", 3)), 3).await;
    let first = app.episode_id(&user, pod, "Episode 1").await;
    let second = app.episode_id(&user, pod, "Episode 2").await;
    let third = app.episode_id(&user, pod, "Episode 3").await;
    for episode in [first, second] {
        app.post("/api/data/queue_pod")
            .key(&user)
            .json(&json!({ "episode_id": episode, "user_id": user.id, "is_youtube": false }))
            .send()
            .await
            .assert_ok();
    }
    let order = |body: Value| -> Vec<i64> {
        body["data"].as_array().unwrap().iter().map(|e| e["episodeid"].as_i64().unwrap()).collect()
    };
    let queue = app.get("/api/data/get_queued_episodes").key(&user).query("user_id", user.id).send().await.ok_json();
    let top = order(queue)[0] as i32;

    let mut phone = Device::connect(port, &user, "phone").await;
    phone.report(top, 10.0, true).await;
    eventually("the session is known", || async {
        listed(&app, &user).await.iter().any(|d| d["episode_id"] == top).then_some(())
    })
    .await;

    app.post("/api/data/queue_pod")
        .key(&user)
        .json(&json!({ "episode_id": third, "user_id": user.id, "is_youtube": false, "position": "top" }))
        .send()
        .await
        .assert_ok();

    let queue = app.get("/api/data/get_queued_episodes").key(&user).query("user_id", user.id).send().await.ok_json();
    let queue = order(queue);
    assert_eq!(queue[0] as i32, top, "the playing episode stays on top: {queue:?}");
    assert_eq!(queue[1] as i32, third, "the new one plays next: {queue:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn task_progress_reaches_the_users_socket() {
    let app = TestApp::new().await;
    let port = app.listen().await;
    let user = app.user("listener").await;
    let url = format!("ws://127.0.0.1:{port}/ws/api/tasks/{}?api_key={}", user.id, user.api_key);
    let (mut socket, _) = tokio_tungstenite::connect_async(url).await.expect("connect");

    let task = app.state.task_manager.create_task("test_job".to_string(), user.id).await.unwrap();
    app.state.task_manager.update_task_progress(&task, 40.0, Some("working".to_string())).await.unwrap();

    let seen = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            match socket.next().await {
                Some(Ok(Message::Text(text))) if text.as_str().contains(&task) => return text.to_string(),
                Some(Ok(_)) => {}
                other => panic!("socket ended: {other:?}"),
            }
        }
    })
    .await
    .expect("a progress message");
    let value: Value = serde_json::from_str(&seen).unwrap();
    assert!(!value.is_null(), "{seen}");
}
