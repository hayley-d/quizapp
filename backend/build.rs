use std::path::{Path, PathBuf};
use std::process::Command;

const SKIP_VARIABLE: &str = "QUIZAPP_SKIP_FRONTEND_BUILD";
const PNPM_VARIABLE: &str = "QUIZAPP_PNPM";

fn main() {
    let manifest_directory = PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is always set by cargo"),
    );
    let frontend_directory = manifest_directory
        .parent()
        .expect("the backend package always has a parent directory")
        .join("frontend");
    let built_index = frontend_directory.join("dist").join("index.html");

    // Emitting any rerun-if-changed line replaces cargo's default "rerun if anything
    // in the package changed" rule. That default is the one thing we must not keep:
    // it would run a full Vite build on every backend-only edit. Everything listed
    // here is an input to `vite build`; frontend/dist is deliberately absent, because
    // watching a directory this script writes is how a rebuild loop starts.
    for build_input in [
        "src",
        "public",
        "index.html",
        "package.json",
        "pnpm-lock.yaml",
        "vite.config.ts",
        "tsconfig.json",
        "tsconfig.app.json",
        "tsconfig.node.json",
    ] {
        println!(
            "cargo:rerun-if-changed={}",
            frontend_directory.join(build_input).display()
        );
    }
    println!("cargo:rerun-if-env-changed={SKIP_VARIABLE}");
    println!("cargo:rerun-if-env-changed={PNPM_VARIABLE}");

    let embed_directory = PathBuf::from(
        std::env::var("OUT_DIR").expect("OUT_DIR is always set by cargo"),
    )
    .join("frontend");

    if std::env::var_os(SKIP_VARIABLE).is_some() {
        if !built_index.exists() {
            fail(&format!(
                "{SKIP_VARIABLE} is set, but {} does not exist.\n\
                 The frontend bundle is compiled into this binary, so it has to be built \
                 at least once.\n\
                 Run `pnpm --dir frontend build`, or unset {SKIP_VARIABLE} and build again.",
                built_index.display()
            ));
        }
        mirror_into_embed_directory(&frontend_directory.join("dist"), &embed_directory);
        return;
    }

    let pnpm = std::env::var(PNPM_VARIABLE).unwrap_or_else(|_| "pnpm".to_string());
    let outcome = Command::new(&pnpm)
        .arg("build")
        .current_dir(&frontend_directory)
        .status();

    match outcome {
        Ok(status) if status.success() => {}
        Ok(status) => fail(&format!(
            "`{pnpm} build` failed in {} with {status}.\n\
             The tsc and vite output above says why; fix it there, not here.",
            frontend_directory.display()
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => fail(&format!(
            "could not find `{pnpm}` on PATH.\n\
             The React bundle is compiled into this binary, so building the backend needs \
             pnpm.\n\
             Either install it, or point {PNPM_VARIABLE} at an absolute path to the binary, \
             or set {SKIP_VARIABLE}=1 to reuse an already-built frontend/dist.\n\
             PATH as this build script saw it:\n  {}",
            std::env::var("PATH").unwrap_or_default()
        )),
        Err(error) => fail(&format!("could not run `{pnpm} build`: {error}")),
    }

    if !built_index.exists() {
        fail(&format!(
            "`{pnpm} build` reported success but {} is missing.\n\
             Check build.outDir in frontend/vite.config.ts.",
            built_index.display()
        ));
    }

    mirror_into_embed_directory(&frontend_directory.join("dist"), &embed_directory);
}

// The embed reads OUT_DIR rather than frontend/dist directly. Cargo preserves OUT_DIR
// between runs, so deleting frontend/dist without touching a watched input - which
// leaves the fingerprint fresh and this script unrun - cannot strand the derive macro
// with no folder to read and a page of unrelated trait errors.
fn mirror_into_embed_directory(source: &Path, destination: &Path) {
    if destination.exists() {
        std::fs::remove_dir_all(destination)
            .unwrap_or_else(|error| fail(&format!("could not clear {}: {error}", destination.display())));
    }
    copy_directory(source, destination);
}

fn copy_directory(source: &Path, destination: &Path) {
    std::fs::create_dir_all(destination)
        .unwrap_or_else(|error| fail(&format!("could not create {}: {error}", destination.display())));

    let entries = std::fs::read_dir(source)
        .unwrap_or_else(|error| fail(&format!("could not read {}: {error}", source.display())));

    for entry in entries {
        let entry =
            entry.unwrap_or_else(|error| fail(&format!("could not read an entry: {error}")));
        let target = destination.join(entry.file_name());
        if entry.path().is_dir() {
            copy_directory(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).unwrap_or_else(|error| {
                fail(&format!("could not copy {}: {error}", entry.path().display()))
            });
        }
    }
}

fn fail(message: &str) -> ! {
    eprintln!("\nfrontend build failed\n{message}\n");
    std::process::exit(1);
}
