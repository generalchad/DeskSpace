//! Makes a published icon set from its upstream package (D16), and tells the
//! icons workflow what to publish.
//!
//! ```text
//! cargo run -p deskspace-config --example icon_set -- trim <prefix> <upstream icons.json> <out dir>
//! cargo run -p deskspace-config --example icon_set -- spec <prefix>
//! ```
//!
//! `trim` writes `<out dir>/<file name>` and rewrites
//! `assets/icons/<prefix>-names.txt`. It refuses, writing nothing, when the
//! trimmed set lacks a name that file lists: a name once shipped is saved in
//! users' configs and must never disappear.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use deskspace_config::icons::{IconSetSpec, icon_set, missing_names, trim};
use deskspace_core::IconSet;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let outcome = match args.as_slice() {
        ["trim", prefix, upstream, out_dir] => {
            spec(prefix).and_then(|spec| run_trim(spec, Path::new(upstream), Path::new(out_dir)))
        }
        ["spec", prefix] => spec(prefix).map(print_spec),
        _ => Err(
            "usage: icon_set trim <prefix> <upstream icons.json> <out dir> | spec <prefix>".into(),
        ),
    };
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("icon_set: {error}");
            ExitCode::FAILURE
        }
    }
}

fn spec(prefix: &str) -> Result<&'static IconSetSpec> {
    icon_set(prefix).ok_or_else(|| format!("no icon set has the prefix {prefix:?}").into())
}

fn print_spec(spec: &IconSetSpec) {
    println!("tag={}", spec.release_tag());
    println!("file={}", spec.file_name());
    println!("upstream={}@{}", spec.upstream, spec.upstream_version);
}

fn names_file(spec: &IconSetSpec) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/icons")
        .join(format!("{}-names.txt", spec.prefix))
}

fn run_trim(spec: &IconSetSpec, upstream: &Path, out_dir: &Path) -> Result<()> {
    let upstream: IconSet = serde_json::from_slice(&std::fs::read(upstream)?)?;
    let set = trim(upstream, spec);

    let names_path = names_file(spec);
    let shipped = match std::fs::read_to_string(&names_path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error.into()),
    };
    let missing = missing_names(&set, &shipped);
    if !missing.is_empty() {
        for name in &missing {
            eprintln!("missing: {name}");
        }
        return Err(format!(
            "{} shipped names are missing from the trimmed set",
            missing.len()
        )
        .into());
    }

    std::fs::create_dir_all(out_dir)?;
    let out = out_dir.join(spec.file_name());
    std::fs::write(&out, serde_json::to_vec(&set)?)?;

    let mut names = set.names().join("\n");
    names.push('\n');
    if let Some(parent) = names_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&names_path, names)?;

    println!(
        "wrote {} ({} icons, {} aliases) and {}",
        out.display(),
        set.icons.len(),
        set.aliases.len(),
        names_path.display()
    );
    Ok(())
}
