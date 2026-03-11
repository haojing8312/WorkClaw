mod helpers;

use runtime_lib::commands::clawhub::search_clawhub_skills;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

async fn spawn_clawhub_search_server() -> (String, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind mock clawhub server");
    let addr = listener.local_addr().expect("local addr");
    let handle = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("accept");
        let mut buf = vec![0u8; 32 * 1024];
        let n = socket.read(&mut buf).await.expect("read request");
        let raw = String::from_utf8_lossy(&buf[..n]).to_string();
        let request_line = raw.lines().next().unwrap_or_default().to_string();

        let body = if request_line.contains("/api/v1/skills?")
            && request_line.contains("q=superpower")
            && request_line.contains("nonSuspicious=true")
        {
            r#"{"items":[{"slug":"superpowers-mode","displayName":"Superpowers Mode","summary":"Enable strict engineering workflow","stats":{"stars":1,"downloads":1100}}]}"#
                .to_string()
        } else {
            r#"{"items":[]}"#.to_string()
        };

        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        socket
            .write_all(response.as_bytes())
            .await
            .expect("write response");
    });
    (format!("http://{}", addr), handle)
}

#[tokio::test]
async fn search_clawhub_skills_uses_library_query_when_search_api_returns_empty() {
    let (_pool, _tmp) = helpers::setup_test_db().await;
    let (base, handle) = spawn_clawhub_search_server().await;
    std::env::set_var("CLAWHUB_API_BASE", &base);

    let items = search_clawhub_skills("superpower".to_string(), Some(1), Some(10))
        .await
        .expect("search results");

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].slug, "superpowers-mode");
    assert_eq!(items[0].name, "Superpowers Mode");

    handle.await.expect("mock server task");
}
