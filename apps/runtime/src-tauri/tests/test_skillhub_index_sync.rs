mod helpers;

use runtime_lib::commands::clawhub::{
    list_clawhub_library_with_pool, sync_skillhub_catalog_with_pool,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

async fn spawn_skillhub_catalog_server(body: String) -> (String, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind skillhub catalog server");
    let addr = listener.local_addr().expect("server addr");
    let handle = tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = [0u8; 2048];
            let _ = socket.read(&mut buf).await;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = socket.write_all(response.as_bytes()).await;
        }
    });

    (format!("http://{addr}/skills.json"), handle)
}

#[tokio::test]
async fn sync_skillhub_catalog_persists_full_index_and_metadata() {
    let (pool, _tmp) = helpers::setup_test_db().await;
    let body = serde_json::json!({
        "skills": [
            {
                "slug": "video-maker",
                "name": "Video Maker",
                "description": "Generate videos",
                "description_zh": "生成视频",
                "homepage": "https://clawhub.ai/skills/video-maker",
                "downloads": 99,
                "stars": 12,
                "tags": ["video", "creator"]
            },
            {
                "slug": "prompt-crafter",
                "name": "Prompt Crafter",
                "description": "Craft prompts",
                "downloads": 120,
                "stars": 8,
                "tags": ["prompt"]
            }
        ]
    })
    .to_string();
    let (url, handle) = spawn_skillhub_catalog_server(body).await;
    std::env::set_var("SKILLHUB_CATALOG_URL", &url);

    let sync = sync_skillhub_catalog_with_pool(&pool, true)
        .await
        .expect("sync catalog");
    assert_eq!(sync.total_skills, 2);
    assert!(sync.last_synced_at.is_some());

    let response =
        list_clawhub_library_with_pool(&pool, None, Some(20), Some("downloads".to_string()))
            .await
            .expect("list from synced index");

    let ordered: Vec<&str> = response.items.iter().map(|item| item.slug.as_str()).collect();
    assert_eq!(ordered, vec!["prompt-crafter", "video-maker"]);
    assert_eq!(response.last_synced_at, sync.last_synced_at);

    std::env::remove_var("SKILLHUB_CATALOG_URL");
    handle.await.expect("join skillhub server");
}
