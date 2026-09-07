//! Permission overrides shared by startup validation and interactive configuration.

use crate::Cli;
use crate::legacy_core::config::Config;
use crate::legacy_core::config::ConfigOverrides;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::config_types::SandboxMode;
use codex_protocol::protocol::AskForApproval;

pub(crate) fn overrides(cli: &Cli) -> ConfigOverrides {
    if cli.manual {
        // The retired TOML value is rejected by the config loader. A typed harness
        // override selects the supported runtime policy and still checks requirements.
        return ConfigOverrides {
            approval_policy: Some(AskForApproval::UnlessTrusted),
            approvals_reviewer: Some(ApprovalsReviewer::User),
            sandbox_mode: Some(SandboxMode::WorkspaceWrite),
            ..Default::default()
        };
    }
    let (sandbox_mode, approval_policy) = if cli.dangerously_bypass_approvals_and_sandbox {
        (
            Some(SandboxMode::DangerFullAccess),
            Some(AskForApproval::Never),
        )
    } else {
        (
            cli.sandbox_mode.map(Into::into),
            cli.approval_policy.map(Into::into),
        )
    };
    ConfigOverrides {
        approval_policy,
        sandbox_mode,
        ..Default::default()
    }
}

pub(crate) fn validate(cli: &Cli, config: &Config) -> std::io::Result<()> {
    if cli.manual
        && (config.permissions.approval_policy.value() != AskForApproval::UnlessTrusted
            || config.approvals_reviewer != ApprovalsReviewer::User)
    {
        return Err(std::io::Error::other(
            "Manual mode is unavailable under the current approval requirements",
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "startup_permissions_tests.rs"]
mod tests;
