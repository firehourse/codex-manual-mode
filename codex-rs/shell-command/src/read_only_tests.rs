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
        (vec!["git", "status"], false),
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
