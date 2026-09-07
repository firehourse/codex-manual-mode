use codex_utils_cargo_bin::cargo_bin;
use pretty_assertions::assert_eq;

#[test]
fn manual_rejects_noninteractive_execution() -> anyhow::Result<()> {
    for command in ["exec", "review"] {
        let output = std::process::Command::new(cargo_bin("codex")?)
            .args(["--manual", command])
            .output()?;
        assert!(!output.status.success());
        assert_eq!(
            String::from_utf8(output.stderr)?.trim(),
            "Error: --manual requires an interactive session; run codex without exec or review",
        );
    }
    Ok(())
}
