use runtime_lib::content_providers::{ContentArtifact, ContentCapability, NormalizedContentResult};
use serde_json::json;

#[test]
fn normalized_result_serializes_core_fields() {
    let result = NormalizedContentResult {
        source_provider: "agent-reach".to_string(),
        capability: ContentCapability::ReadUrl,
        title: Some("Example".to_string()),
        url: Some("https://example.com".to_string()),
        text: "plain text".to_string(),
        markdown: Some("# Example".to_string()),
        metadata: json!({
            "platform": "github"
        }),
        artifacts: vec![ContentArtifact {
            kind: "transcript".to_string(),
            uri: "file:///tmp/transcript.txt".to_string(),
            label: Some("Transcript".to_string()),
        }],
    };

    let value = serde_json::to_value(&result).expect("serialize");

    assert_eq!(value["source_provider"], "agent-reach");
    assert_eq!(value["capability"], "read_url");
    assert_eq!(value["title"], "Example");
    assert_eq!(value["url"], "https://example.com");
    assert_eq!(value["text"], "plain text");
    assert_eq!(value["markdown"], "# Example");
    assert_eq!(value["metadata"]["platform"], "github");
    assert_eq!(value["artifacts"][0]["kind"], "transcript");
}
