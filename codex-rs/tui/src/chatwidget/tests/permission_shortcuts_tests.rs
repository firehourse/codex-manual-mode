use super::permissions::requirements_stack;
use super::*;
use ApprovalsReviewer::AutoReview;
use ApprovalsReviewer::User;
use AskForApproval::OnRequest;
use AskForApproval::UnlessTrusted;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn shift_tab_toggles_manual_without_changing_model_or_plan() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let thread_id = ThreadId::new();
    chat.thread_id = Some(thread_id);
    chat.set_feature_enabled(Feature::GuardianApproval, /*enabled*/ false);
    chat.set_permission_profile_from_session_snapshot(PermissionProfileSnapshot::active(
        PermissionProfile::workspace_write(),
        ActivePermissionProfile::new(":workspace"),
    ))
    .unwrap();
    chat.set_approvals_reviewer(User);
    chat.set_approval_policy(OnRequest);
    #[cfg(target_os = "windows")]
    {
        chat.local_settings.notices.hide_world_writable_warning = Some(true);
        chat.set_windows_sandbox_mode(Some(WindowsSandboxModeToml::Unelevated));
    }
    let initial = chat.current_collaboration_mode().clone();
    while rx.try_recv().is_ok() {}
    for (next_policy, label) in [
        (UnlessTrusted, "Manual"),
        (OnRequest, ASK_FOR_APPROVAL_LABEL),
    ] {
        chat.handle_key_event(KeyEvent::from(KeyCode::BackTab));
        chat.handle_key_event(KeyEvent::from(KeyCode::BackTab));
        let AppEvent::ApplyPermissionShortcut {
            thread_id: target,
            selection,
        } = rx.try_recv().expect("permission selection")
        else {
            panic!("expected a confirmed permission update request");
        };
        assert_eq!(
            (
                target,
                selection.profile_id.as_str(),
                selection.approval_policy,
                selection.approvals_reviewer,
                selection.display_label.as_str()
            ),
            (
                thread_id,
                ":workspace",
                Some(next_policy),
                Some(User),
                label
            ),
        );
        assert!(
            rx.try_recv().is_err(),
            "one in-flight mode change at a time"
        );
        assert_eq!(chat.current_collaboration_mode(), &initial);
        assert_eq!(chat.active_collaboration_mode_kind(), ModeKind::Default);
        // Until the server confirms, the label and effective policy must stay unchanged.
        assert_ne!(
            AskForApproval::from(chat.config.permissions.approval_policy.value()),
            next_policy
        );
        chat.set_approval_policy(next_policy);
        chat.complete_permission_shortcut(thread_id);
    }
}

#[tokio::test]
async fn manual_mode_changes_wait_until_the_active_turn_stops() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.local_settings.tui.status_line = Some(vec!["model-with-reasoning".to_string()]);
    chat.thread_id = Some(ThreadId::new());
    chat.set_approval_policy(OnRequest);
    chat.on_task_started();
    while rx.try_recv().is_ok() {}
    chat.handle_key_event(KeyEvent::from(KeyCode::BackTab));
    let AppEvent::InsertHistoryCell(cell) = rx.try_recv().expect("stop-turn notice") else {
        panic!("must not change the label while the active turn still has the old policy");
    };
    insta::assert_snapshot!(
        "permission_shortcut_requires_idle",
        lines_to_single_string(&cell.display_lines(/*width*/ 90))
    );
    assert!(rx.try_recv().is_err());
    assert_eq!(
        AskForApproval::from(chat.config.permissions.approval_policy.value()),
        OnRequest
    );
    chat.open_permission_profiles_popup(crate::permission_discovery::PermissionDiscovery::local(
        &chat.config,
    ));
    let popup = render_bottom_popup(&chat, /*width*/ 120);
    assert!(
        popup
            .lines()
            .any(|line| line.contains("Manual") && line.contains("disabled")),
        "{popup}"
    );
    chat.handle_key_event(KeyEvent::from(KeyCode::Esc));
    chat.on_task_complete(
        /*last_agent_message*/ None, /*duration_ms*/ None, /*from_replay*/ false,
    );
    chat.set_approval_policy(UnlessTrusted);
    chat.set_approvals_reviewer(User);
    chat.on_task_started();
    assert_chatwidget_snapshot!(
        "manual_mode_running",
        render_bottom_popup(&chat, /*width*/ 90)
    );
    chat.set_feature_enabled(Feature::GuardianApproval, /*enabled*/ true);
    for named_profiles in [false, true] {
        if named_profiles {
            chat.open_permission_profiles_popup(
                crate::permission_discovery::PermissionDiscovery::local(&chat.config),
            );
        } else {
            chat.open_legacy_permissions_popup();
        }
        let popup = render_bottom_popup(&chat, /*width*/ 140);
        for mode in [
            ASK_FOR_APPROVAL_LABEL,
            APPROVE_FOR_ME_LABEL,
            "Full Access",
            "Manual",
        ] {
            assert!(
                popup
                    .lines()
                    .any(|line| line.contains(mode) && line.contains("disabled")),
                "{popup}"
            );
        }
        chat.handle_key_event(KeyEvent::from(KeyCode::Esc));
    }
}

