use super::is_read_only_command;
use pretty_assertions::assert_eq;

#[test]
fn classifies_read_only_commands_and_side_effecting_options() {
    for (words, expected) in [
        (vec!["rg", "-n", "needle", "src"], true),
        (vec!["rg", "--files", "-g", "*.rs"], true),
        (vec!["rg.exe", "--count", "needle", "."], true),
        (vec!["cat", "Cargo.toml"], true),
        (vec!["ls", "-la"], true),
        (vec!["head", "-n", "20", "file.txt"], true),
        (vec!["tail", "-n", "20", "file.txt"], true),
        (vec!["wc", "-l", "file.txt"], true),
        (vec!["cd", "src"], true),
        (vec!["diff", "-u", "old", "new"], true),
        (vec!["du", "-sh", "src"], true),
        (vec!["find", ".", "-name", "*.rs", "-print"], true),
        (vec!["sort", "-u", "input"], true),
        (vec!["sed", "-n", "1,20p", "file.txt"], true),
        (vec!["sed", "-n", "20p"], true),
        (vec!["rg", "--pre", "helper", "needle"], false),
        (vec!["rg", "--pre=helper", "needle"], false),
        (vec!["rg", "--hostname-bin=helper", "needle"], false),
        (vec!["rg", "--search-zip", "needle"], false),
        (vec!["rg", "-nz", "needle"], false),
        (vec!["sed", "-i", "s/a/b/", "file.txt"], false),
        (vec!["sed", "-n", "1p;w output", "file.txt"], false),
        (vec!["sed", "-n", "1p", "--file=script"], false),
        (vec!["sed", "-n", "1,2,3p"], false),
        (vec!["sed", "-n", "p"], false),
        (vec!["uniq", "input", "output"], false),
        (vec!["sort", "-o", "output", "input"], false),
        (vec!["find", ".", "-delete"], false),
        (vec!["sort", "--out=output", "input"], false),
        (vec!["sort", "--compress-program=helper", "input"], false),
        (vec!["find", ".", "-exec", "helper", ";"], false),
        (vec!["find", ".", "-fprint", "output"], false),
        (vec!["python3", "-c", "print('hello')"], false),
        (vec!["env", "rg", "needle"], false),
        (vec!["./rg", "needle"], false),
        (vec!["/tmp/rg", "needle"], false),
        (vec!["bash", "-c", "rg needle"], false),
        (vec![], false),
    ] {
        let command = words.into_iter().map(str::to_owned).collect::<Vec<_>>();
        assert_eq!(is_read_only_command(&command), expected, "{command:?}");
    }
}

#[test]
fn git_queries_do_not_allow_mutating_commands_or_helper_options() {
    for (words, expected) in [
        (vec!["git", "status", "--short", "--branch"], true),
        (vec!["git", "diff", "--cached", "--check"], true),
        (vec!["git", "--no-pager", "log", "-8", "--oneline"], true),
        (vec!["git", "show", "HEAD:Cargo.toml"], true),
        (vec!["git", "branch", "-vv"], true),
        (vec!["git", "branch", "--list", "feature/*"], true),
        (vec!["git", "tag", "--list", "v*"], true),
        (vec!["git", "remote", "-v"], true),
        (vec!["git", "remote", "get-url", "--push", "origin"], true),
        (vec!["git", "worktree", "list", "--porcelain"], true),
        (
            vec!["git", "config", "--show-origin", "--get", "core.hooksPath"],
            true,
        ),
        (vec!["git", "config", "--get-regexp", "remote.*"], true),
        (vec!["git.exe", "rev-parse", "--git-path", "hooks"], true),
        (
            vec!["git", "ls-files", "--others", "--exclude-standard"],
            true,
        ),
        (vec!["git", "add", "file"], false),
        (vec!["git", "commit", "-m", "message"], false),
        (vec!["git", "push", "origin", "main"], false),
        (vec!["git", "branch", "new-branch"], false),
        (vec!["git", "branch", "--list", "-D", "main"], false),
        (vec!["git", "tag", "v1"], false),
        (vec!["git", "remote", "add", "origin", "url"], false),
        (vec!["git", "worktree", "add", "path"], false),
        (vec!["git", "config", "core.hooksPath", "helper"], false),
        (
            vec!["git", "config", "--get", "--unset", "user.name"],
            false,
        ),
        (vec!["git", "config", "--list", "--edit"], false),
        (vec!["git", "-c", "core.fsmonitor=helper", "status"], false),
        (vec!["git", "--paginate", "log"], false),
        (vec!["git", "diff", "--output=output"], false),
        (vec!["git", "log", "--out=output"], false),
        (vec!["git", "show", "--textconv", "HEAD:file"], false),
        (vec!["git", "diff", "--ext-diff"], false),
        (vec!["git", "difftool"], false),
    ] {
        let command = words.into_iter().map(str::to_owned).collect::<Vec<_>>();
        assert_eq!(is_read_only_command(&command), expected, "{command:?}");
    }
}
