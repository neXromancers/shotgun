use std::path::Path;
use std::process::Command;
use std::{str, env, fs};

fn main() {
    add_manpage("shotgun.1");
        
    let git_version = Command::new("git").arg("describe").arg("--tags").output();
    if let Ok(git_version) = git_version {
        if git_version.status.success() {
            let git_version =
                String::from_utf8(git_version.stdout).expect("Failed to decode git description");
            print!("cargo:rustc-env=GIT_VERSION={git_version}");
        }
    }
}

fn add_manpage(manpage: &str) {
    let manpage_source_path = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join("docs/").join(manpage);

    let root_path: String = match env::var("CARGO_INSTALL_ROOT") {
        Ok(r) => r,
        Err(_) => env::var("HOME").unwrap() + "/.cargo/",
    };
    let destination_path = root_path + "man/man1/";

    let manpath = Command::new("manpath").output().expect("Couldn't run \"manpath\"").stdout;
    let manpath_parsed = str::from_utf8(&manpath).unwrap(); // could next check $MANPATH, but if the manpath command
                                                            // doesn't exsist that probably wont either. It's likely
                                                            // they don't have man installed.
    if !manpath_parsed.contains(&destination_path) {
        println!("cargo:warning=Looks like \"{}\" isn't on your manpath, to see the man page please add it. See `man manpath`", destination_path);
    }

    let mut dir_builder = fs::DirBuilder::new();
    dir_builder.recursive(true);
    dir_builder.create(&destination_path).expect("Failed to create manpage's parent directories.");
    fs::copy(manpage_source_path, destination_path.clone() + manpage).expect(&format!("Failed to install manpage to {}", destination_path));

}

