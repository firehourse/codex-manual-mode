# Manual mode in this fork

Manual mode lets you review proposed file edits and untrusted shell commands before
Codex executes them. It uses Codex's internal `UnlessTrusted` approval policy, the
workspace sandbox, and a human reviewer. It adds no new model mode, config format,
or app-server protocol variant.

## What Claude Code does

Claude Code calls its `default` permission mode **Manual**. File reads and its
built-in read-only shell commands can run freely. Edits and other commands ask for
approval. A one-time approval authorizes that action; a broader approval can cover
later edits in the session or save a command rule. Permission prompts also support
feedback on individual decisions. These controls are enforced by the application.
[Claude Code permissions](https://code.claude.com/docs/en/permissions).

Shift+Tab cycles permission modes, and the status bar identifies the active one.
`acceptEdits` allows workspace edits automatically; `plan` explores and proposes
work; `auto` delegates decisions to a classifier. Those are different amounts of
oversight. This fork follows the manual review interaction, using Codex's own
approval engine. [Claude Code permission modes](https://code.claude.com/docs/en/permission-modes).

## Use it

Build from this repository with the Rust toolchain pinned in
`codex-rs/rust-toolchain.toml`:

```sh
cd codex-rs
python3 ../scripts/build_codex_manual.py
./target/debug/codex --manual
```

The helper builds both `codex` and its `codex-code-mode-host` companion, using
the repository's checksum-verified V8 archive and bindings for the pinned version.
It caches these build inputs under `codex-rs/target/manual-v8`. Keep the two
executables together. Building only `codex` leaves Code Mode unavailable; if you
already encountered that warning, rebuild with the helper and restart Codex.

Run the resulting binary from the project you want to work on, or pass `--cd`.
The `--manual` flag selects Manual before the first task, including a prompt
passed on the command line. Confirm that the footer shows **Manual mode**.
It also works with `codex resume --manual` and `codex fork --manual`.

The startup config loader rejects `approval_policy="untrusted"`, including when
passed with `-c`. Remove that setting if you saved it in `config.toml`. Manual
selects the internal policy through a typed startup override with `--manual`.
You can also start normally and choose Manual in the TUI.

The local `codex-manual` launcher opens this fork with startup update checks
disabled and `--manual` enabled. Every new launch starts in Manual. The flag is
for interactive sessions; `exec` and `review` do not support Manual approvals.

- Select **Manual** in `/permissions`, or press **Shift+Tab** in the composer.
- Shift+Tab cycles **Manual → Ask for approval → Manual**. If **Approve for me**
  is available, it appears between Ask for approval and Manual. Codex's existing
  Ask for approval mode permits workspace edits automatically.
- Native read/search tools keep their existing behavior. Shell-based reads such
  as `rg` can also prompt unless an existing command rule allows them. Claude's
  built-in read-only shell exemptions are not copied into this fork.
- The footer shows **Manual mode** when approvals are directed to you. Mode
  changes via Shift+Tab are confirmed by the server before the UI updates and
  do not rewrite your global config. Use `codex-manual` or `--manual` to keep
  Manual as the starting mode when restarting; mode changes made in the TUI
  apply to the current session.
- Enter planning through `/plan`. Shift+Tab leaves Plan when idle and restores
  the existing execution permissions. It no longer enters Plan from execution
  mode. Popups keep ownership of their keys.
- Inspect the proposed diff in the approval prompt; **Ctrl+A** opens the full
  patch pager. Approve once or reject using the displayed controls. You can
  interrupt and send a narrower instruction when the proposed action is wrong.
- Use one-time approvals for continued oversight. Saved command rules and
  session approvals remain effective. Managed requirements can disable Manual.

If a turn is running, stop it with Esc before changing modes. This checkout
captures approval policy for a turn; changing thread settings alone would not
change an active turn's policy. Shift+Tab therefore waits for idle, and the
permissions picker disables transitions into or out of Manual during a turn.
Changing modes does not undo completed edits.
The mode controls local edit and execution approvals. Existing MCP/app permission
rules, hooks, and tool-specific controls still apply; this is not a gate on every
tool call or every step of the model's reasoning. Reviewable steps help you catch
mistakes early, but human approval alone does not establish correctness.
[Codex sandbox and approvals](https://learn.chatgpt.com/docs/sandboxing).

## RAM and file writes

The existing patch path parses and validates a proposed edit into an in-memory
`ApplyPatchAction`, then requests approval before invoking the write operation.
The approval UI renders the patch from that data. This fork introduces no `/tmp`
patch staging area and does not write proposed source changes before approval.
See `core/src/apply_patch.rs`, `core/src/safety.rs`, and
`core/src/tools/runtimes/apply_patch.rs` under `codex-rs`.

This is per-tool-call review, not a virtual filesystem containing a whole task's
uncommitted changes. One patch or shell command can affect multiple files, so
review the whole proposal. Normal transcripts, logs, and tool-created temporary
files still follow Codex's existing behavior; RAM staging is not a promise that
the proposal is absent from session storage.

## Keep the fork across upstream updates

Build and install the fork under a separate name/path, for example
`~/.local/bin/codex-manual`, instead of replacing a package-managed `codex` binary.
On Windows, use a separate directory containing `codex-manual.exe`. Build the
release binaries with `python3 ../scripts/build_codex_manual.py --release` from
`codex-rs` before copying both executables there.
Keep any required packaged companion binaries/resources with the installation
when using features that need them.

Official installers and package-manager upgrades install official releases.
They do not merge fork source code. A separately installed fork remains at its
current version until you update and rebuild it.
[Official CLI installation and updates](https://learn.chatgpt.com/docs/cli).
This checkout identifies ordinary source builds as `0.0.0` and suppresses its
startup update prompt (`tui/src/updates.rs`). If you later assign release versions
to the fork, also give it its own update distribution or launch it with
`-c check_for_update_on_startup=false`.

The explicit `codex update` command is disabled in debug builds. In release
builds, detected update actions invoke official installers or package managers
(`cli/src/main.rs`, `tui/src/update_action.rs`). Update this fork through Git and
Cargo. The managed app-server daemon has its own automatic updater targeting
`$CODEX_HOME/packages/standalone/current`; keep the fork binary outside that
managed tree. That updater operates independently of the TUI startup setting
(`app-server-daemon/src/update_loop.rs` and `managed_install.rs`).

Keep the fork's changes committed before integrating upstream. In this checkout,
`origin` is `firehourse/codex-manual-mode`; add the official repository once:

```sh
git remote add upstream https://github.com/openai/codex.git
git fetch upstream --tags
git switch -c integrate-upstream
git merge upstream/main
```

Resolve conflicts, review the merge, run the scoped checks below, and rebuild.
For releases, merge a chosen upstream release tag instead of `upstream/main`.
Merging preserves published fork history; use rebasing only when rewriting that
branch's history is appropriate. Do not reset the fork branch to upstream, which
would discard the fork's commits from that branch.
[GitHub's fork synchronization guide](https://docs.github.com/en/pull-requests/how-tos/work-with-forks/syncing-a-fork).

This feature needs no migration of existing sessions: it reuses `untrusted`,
`user`, and the workspace permission profile. Upstream can still change its
storage or APIs later, so inspect those changes during each update. The most
likely conflicts are in permission menus, keyboard routing, and footer rendering.
Also check for changes to the engine's `UnlessTrusted` behavior; the Manual preset
depends on this internal policy even though startup config rejects `untrusted`.
If upstream gains an equivalent mode, remove the fork UI after checking that its
approval behavior matches your workflow.

## Validation when updating

From `codex-rs`, run
`just test -p codex-tui -p codex-cli -p codex-utils-approval-presets`.
Check configuration loading with `./target/debug/codex features list`, then start
the TUI with `--manual`. `--help` and `--version` exit before loading config and
cannot validate the startup instructions.
Review generated snapshots before accepting them. Key behaviors covered include
Manual selection, blocking mode changes during a turn, preserving model settings,
keeping popups in control, respecting managed requirements, server confirmation,
and leaving global config untouched. Startup tests also cover script creation:
the preview arrives before approval, rejection leaves no file, and acceptance
writes the displayed contents. The existing patch-pager tests cover full
diff review and approval decisions.

Follow the repository's `AGENTS.md` for formatting, linting, and any additional
tests required by the upstream changes you integrate.
