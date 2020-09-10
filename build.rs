use anyhow::{ensure, Context, Result};
use std::process::Command;

fn git_version() -> Result<String> {
    let v = Command::new("git")
        .arg("describe")
        .arg("--tags")
        .output()
        .context("Git description command execution")?;
    let s = v.status;
    ensure!(s.success(), "Git description command status: {}", s);
    String::from_utf8(v.stdout).context("Git description isn't UTF-8")
}

fn main() -> Result<()> {
    match git_version().context("GIT_VERSION assignment error") {
        Ok(version) => print!("cargo:rustc-env=GIT_VERSION={}", version),
        Err(e) => eprintln!("{:?}", e),
    }
    Ok(())
}
