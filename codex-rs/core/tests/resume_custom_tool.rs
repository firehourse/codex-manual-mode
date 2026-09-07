#![allow(clippy::expect_used)]

use codex_history::RolloutItem;
use codex_history::RolloutLine;
use codex_protocol::ResponseItemId;
use codex_protocol::ThreadId;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::SessionMeta;
use codex_protocol::protocol::SessionMetaLine;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::io::Write;
use std::sync::Arc;
use tempfile::TempDir;
use uuid::Uuid;
use wiremock::MockServer;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn resume_recovers_missing_custom_tool_output_without_rewriting_history() -> anyhow::Result<()>
{
    let call_id = "interrupted-exec-call";
    let thread_id = ThreadId::default();
    let rollout = [
        RolloutLine {
            timestamp: "2024-01-01T00:00:00.000Z".to_string(),
            ordinal: None,
            item: RolloutItem::SessionMeta(SessionMetaLine {
                meta: SessionMeta {
                    session_id: thread_id.into(),
                    id: thread_id,
                    timestamp: "2024-01-01T00:00:00Z".to_string(),
                    cwd: ".".into(),
                    originator: "test_originator".to_string(),
                    cli_version: "test_version".to_string(),
                    model_provider: Some("test-provider".to_string()),
                    ..Default::default()
                },
                git: None,
            }),
        },
        RolloutLine {
            timestamp: "2024-01-01T00:00:01.000Z".to_string(),
            ordinal: None,
            item: RolloutItem::ResponseItem(
                ResponseItem::CustomToolCall {
                    id: Some(ResponseItemId::with_suffix("ctc", "existing")),
                    status: Some("completed".to_string()),
                    call_id: call_id.to_string(),
                    name: "exec".to_string(),
                    namespace: None,
                    input: "text(\"interrupted\");".to_string(),
                    internal_chat_message_metadata_passthrough: None,
                }
                .into(),
            ),
        },
    ];
    let tmpdir = TempDir::new()?;
    let session_path = tmpdir.path().join("interrupted-custom-tool.jsonl");
    let mut file = std::fs::File::create(&session_path)?;
    for line in rollout {
        writeln!(file, "{}", serde_json::to_string(&line)?)?;
    }

    let server = MockServer::start().await;
    let response_mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
            sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
        ],
    )
    .await;
    let codex_home = Arc::new(TempDir::new()?);
    let mut builder = test_codex();

    for prompt in ["first resume", "second resume"] {
        let resumed = builder
            .resume(&server, Arc::clone(&codex_home), session_path.clone())
            .await?;
        resumed.submit_turn(prompt).await?;
        resumed.codex.shutdown_and_wait().await?;

        assert!(
            !std::fs::read_to_string(&session_path)?
                .contains("\"type\":\"custom_tool_call_output\""),
            "prompt-only repair should not be persisted to the rollout"
        );
    }

    let requests = response_mock.requests();
    assert_eq!(requests.len(), 2);
    let first_output = requests[0].custom_tool_call_output(call_id);
    let output_id = first_output["id"]
        .as_str()
        .expect("reconstructed output should have an item ID");
    let output_uuid = output_id
        .strip_prefix("ctco_")
        .expect("synthetic output should use the custom tool output prefix");
    assert_eq!(
        Uuid::parse_str(output_uuid)?.get_version(),
        Some(uuid::Version::Sha1)
    );
    assert_eq!(
        first_output,
        json!({
            "type": "custom_tool_call_output",
            "id": output_id,
            "call_id": call_id,
            "output": "aborted",
        })
    );
    assert_eq!(requests[1].custom_tool_call_output(call_id), first_output);

    Ok(())
}
