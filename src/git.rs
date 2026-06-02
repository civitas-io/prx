use std::path::Path;

/// Run `git show <ref>:<path>` and return the file contents.
pub fn show_file(path: &str, git_ref: &str) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["show", &format!("{git_ref}:{path}")])
        .output()
        .ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        None
    }
}

/// Run `git diff --name-only <ref>..HEAD` and return changed file paths.
pub fn changed_files(root: &Path, git_ref: &str) -> Option<Vec<String>> {
    let output = std::process::Command::new("git")
        .args(["diff", "--name-only", &format!("{git_ref}..HEAD")])
        .current_dir(root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    Some(
        stdout
            .lines()
            .filter(|l| !l.is_empty())
            .map(String::from)
            .collect(),
    )
}
