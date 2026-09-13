//! Conservative read-only classification for Manual mode shell approvals.

mod git;

/// Recognizes plain command words that can run without approval in the sandbox.
///
/// Callers must parse shell syntax before calling this function and keep explicit
/// policy rules and sandbox escalation checks in effect. Paths to executables,
/// wrappers, and unknown commands require their usual approval.
pub fn is_read_only_command(command: &[String]) -> bool {
    let Some((program, args)) = command.split_first() else {
        return false;
    };
    match program.as_str() {
        "basename" | "cat" | "cd" | "cut" | "df" | "diff" | "dirname" | "du" | "echo" | "false"
        | "grep" | "head" | "id" | "ls" | "nl" | "paste" | "pwd" | "readlink" | "realpath"
        | "stat" | "tail" | "test" | "tr" | "true" | "uname" | "wc" | "whoami" => true,
        "git" | "git.exe" => git::is_read_only(args),
        "find" => !args.iter().any(|arg| {
            matches!(
                arg.as_str(),
                "-delete"
                    | "-exec"
                    | "-execdir"
                    | "-ok"
                    | "-okdir"
                    | "-fprint"
                    | "-fprint0"
                    | "-fprintf"
                    | "-fls"
            )
        }),
        "sort" => !args.iter().any(|arg| {
            let option = arg.split('=').next().unwrap_or(arg);
            (option.starts_with("--")
                && option.len() > 2
                && ["--output", "--compress-program"]
                    .iter()
                    .any(|name| name.starts_with(option)))
                || (arg.starts_with('-') && !arg.starts_with("--") && arg.contains('o'))
        }),
        "rg" | "rg.exe" => !args.iter().any(|arg| {
            let option = arg.split('=').next().unwrap_or(arg);
            matches!(option, "--pre" | "--hostname-bin" | "--search-zip")
                || (arg.starts_with('-') && !arg.starts_with("--") && arg.contains('z'))
        }),
        "sed" => {
            let [flag, expression, paths @ ..] = args else {
                return false;
            };
            flag == "-n"
                && expression.strip_suffix('p').is_some_and(|range| {
                    range.split(',').count() <= 2
                        && range.split(',').all(|address| {
                            !address.is_empty() && address.bytes().all(|byte| byte.is_ascii_digit())
                        })
                })
                && paths.iter().all(|path| !path.starts_with('-'))
        }
        _ => false,
    }
}

#[cfg(test)]
#[path = "read_only_tests.rs"]
mod tests;
