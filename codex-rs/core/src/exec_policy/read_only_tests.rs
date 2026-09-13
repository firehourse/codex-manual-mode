use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn manual_powershell_search_uses_the_same_read_only_classifier() {
    let command = vec_str(&["powershell.exe", "-NoProfile", "-Command", "rg --files"]);
    let requirement = ExecPolicyManager::new(Arc::new(Policy::empty()))
        .create_exec_approval_requirement_for_command_platform(
            ExecApprovalRequest {
                command: &command,
                approval_policy: AskForApproval::UnlessTrusted,
                permission_profile: PermissionProfile::workspace_write(),
                environment_policy: None,
                windows_sandbox_level: WindowsSandboxLevel::RestrictedToken,
                sandbox_permissions: SandboxPermissions::UseDefault,
                prefix_rule: None,
                allow_prefix_rules: AllowPrefixRules::Honor,
            },
            DangerousCommandPlatform::Windows,
        )
        .await;
    assert_eq!(
        requirement,
        ExecApprovalRequirement::Skip {
            bypass_sandbox: false,
            proposed_execpolicy_amendment: Some(ExecPolicyAmendment::new(vec_str(&[
                "rg", "--files"
            ]))),
        }
    );
}

#[tokio::test]
async fn manual_read_only_commands_stay_in_the_sandbox() {
    for words in [
        vec!["rg", "-n", "needle", "src"],
        vec!["bash", "-lc", "rg -n needle src | head -n 20"],
        vec!["zsh", "-c", "cd src && rg --files; cat Cargo.toml"],
    ] {
        let command = vec_str(&words);
        let first_command = commands_for_exec_policy(&command)
            .commands
            .remove(/*index*/ 0);
        assert_exec_approval_requirement_for_command(
            ExecApprovalRequirementScenario {
                policy_src: None,
                command,
                approval_policy: AskForApproval::UnlessTrusted,
                permission_profile: PermissionProfile::workspace_write(),
                sandbox_permissions: SandboxPermissions::UseDefault,
                prefix_rule: None,
            },
            ExecApprovalRequirement::Skip {
                bypass_sandbox: false,
                proposed_execpolicy_amendment: Some(ExecPolicyAmendment::new(first_command)),
            },
        )
        .await;
    }
}

#[test]
fn manual_read_only_commands_still_require_approval_without_a_sandbox() {
    let command = vec_str(&["rg", "--files"]);
    assert_eq!(
        render_decision_for_unmatched_command(
            &command,
            UnmatchedCommandContext {
                approval_policy: AskForApproval::UnlessTrusted,
                permission_profile: &PermissionProfile::Disabled,
                windows_sandbox_level: WindowsSandboxLevel::Disabled,
                sandbox_permissions: SandboxPermissions::UseDefault,
                command_origin: ExecPolicyCommandOrigin::Generic,
            },
        ),
        Decision::Prompt,
    );
}

#[tokio::test]
async fn manual_read_only_rules_and_escalation_keep_their_priority() {
    let command = vec_str(&["rg", "--files"]);
    for (policy_src, sandbox_permissions, expected) in [
        (
            Some(r#"prefix_rule(pattern=["rg"], decision="prompt")"#.to_string()),
            SandboxPermissions::UseDefault,
            ExecApprovalRequirement::NeedsApproval {
                reason: Some("`rg --files` requires approval by policy".to_string()),
                proposed_execpolicy_amendment: None,
            },
        ),
        (
            Some(r#"prefix_rule(pattern=["rg"], decision="forbidden")"#.to_string()),
            SandboxPermissions::UseDefault,
            ExecApprovalRequirement::Forbidden {
                reason: "`rg --files` rejected: policy forbids commands starting with `rg`"
                    .to_string(),
            },
        ),
        (
            None,
            SandboxPermissions::RequireEscalated,
            ExecApprovalRequirement::NeedsApproval {
                reason: None,
                proposed_execpolicy_amendment: Some(ExecPolicyAmendment::new(command.clone())),
            },
        ),
    ] {
        assert_exec_approval_requirement_for_command(
            ExecApprovalRequirementScenario {
                policy_src,
                command: command.clone(),
                approval_policy: AskForApproval::UnlessTrusted,
                permission_profile: PermissionProfile::workspace_write(),
                sandbox_permissions,
                prefix_rule: None,
            },
            expected,
        )
        .await;
    }
}

#[tokio::test]
async fn manual_shell_side_effects_require_approval() {
    for script in [
        "rg needle input > output",
        "cat input >> output",
        "rg $(touch output) input",
        "rg needle <(touch output)",
        "rg needle input && touch output",
        "rg --pre=helper needle input",
        "rg -nz needle input",
        "rg --pre{=,=sh} needle input",
        "RIPGREP_CONFIG_PATH=config rg needle input",
    ] {
        let requirement = exec_approval_requirement_for_command(ExecApprovalRequirementScenario {
            policy_src: None,
            command: vec_str(&["bash", "-lc", script]),
            approval_policy: AskForApproval::UnlessTrusted,
            permission_profile: PermissionProfile::workspace_write(),
            sandbox_permissions: SandboxPermissions::UseDefault,
            prefix_rule: None,
        })
        .await;
        assert!(
            matches!(requirement, ExecApprovalRequirement::NeedsApproval { .. }),
            "{script}: {requirement:?}"
        );
    }
}
