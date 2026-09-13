use super::*;
use crate::app_server_session::TurnPermissionsOverride;
use crate::legacy_core::config::ConfigBuilder;
use clap::Parser;
use codex_app_server_client::AppServerEvent;
use codex_app_server_protocol::FileChangeApprovalDecision;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::UserInput;
use codex_config::LoaderOverrides;
use core_test_support::responses;
use pretty_assertions::assert_eq;

#[test]
fn manual_startup_rejects_conflicting_permission_flags() {
    for flags in [
        vec!["--ask-for-approval", "never"],
        vec!["--sandbox", "danger-full-access"],
        vec!["--approve-for-me"],
        vec!["--yolo"],
    ] {
        let args = [vec!["codex", "--manual"], flags].concat();
        assert_eq!(
            Cli::try_parse_from(args).unwrap_err().kind(),
            clap::error::ErrorKind::ArgumentConflict,
        );
    }
}

#[tokio::test]
async fn manual_startup_respects_managed_requirements() -> color_eyre::Result<()> {
    for requirement in [
        "allowed_approval_policies = [\"on-request\"]",
        "allowed_approvals_reviewers = [\"auto_review\"]",
    ] {
        let home = tempfile::tempdir()?;
        let cli = Cli::parse_from(["codex", "--manual"]);
        let config = ConfigBuilder::default()
            .codex_home(home.path().to_path_buf())
            .loader_overrides(LoaderOverrides::without_managed_config_for_tests())
            .cloud_config_bundle(codex_config::test_support::CloudConfigBundleFixture::loader_with_enterprise_requirement(requirement))
            .harness_overrides(ConfigOverrides {
                cwd: Some(home.path().to_path_buf()),
                ..overrides(&cli)
            })
            .build().await?;
        let error =
            validate(&cli, &config).expect_err("Manual must not accept a requirements fallback");
        insta::allow_duplicates! {
            insta::assert_snapshot!("manual_startup_requirements_error", error.to_string());
        }
    }
    Ok(())
}

#[tokio::test]
async fn manual_startup_requests_approval_before_creating_script() -> color_eyre::Result<()> {
    for decision in [
        FileChangeApprovalDecision::Accept,
        FileChangeApprovalDecision::Decline,
    ] {
        let server = wiremock::MockServer::start().await;
        let home = tempfile::tempdir()?;
        let workspace = tempfile::tempdir()?;
        let scratch = tempfile::tempdir()?;
        let script = scratch.path().join("review-script.js");
        let contents = "console.log('reviewed');\n";
        let patch = format!(
            "*** Begin Patch\n*** Add File: {}\n+console.log('reviewed');\n*** End Patch",
            script.display()
        );
        let mock = responses::mount_sse_sequence(
            &server,
            vec![
                responses::sse(vec![
                    responses::ev_response_created("patch-response"),
                    responses::ev_apply_patch_custom_tool_call("create-script", &patch),
                    responses::ev_completed("patch-response"),
                ]),
                responses::sse(vec![
                    responses::ev_response_created("done-response"),
                    responses::ev_assistant_message("done-message", "done"),
                    responses::ev_completed("done-response"),
                ]),
            ],
        )
        .await;
        let config_path = home.path().join("config.toml");
        let saved_config = format!(
            r#"model = "gpt-5.4"
model_provider = "manual-test"
approval_policy = "on-request"
approvals_reviewer = "auto_review"

[features]
code_mode = false
code_mode_only = false

[model_providers.manual-test]
name = "Manual test"
base_url = "{}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0

[projects.{}]
trust_level = "trusted"
"#,
            server.uri(),
            serde_json::to_string(&codex_config::loader::project_trust_key(workspace.path()))?
        );
        std::fs::write(&config_path, &saved_config)?;
        let cli = Cli::parse_from(["codex", "--manual"]);
        let config = ConfigBuilder::default()
            .codex_home(home.path().to_path_buf())
            .loader_overrides(LoaderOverrides::without_managed_config_for_tests())
            .harness_overrides(ConfigOverrides {
                cwd: Some(workspace.path().to_path_buf()),
                ..overrides(&cli)
            })
            .build()
            .await?;
        validate(&cli, &config)?;
        let mut app_server = Box::pin(crate::start_embedded_app_server_for_picker(&config)).await?;
        let started = app_server.start_thread(&config).await?;
        assert_eq!(
            (
                started.session.approval_policy,
                started.session.approvals_reviewer
            ),
            (
                codex_app_server_protocol::AskForApproval::UnlessTrusted,
                ApprovalsReviewer::User
            ),
        );
        app_server
            .turn_start(
                started.session.thread_id,
                "manual-user-message".to_string(),
                vec![UserInput::Text {
                    text: "Create the review script".to_string(),
                    text_elements: Vec::new(),
                }],
                config.cwd.to_path_buf(),
                Some(started.session.approval_policy),
                Some(started.session.approvals_reviewer.into()),
                TurnPermissionsOverride::Preserve,
                &config.workspace_roots,
                "gpt-5.4".to_string(),
                /*effort*/ None,
                /*summary*/ None,
                /*service_tier*/ None,
                /*collaboration_mode*/ None,
                /*personality*/ None,
                /*output_schema*/ None,
            )
            .await?;
        let mut approval_requested = false;
        let mut previewed = false;
        loop {
            let event = tokio::time::timeout(
                std::time::Duration::from_secs(/*secs*/ 15),
                app_server.next_event(),
            )
            .await?
            .expect("app-server remains connected");
            match event {
                AppServerEvent::ServerRequest(request) => {
                    let ServerRequest::FileChangeRequestApproval { request_id, .. } = *request
                    else {
                        panic!("expected file creation approval: {request:?}");
                    };
                    assert!(!approval_requested, "one patch must request one approval");
                    assert!(previewed, "the full diff must arrive before approval");
                    assert!(
                        !script.exists(),
                        "the script must not exist before approval"
                    );
                    approval_requested = true;
                    app_server
                        .resolve_server_request(
                            request_id,
                            serde_json::json!({"decision": decision}),
                        )
                        .await?;
                }
                AppServerEvent::ServerNotification(notification) => match *notification {
                    ServerNotification::ItemStarted(notification) => {
                        if let ThreadItem::FileChange { changes, .. } = notification.item {
                            previewed = true;
                            let preview = serde_json::to_string_pretty(&changes)?.replace(
                                &script.to_string_lossy().replace('\\', "\\\\"),
                                "<SCRATCH>/review-script.js",
                            );
                            insta::allow_duplicates! { insta::assert_snapshot!("manual_startup_script_preview", preview); }
                        }
                    }
                    ServerNotification::TurnCompleted(notification) => {
                        assert_eq!(notification.turn.error, None);
                        assert!(
                            approval_requested,
                            "Manual must request approval before completing the turn"
                        );
                        break;
                    }
                    _ => {}
                },
                AppServerEvent::Lagged { .. } | AppServerEvent::Disconnected { .. } => {
                    panic!("lost app-server events: {event:?}")
                }
            }
        }
        let expected =
            (decision == FileChangeApprovalDecision::Accept).then(|| contents.to_string());
        assert_eq!(
            (script.try_exists()?, std::fs::read_to_string(&script).ok()),
            (expected.is_some(), expected),
            "patch output: {}",
            mock.requests()
                .last()
                .expect("tool response")
                .custom_tool_call_output("create-script")
        );
        assert_eq!(mock.requests().len(), 2);
        assert_eq!(std::fs::read_to_string(&config_path)?, saved_config);
        app_server.shutdown().await?;
    }
    Ok(())
}
