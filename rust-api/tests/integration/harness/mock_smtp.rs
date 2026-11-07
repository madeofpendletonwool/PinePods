//! A minimal SMTP server on a real socket that records what it is sent.

use std::sync::{Arc, Mutex};

use base64::Engine as _;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[derive(Clone, Debug, Default)]
pub struct SentMail {
    pub from: String,
    pub to: Vec<String>,
    pub data: String,
    /// `user\0password` from AUTH PLAIN, when the client signed in.
    pub auth: Option<String>,
}

impl SentMail {
    /// The value of a header, ignoring case.
    pub fn header(&self, name: &str) -> Option<String> {
        let prefix = format!("{}:", name.to_lowercase());
        self.data
            .lines()
            .take_while(|l| !l.is_empty())
            .find(|l| l.to_lowercase().starts_with(&prefix))
            .map(|l| l[prefix.len()..].trim().to_string())
    }

    /// The body with its transfer encoding removed.
    pub fn body(&self) -> String {
        let raw = self.data.split_once("\r\n\r\n").map_or("", |(_, b)| b);
        match self.header("Content-Transfer-Encoding").as_deref() {
            Some("base64") => {
                let joined: String = raw.split_whitespace().collect();
                let bytes = base64::engine::general_purpose::STANDARD.decode(joined).expect("base64 body");
                String::from_utf8_lossy(&bytes).into_owned()
            }
            Some("quoted-printable") => {
                let soft = raw.replace("=\r\n", "");
                let bytes = soft.as_bytes();
                let mut out = Vec::with_capacity(bytes.len());
                let mut i = 0;
                while i < bytes.len() {
                    if bytes[i] == b'=' && i + 2 < bytes.len() {
                        let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
                        if let Ok(b) = u8::from_str_radix(hex, 16) {
                            out.push(b);
                            i += 3;
                            continue;
                        }
                    }
                    out.push(bytes[i]);
                    i += 1;
                }
                String::from_utf8_lossy(&out).into_owned()
            }
            _ => raw.to_string(),
        }
    }
}

pub struct MockSmtp {
    pub port: u16,
    sent: Arc<Mutex<Vec<SentMail>>>,
    task: tokio::task::JoinHandle<()>,
}

impl MockSmtp {
    pub async fn start() -> MockSmtp {
        Self::with_auth_result(true).await
    }

    /// A server that turns every sign-in away.
    pub async fn rejecting_logins() -> MockSmtp {
        Self::with_auth_result(false).await
    }

    async fn with_auth_result(accept_auth: bool) -> MockSmtp {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let sent: Arc<Mutex<Vec<SentMail>>> = Arc::default();
        let log = sent.clone();
        let task = tokio::spawn(async move {
            while let Ok((socket, _)) = listener.accept().await {
                tokio::spawn(session(socket, log.clone(), accept_auth));
            }
        });
        MockSmtp { port, sent, task }
    }

    pub fn sent(&self) -> Vec<SentMail> {
        self.sent.lock().unwrap().clone()
    }
}

impl Drop for MockSmtp {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn session(socket: tokio::net::TcpStream, log: Arc<Mutex<Vec<SentMail>>>, accept_auth: bool) {
    let (read, mut write) = socket.into_split();
    let mut lines = BufReader::new(read).lines();
    let mut mail = SentMail::default();
    let _ = write.write_all(b"220 mock ESMTP\r\n").await;
    while let Ok(Some(line)) = lines.next_line().await {
        let upper = line.to_uppercase();
        let reply: &[u8] = if upper.starts_with("EHLO") || upper.starts_with("HELO") {
            b"250-mock\r\n250 AUTH PLAIN LOGIN\r\n"
        } else if upper.starts_with("AUTH PLAIN") {
            if !accept_auth {
                b"535 5.7.8 authentication credentials invalid\r\n"
            } else {
                let encoded = line.split_whitespace().nth(2).unwrap_or("");
                let decoded = base64::engine::general_purpose::STANDARD.decode(encoded).unwrap_or_default();
                let text = String::from_utf8_lossy(&decoded);
                mail.auth = Some(text.trim_start_matches('\0').to_string());
                b"235 2.7.0 ok\r\n"
            }
        } else if upper.starts_with("MAIL FROM") {
            mail.from = address(&line);
            b"250 ok\r\n"
        } else if upper.starts_with("RCPT TO") {
            mail.to.push(address(&line));
            b"250 ok\r\n"
        } else if upper == "DATA" {
            let _ = write.write_all(b"354 go ahead\r\n").await;
            let mut data = String::new();
            while let Ok(Some(l)) = lines.next_line().await {
                if l == "." {
                    break;
                }
                data.push_str(l.strip_prefix('.').filter(|_| l.starts_with("..")).unwrap_or(&l));
                data.push_str("\r\n");
            }
            mail.data = data;
            log.lock().unwrap().push(std::mem::take(&mut mail));
            b"250 queued\r\n"
        } else if upper == "QUIT" {
            let _ = write.write_all(b"221 bye\r\n").await;
            return;
        } else {
            b"250 ok\r\n"
        };
        if write.write_all(reply).await.is_err() {
            return;
        }
    }
}

fn address(line: &str) -> String {
    line.split('<').nth(1).and_then(|s| s.split('>').next()).unwrap_or("").to_string()
}
