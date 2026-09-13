#![allow(clippy::expect_used)]

use anyhow::Result;
use codex_core::TurnInputRequest;
use codex_core::config::Constrained;
use codex_features::Feature;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::skip_if_target_windows;
use core_test_support::test_codex::TestCodexHarness;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::json;
use test_case::test_case;

#[derive(Clone, Copy)]
enum Approval {
    Skip,
    Prompt,
}

#[test_case("rg --no-config -n needle input.txt", "", Approval::Skip; "search")]
#[test_case("cat input.txt | head -n 1", "", Approval::Skip; "pipeline")]
#[test_case("git diff --no-index --no-ext-diff --no-textconv /dev/null input.txt", "", Approval::Skip; "git diff")]
#[test_case("find . -name input.txt -print", "", Approval::Skip; "find")]
#[test_case("sort input.txt", "", Approval::Skip; "sort")]
#[test_case("git diff --output=output.txt", "", Approval::Prompt; "git output")]
#[test_case("git add input.txt", "", Approval::Prompt; "git write")]
#[test_case("cd . && git status", "", Approval::Prompt; "git after cd")]
#[test_case("find . -name input.txt -delete", "", Approval::Prompt; "find delete")]
#[test_case("sort -o output.txt input.txt", "", Approval::Prompt; "sort output")]
#[test_case(
    "git diff --no-index /dev/null input.txt",
    r#"prefix_rule(pattern=["git"], decision="prompt")"#,
    Approval::Prompt; "explicit git rule"
)]
#[test_case("cat input.txt > output.txt", "", Approval::Prompt; "redirect")]
#[test_case("cat input.txt && touch output.txt", "", Approval::Prompt; "mixed commands")]
#[test_case("rg --pre=helper needle input.txt", "", Approval::Prompt; "preprocessor")]
#[test_case(
    "cat input.txt",
    r#"prefix_rule(pattern=["cat"], decision="prompt")"#,
    Approval::Prompt; "explicit rule"
)]
#[tokio::test]
async fn manual_read_only_approval_flow(
    command: &str,
    policy_rule: &str,
    approval: Approval,
) -> Result<()> {
    skip_if_target_windows!(Ok(()), "exercises POSIX shell commands and pipelines");

    let policy_rule = policy_rule.to_owned();
    let builder = test_codex()
        .with_model("gpt-5.2")
        .with_config(move |config| {
            config
                .features
                .enable(Feature::UnifiedExec)
                .expect("enable unified exec");
            config.permissions.approval_policy =
                Constrained::allow_any(AskForApproval::UnlessTrusted);
            config
                .permissions
                .set_permission_profile(PermissionProfile::workspace_write())
                .expect("set workspace permissions");
            config.approvals_reviewer = ApprovalsReviewer::User;
            let rules = config.codex_home.join("rules");
            std::fs::create_dir_all(&rules).expect("create rules directory");
            std::fs::write(rules.join("manual.rules"), &policy_rule).expect("write test policy");
        });
    let harness = TestCodexHarness::with_auto_env_builder(builder).await?;
    let test = harness.test();
    harness.write_file("input.txt", "needle\n").await?;
    let call_id = "manual-shell";
    let args = json!({"cmd": command, "login": false, "yield_time_ms": 1_000});
    let initial = mount_sse_once(
        harness.server(),
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(call_id, "exec_command", &serde_json::to_string(&args)?),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let results = mount_sse_once(
        harness.server(),
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "inspect the input file".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    let event = wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::ExecApprovalRequest(_) | EventMsg::TurnComplete(_)
        )
    })
    .await;
    match approval {
        Approval::Skip => assert!(matches!(event, EventMsg::TurnComplete(_)), "{event:?}"),
        Approval::Prompt => {
            let EventMsg::ExecApprovalRequest(request) = event else {
                panic!("expected approval: {event:?}");
            };
            assert_eq!(request.call_id, call_id);
            assert!(!harness.path_exists("output.txt").await?);
            test.codex
                .submit(Op::ExecApproval {
                    id: request.effective_approval_id(),
                    turn_id: None,
                    decision: ReviewDecision::denied("manual test rejected the command"),
                })
                .await?;
            wait_for_event(&test.codex, |event| {
                matches!(event, EventMsg::TurnComplete(_))
            })
            .await;
        }
    }
    let output = results
        .single_request()
        .function_call_output_text(call_id)
        .expect("shell output");
    match approval {
        Approval::Skip => assert!(
            output
                .lines()
                .any(|line| matches!(line, "needle" | "1:needle" | "+needle" | "./input.txt")),
            "{output}"
        ),
        Approval::Prompt => assert!(
            output.contains("manual test rejected the command"),
            "{output}"
        ),
    }
    assert_eq!(initial.requests().len(), 1);
    assert_eq!(harness.read_file_text("input.txt").await?, "needle\n");
    assert!(!harness.path_exists("output.txt").await?);
    Ok(())
}