#[tokio::test]
async fn manual_mode_picker_and_footer_keep_user_approval_visible() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.local_settings.tui.status_line = Some(vec!["model-with-reasoning".to_string()]);
    chat.thread_id = Some(ThreadId::new());
    chat.set_permission_profile_from_session_snapshot(PermissionProfileSnapshot::active(
        PermissionProfile::workspace_write(),
        ActivePermissionProfile::new(":workspace"),
    ))
    .unwrap();
    chat.set_approval_policy(UnlessTrusted);
    chat.set_approvals_reviewer(User);
    assert_chatwidget_snapshot!("manual_mode_idle", render_bottom_popup(&chat, /*width*/ 90));
    chat.open_permission_profiles_popup(crate::permission_discovery::PermissionDiscovery::local(
        &chat.config,
    ));
    assert_chatwidget_snapshot!(
        "manual_mode_picker",
        render_bottom_popup(&chat, /*width*/ 100)
    );
    while rx.try_recv().is_ok() {}
    chat.handle_key_event(KeyEvent::from(KeyCode::BackTab));
    assert!(
        rx.try_recv().is_err(),
        "picker must retain ownership of its keys"
    );
    chat.handle_key_event(KeyEvent::from(KeyCode::Esc));
    chat.set_approvals_reviewer(AutoReview);
    assert!(!render_bottom_popup(&chat, /*width*/ 90).contains("Manual mode"));
}

#[tokio::test]
async fn manual_mode_respects_reviewer_requirements_in_both_pickers() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::GuardianApproval, /*enabled*/ true);
    chat.config.config_layer_stack = requirements_stack(codex_config::ConfigRequirementsToml {
        allowed_approvals_reviewers: Some(vec![AutoReview]),
        ..Default::default()
    });
    for named_profiles in [false, true] {
        if named_profiles {
            chat.open_permission_profiles_popup(
                crate::permission_discovery::PermissionDiscovery::local(&chat.config),
            );
        } else {
            chat.open_legacy_permissions_popup();
        }
        let popup = render_bottom_popup(&chat, /*width*/ 120);
        assert!(
            popup
                .lines()
                .any(|line| line.contains("Manual") && line.contains("disabled")),
            "{popup}"
        );
        chat.handle_key_event(KeyEvent::from(KeyCode::Esc));
        assert!(
            rx.try_recv().is_err(),
            "opening the picker must not change permissions"
        );
    }
}

