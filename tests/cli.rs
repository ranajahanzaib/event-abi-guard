use serde_json::Value;
use std::process::{Command, Output};
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_eventabi"))
        .args(args)
        .output()
        .unwrap()
}
fn check(example: &str, extra: &[&str]) -> Output {
    let path = format!(
        "{}/examples/{example}/manifest.yaml",
        env!("CARGO_MANIFEST_DIR")
    );
    let mut args = vec!["check", "--manifest", path.as_str()];
    args.extend(extra);
    run(&args)
}
#[test]
fn breaking_exit_one() {
    assert_eq!(check("basic", &[]).status.code(), Some(1));
}
#[test]
fn compatible_exit_zero() {
    assert!(check("compatible", &[]).status.success());
}
#[test]
fn invalid_exit_two_even_never() {
    assert_eq!(
        run(&["check", "--manifest", "missing.yaml", "--fail-on", "never"])
            .status
            .code(),
        Some(2)
    );
}
#[test]
fn never_overrides_violations() {
    assert!(check("basic", &["--fail-on", "never"]).status.success());
}
#[test]
fn warning_policy() {
    let o = run(&[
        "diff",
        "--old",
        "examples/basic/old.json",
        "--new",
        "examples/basic/new.json",
        "--fail-on",
        "warning",
    ]);
    assert_eq!(o.status.code(), Some(1));
}
#[test]
fn diff_default_not_consumer_failure() {
    assert!(
        run(&[
            "diff",
            "--old",
            "examples/basic/old.json",
            "--new",
            "examples/basic/new.json"
        ])
        .status
        .success()
    );
}
#[test]
fn json_is_clean_with_annotations() {
    let o = check("basic", &["--format", "json", "--github-annotations"]);
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["summary"]["breaking"], 1);
    assert!(
        String::from_utf8(o.stderr)
            .unwrap()
            .contains("::error title=EAG001::")
    );
}
#[test]
fn example_consumers_differ() {
    let o = check("multi-consumer", &["--format", "json"]);
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    let violations: Vec<_> = v["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["severity"] == "breaking")
        .collect();
    assert_eq!(violations.len(), 2);
    assert!(
        violations
            .iter()
            .all(|f| f["consumer"] != "monitor" && f["consumer"] != "positional-decoder")
    );
}
#[test]
fn indexed_example_only_decoder_breaks() {
    let o = check("indexed-change", &["--format", "json"]);
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["summary"]["breaking"], 1);
    assert!(
        v["findings"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|f| f["severity"] == "breaking")
            .all(|f| f["consumer"] == "decoder")
    );
}
#[test]
fn output_files() {
    let dir = std::env::temp_dir().join(format!("eventabi-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let json = dir.join("report.json");
    let md = dir.join("report.md");
    let o = check(
        "basic",
        &[
            "--json-out",
            json.to_str().unwrap(),
            "--markdown-out",
            md.to_str().unwrap(),
        ],
    );
    assert_eq!(o.status.code(), Some(1));
    let v: Value = serde_json::from_slice(&std::fs::read(json).unwrap()).unwrap();
    assert_eq!(v["schema_version"], 1);
    assert!(
        std::fs::read_to_string(md)
            .unwrap()
            .contains("billing\\-indexer")
    );
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn empty_diff_json() {
    let o = run(&[
        "diff",
        "--old",
        "examples/basic/old.json",
        "--new",
        "examples/basic/old.json",
        "--format",
        "json",
    ]);
    assert!(o.status.success());
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["findings"], serde_json::json!([]));
}
#[test]
fn validate_accepts_breaking_proposal() {
    assert!(
        run(&["validate", "--manifest", "examples/basic/manifest.yaml"])
            .status
            .success()
    );
}
#[test]
fn help_and_version() {
    for arg in ["--help", "--version"] {
        assert!(run(&[arg]).status.success());
    }
    assert_eq!(run(&["check"]).status.code(), Some(2));
}
#[test]
fn markdown_stdout() {
    let o = check("basic", &["--format", "markdown"]);
    assert!(
        String::from_utf8(o.stdout)
            .unwrap()
            .starts_with("# EventABI Guard")
    );
}
#[test]
fn output_io_errors_are_two() {
    assert_eq!(
        check(
            "basic",
            &["--json-out", "/nonexistent/eventabi/report.json"]
        )
        .status
        .code(),
        Some(2)
    );
}
#[test]
fn artifact_snapshot_example() {
    let o = run(&[
        "check",
        "--manifest",
        "examples/foundry/.eventabi/manifest-snapshot.yaml",
        "--format",
        "json",
    ]);
    assert_eq!(o.status.code(), Some(1));
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["summary"]["breaking"], 1);
}
#[test]
fn manifest_relative_paths_independent_of_working_directory() {
    let path = format!(
        "{}/examples/compatible/manifest.yaml",
        env!("CARGO_MANIFEST_DIR")
    );
    let o = Command::new(env!("CARGO_BIN_EXE_eventabi"))
        .current_dir(std::env::temp_dir())
        .args(["check", "--manifest", &path])
        .output()
        .unwrap();
    assert!(o.status.success());
}
