//! Command parsing and safety utilities shared across Codex crates.

pub mod shell_detect;
pub mod shell_snapshot;

pub mod bash;
pub(crate) mod command_safety;
pub mod parse_command;
pub mod powershell;
mod read_only;

pub use command_safety::is_dangerous_command;
pub use read_only::is_read_only_command;