#[tokio::test]
async fn permission_shortcuts_cycle_builtin_modes() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let thread_id = ThreadId::new();
    chat.thread_id = Some(thread_id);
    chat.set_feature_enabled(Feature::GuardianApproval, /*enabled*/ true);
    chat.chat_keymap.next_permission_mode = vec![crate::key_hint::plain(KeyCode::F(8))];
    chat.chat_keymap.previous_permission_mode = vec![crate::key_hint::plain(KeyCode::F(7))];
    #[cfg(target_os = "windows")]
    {
        chat.local_settings.notices.hide_world_writable_warning = Some(true);
        chat.set_windows_sandbox_mode(Some(WindowsSandboxModeToml::Unelevated));
    }
    for (current, reviewer, policy, key, expected, next_reviewer, next_policy) in [
        (
            ":workspace",
            User,
            OnRequest,
            KeyCode::F(8),
            ":workspace",
            AutoReview,
            OnRequest,
        ),
        (
            ":workspace",
            AutoReview,
            OnRequest,
            KeyCode::F(8),
            ":workspace",
            User,
            UnlessTrusted,
        ),
        (
            ":workspace",
            User,
            UnlessTrusted,
            KeyCode::F(8),
            ":read-only",
            User,
            OnRequest,
        ),
        (
            ":read-only",
            User,
            OnRequest,
            KeyCode::F(8),
            ":workspace",
            User,
            OnRequest,
        ),
        (
            ":read-only",
            User,
            OnRequest,
            KeyCode::F(7),
            ":workspace",
            User,
            UnlessTrusted,
        ),
    ] {
        let profile = if current == ":read-only" {
            PermissionProfile::read_only()
        } else {
            PermissionProfile::workspace_write()
        };
        chat.config
            .permissions
            .set_permission_profile_from_session_snapshot(PermissionProfileSnapshot::active(
                profile,
                ActivePermissionProfile::new(current),
            ))
            .expect("set current profile");
        chat.config.approvals_reviewer = reviewer;
        chat.set_approval_policy(policy);
        chat.handle_key_event(KeyEvent::from(key));
        chat.handle_key_event(KeyEvent::from(key));
        let AppEvent::ApplyPermissionShortcut {
            thread_id: target,
            selection,
        } = rx.try_recv().expect("permission selection")
        else {
            panic!("expected one typed permission selection");
        };
        assert_eq!(
            (
                target,
                selection.profile_id.as_str(),
                selection.approval_policy,
                selection.approvals_reviewer
            ),
            (thread_id, expected, Some(next_policy), Some(next_reviewer))
        );
        assert!(
            rx.try_recv().is_err(),
            "pending shortcut must not be duplicated"
        );
        chat.complete_permission_shortcut(thread_id);
    }
    #[cfg(target_os = "windows")]
    {
        chat.set_windows_sandbox_mode(/*mode*/ None);
        chat.set_feature_enabled(Feature::WindowsSandbox, /*enabled*/ false);
        chat.set_feature_enabled(Feature::WindowsSandboxElevated, /*enabled*/ false);
        chat.config
            .permissions
            .set_permission_profile(PermissionProfile::read_only())
            .unwrap();
        chat.config.approvals_reviewer = User;
        chat.handle_key_event(KeyEvent::from(KeyCode::F(8)));
        assert!(matches!(
            rx.try_recv(),
            Ok(AppEvent::ApplyPermissionShortcut {
                selection: PermissionProfileSelection {
                    approvals_reviewer: Some(AutoReview),
                    ..
                },
                ..
            })
        ));
        assert!(rx.try_recv().is_err());
    }
}

#[tokio::test]
async fn permission_shortcuts_respect_managed_mode_requirements() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.set_feature_enabled(Feature::GuardianApproval, /*enabled*/ true);
    chat.config.approvals_reviewer = AutoReview;
    chat.chat_keymap.next_permission_mode = vec![crate::key_hint::plain(KeyCode::F(8))];
    chat.config
        .permissions
        .set_permission_profile_from_session_snapshot(PermissionProfileSnapshot::active(
            PermissionProfile::workspace_write(),
            ActivePermissionProfile::new(":workspace"),
        ))
        .expect("set active profile");

    for requirements in [
        codex_config::ConfigRequirementsToml {
            allowed_approvals_reviewers: Some(vec![AutoReview]),
            ..Default::default()
        },
        codex_config::ConfigRequirementsToml {
            auto_review: Some(codex_config::AutoReviewRequirementsToml {
                required_on_models: Some(vec![chat.current_model().to_string()]),
                ..Default::default()
            }),
            ..Default::default()
        },
    ] {
        chat.config.config_layer_stack = requirements_stack(requirements);
        chat.handle_key_event(KeyEvent::from(KeyCode::F(8)));
        let AppEvent::InsertHistoryCell(cell) = rx.try_recv().expect("unavailable-mode notice")
        else {
            panic!("must not submit a forbidden mode");
        };
        insta::assert_snapshot!(
            "permission_shortcut_no_alternative",
            lines_to_single_string(&cell.display_lines(/*width*/ 80))
        );
        assert!(rx.try_recv().is_err(), "must not submit a forbidden mode");
    }
}
