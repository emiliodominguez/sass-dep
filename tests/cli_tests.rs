//! End-to-end tests of the `sass-dep` binary.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

/// Runs the binary with `--root <root>` and the given arguments.
fn run(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_sass-dep"))
        .arg("--root")
        .arg(root)
        .args(args)
        .output()
        .expect("failed to run sass-dep")
}

/// Creates a small layered project with one forbidden dependency.
fn project() -> TempDir {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    fs::create_dir_all(root.join("src/components")).unwrap();
    fs::create_dir_all(root.join("src/pages")).unwrap();
    fs::write(
        root.join("src/main.scss"),
        "@use \"components/card\";\n@use \"pages/home\";\n",
    )
    .unwrap();
    fs::write(
        root.join("src/components/_card.scss"),
        "@use \"../pages/home\";\n",
    )
    .unwrap();
    fs::write(
        root.join("src/pages/_home.scss"),
        "@import \"../tokens\";\n",
    )
    .unwrap();
    fs::write(root.join("src/_tokens.scss"), "$x: 1;\n").unwrap();
    fs::write(root.join("src/_unused.scss"), "").unwrap();
    temp
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn analyze_outputs_json_and_formats() {
    let temp = project();
    let output = run(
        temp.path(),
        &["analyze", "src/main.scss", "--include-orphans"],
    );
    assert!(output.status.success());

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["version"], "1.1.0");
    assert_eq!(json["analysis"]["statistics"]["orphan_files"], 1);

    for (format, prefix) in [
        ("dot", "digraph"),
        ("mermaid", "graph LR"),
        ("d2", "direction: right"),
    ] {
        let output = run(
            temp.path(),
            &["analyze", "src/main.scss", "--format", format],
        );
        assert!(output.status.success());
        assert!(stdout(&output).starts_with(prefix), "{format}");
    }
}

#[test]
fn config_entry_points_and_rules() {
    let temp = project();
    fs::write(
        temp.path().join(".sass-dep.toml"),
        r#"
entry_points = ["src/main.scss"]

[check]
no_cycles = true

[[rules]]
name = "layers"
from = "src/components/**"
deny = "src/pages/**"
message = "components must stay page-agnostic"
"#,
    )
    .unwrap();

    let output = run(temp.path(), &["check", "--format", "json"]);
    assert_eq!(output.status.code(), Some(1));

    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let violations = report["violations"].as_array().unwrap();
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0]["kind"], "rule");
    assert_eq!(violations[0]["file"], "src/components/_card.scss");
    assert_eq!(violations[0]["line"], 1);
}

#[test]
fn check_flags_and_github_format() {
    let temp = project();

    let ok = run(temp.path(), &["check", "src/main.scss", "--no-cycles"]);
    assert_eq!(ok.status.code(), Some(0));

    let output = run(
        temp.path(),
        &[
            "check",
            "src/main.scss",
            "--no-legacy-imports",
            "--no-orphans",
            "--format",
            "github",
        ],
    );
    assert_eq!(output.status.code(), Some(1));
    let text = stdout(&output);
    assert!(text
        .contains("::error file=src/pages/_home.scss,line=1,col=1,title=sass-dep legacy-import::"));
    assert!(text.contains("::error file=src/_unused.scss,title=sass-dep orphan::"));
}

#[test]
fn exit_codes_for_errors() {
    let temp = project();

    // No entry points anywhere -> usage error.
    assert_eq!(run(temp.path(), &["analyze"]).status.code(), Some(2));
    // Missing entry file -> IO error.
    assert_eq!(
        run(temp.path(), &["analyze", "nope.scss"]).status.code(),
        Some(3)
    );
    // Invalid config -> configuration error.
    fs::write(temp.path().join(".sass-dep.toml"), "unknown_key = 1\n").unwrap();
    assert_eq!(
        run(temp.path(), &["analyze", "src/main.scss"])
            .status
            .code(),
        Some(2)
    );
}

#[test]
fn why_and_impact() {
    let temp = project();

    let why = run(
        temp.path(),
        &["why", "src/_tokens.scss", "-e", "src/main.scss"],
    );
    assert!(why.status.success());
    let text = stdout(&why);
    assert!(text.starts_with("src/main.scss\n"));
    assert!(text.contains("@import (line 1) → src/_tokens.scss"));

    let impact = run(
        temp.path(),
        &[
            "impact",
            "src/_tokens.scss",
            "README.md",
            "-e",
            "src/main.scss",
            "--format",
            "json",
        ],
    );
    assert!(impact.status.success());
    let report: serde_json::Value = serde_json::from_slice(&impact.stdout).unwrap();
    assert_eq!(report["changed"], serde_json::json!(["src/_tokens.scss"]));
    assert_eq!(report["ignored"], serde_json::json!(["README.md"]));
    assert_eq!(
        report["affected"],
        serde_json::json!([
            "src/components/_card.scss",
            "src/main.scss",
            "src/pages/_home.scss"
        ])
    );
    assert_eq!(report["entry_points"], serde_json::json!(["src/main.scss"]));
}

#[test]
fn init_detects_entry_points() {
    let temp = project();
    let output = run(temp.path(), &["init"]);
    assert!(output.status.success());

    let config = fs::read_to_string(temp.path().join(".sass-dep.toml")).unwrap();
    assert!(config.contains("\"src/main.scss\""));
    assert!(!config.contains("_unused"));

    // Refuses to overwrite without --force; the generated config is usable.
    assert_eq!(run(temp.path(), &["init"]).status.code(), Some(2));
    assert!(run(temp.path(), &["analyze", "-q"]).status.success());
}

#[test]
fn export_roundtrip_from_stdin() {
    let temp = project();
    let analysis = run(temp.path(), &["analyze", "src/main.scss"]);

    let mut child = Command::new(env!("CARGO_BIN_EXE_sass-dep"))
        .args(["export", "-", "--format", "mermaid"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    std::io::Write::write_all(child.stdin.as_mut().unwrap(), &analysis.stdout).unwrap();
    let output = child.wait_with_output().unwrap();

    assert!(output.status.success());
    assert!(stdout(&output).contains("classDef entry"));
}
