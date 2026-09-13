//! Git queries permitted by Manual mode, without allowing mutating subcommands.

pub(super) fn is_read_only(mut args: &[String]) -> bool {
    while let Some((option, rest)) = args.split_first()
        && matches!(
            option.as_str(),
            "--no-pager" | "--no-optional-locks" | "--literal-pathspecs"
        )
    {
        args = rest;
    }
    let Some((subcommand, args)) = args.split_first() else {
        return false;
    };

    // Diff options are shared by several read commands. Reject output files,
    // explicitly requested helpers, and abbreviated spellings of those options.
    if args
        .iter()
        .take_while(|arg| arg.as_str() != "--")
        .any(|arg| {
            let option = arg.split('=').next().unwrap_or(arg);
            option.starts_with("--")
                && option.len() > 2
                && [
                    "--output",
                    "--ext-diff",
                    "--textconv",
                    "--show-signature",
                    "--exec",
                    "--open-files-in-pager",
                ]
                .iter()
                .any(|name| name.starts_with(option))
        })
    {
        return false;
    }

    match subcommand.as_str() {
        "status" | "diff" | "log" | "show" | "blame" | "ls-files" | "ls-tree" | "rev-parse"
        | "rev-list" | "describe" | "show-ref" | "count-objects" => true,
        "branch" | "tag" => args.iter().all(|arg| {
            matches!(
                arg.as_str(),
                "--list"
                    | "-l"
                    | "--all"
                    | "-a"
                    | "--remotes"
                    | "-r"
                    | "--verbose"
                    | "-v"
                    | "-vv"
                    | "--show-current"
                    | "--no-color"
            ) || arg.starts_with("--format=")
                || arg.starts_with("--sort=")
                || arg.starts_with("--contains=")
                || arg.starts_with("--merged=")
                || (!arg.starts_with('-')
                    && args
                        .iter()
                        .any(|arg| matches!(arg.as_str(), "--list" | "-l")))
        }),
        "remote" => match args {
            [] => true,
            [flag] => matches!(flag.as_str(), "-v" | "--verbose"),
            [action, rest @ ..] if action == "get-url" => rest
                .iter()
                .all(|arg| !arg.starts_with('-') || matches!(arg.as_str(), "--all" | "--push")),
            _ => false,
        },
        "worktree" => matches!(args, [action, rest @ ..] if action == "list"
            && rest.iter().all(|arg| matches!(arg.as_str(), "--porcelain" | "-v" | "--verbose" | "-z"))),
        "config" => {
            let mut read_action = false;
            let mut args = args.iter();
            while let Some(arg) = args.next() {
                match arg.as_str() {
                    "--get" | "--get-all" | "--get-regexp" | "--get-urlmatch" | "--list" | "-l" => {
                        read_action = true;
                    }
                    "--global" | "--system" | "--local" | "--worktree" | "--show-origin"
                    | "--show-scope" | "--includes" | "--no-includes" | "--null" | "-z"
                    | "--name-only" | "--bool" | "--int" | "--bool-or-int" | "--path" => {}
                    "--file" | "-f" | "--blob" | "--type" => {
                        if args.next().is_none() {
                            return false;
                        }
                    }
                    _ if arg.starts_with("--file=")
                        || arg.starts_with("--blob=")
                        || arg.starts_with("--type=") => {}
                    _ if !arg.starts_with('-') => {}
                    _ => return false,
                }
            }
            read_action
        }
        _ => false,
    }
}
