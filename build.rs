use std::process::Command;

fn main() {
    let git_version = Command::new("git").arg("describe").arg("--tags").output();
    if let Ok(git_version) = git_version {
        if git_version.status.success() {
            let git_version =
                String::from_utf8(git_version.stdout).expect("Failed to decode git description");
            print!("cargo:rustc-env=GIT_VERSION={git_version}");
        }
    }
}
