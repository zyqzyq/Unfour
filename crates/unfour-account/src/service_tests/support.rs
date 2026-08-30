use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::Barrier;
use tokio::task::JoinHandle;

pub(super) const TOKEN: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-abcde";

pub(super) struct TestApi {
    pub url: String,
    task: JoinHandle<Vec<String>>,
}

impl Drop for TestApi {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl TestApi {
    pub async fn start(responses: Vec<(u16, Value)>, gate: Option<Arc<Barrier>>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            let mut requests = Vec::new();
            for (index, (status, body)) in responses.into_iter().enumerate() {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut request = Vec::new();
                loop {
                    let mut chunk = [0; 4096];
                    let count = stream.read(&mut chunk).await.unwrap();
                    assert_ne!(count, 0, "request closed before its body arrived");
                    request.extend_from_slice(&chunk[..count]);
                    if let Some(end) = request.windows(4).position(|part| part == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&request[..end]);
                        let length = headers
                            .lines()
                            .find_map(|line| {
                                let (key, value) = line.split_once(':')?;
                                key.eq_ignore_ascii_case("content-length")
                                    .then(|| value.trim().parse::<usize>().unwrap())
                            })
                            .unwrap_or(0);
                        if request.len() >= end + 4 + length {
                            break;
                        }
                    }
                }
                requests.push(String::from_utf8(request).unwrap());
                if index == 0 {
                    if let Some(gate) = &gate {
                        gate.wait().await;
                        gate.wait().await;
                    }
                }
                let body = body.to_string();
                stream.write_all(format!(
                    "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()
                ).as_bytes()).await.unwrap();
            }
            requests
        });
        Self { url, task }
    }

    pub async fn finish(mut self) -> Vec<String> {
        tokio::time::timeout(Duration::from_secs(5), &mut self.task)
            .await
            .expect("expected HTTP requests were not received")
            .unwrap()
    }

    pub fn service(&self) -> AccountService {
        AccountService::with_secret_store(
            &self.url,
            "https://web.example.test",
            true,
            unfour_secret_store::SecretStore::in_memory("account-service-tests"),
        )
        .unwrap()
    }
}

pub(super) fn profile(status: &str) -> Value {
    json!({
        "id": "account-a", "email": "account-a@example.test",
        "username": null, "displayName": null, "avatarUrl": null,
        "entitlements": [{"code": "cloud_sync", "status": status, "validUntil": null}],
        "devices": []
    })
}

pub(super) fn session_response() -> Value {
    json!({"sessionToken": TOKEN, "expiresAt": "2099-01-01T00:00:00Z", "account": profile("active")})
}

pub(super) async fn save_session(service: &AccountService) {
    service
        .sessions
        .save(StoredSession {
            session_token: TOKEN.into(),
            expires_at: time::OffsetDateTime::now_utc() + time::Duration::days(1),
        })
        .await
        .unwrap();
}

pub(super) async fn callback(service: &AccountService) -> String {
    let url = service.begin_sign_in().await.unwrap();
    let state = url
        .query_pairs()
        .find(|(key, _)| key == "state")
        .unwrap()
        .1
        .into_owned();
    format!(
        "{AUTH_CALLBACK_URI}?authorizationCode={}&state={state}",
        "C".repeat(43)
    )
}
