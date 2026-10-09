//! All integration tests, one binary. No test here sets process-global state, so none needs its
//! own process.

mod decode;
mod evdev_parity;
mod feel_presets;
mod fixtures_current;
mod gestures_replay;
mod recorded_calibration;
mod reference_cases;
mod reference_feel;
mod support;

/// Fails when a test file is added but not included above, or left as its own binary beside
/// this one: every `tests/palmrest/*.rs` must be a `mod` here, and `tests/*.rs` must not exist.
#[test]
fn every_test_file_is_included() {
    use std::fs;
    use std::path::PathBuf;

    let tests = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests");
    let rs_files = |dir: PathBuf| -> Vec<String> {
        fs::read_dir(dir)
            .expect("tests dir is readable")
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "rs"))
            .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
            .collect()
    };
    assert_eq!(
        rs_files(tests.clone()),
        Vec::<String>::new(),
        "a tests/*.rs file would be its own binary; move it into tests/palmrest"
    );
    let main = fs::read_to_string(tests.join("palmrest/main.rs")).expect("main.rs");
    for name in rs_files(tests.join("palmrest")) {
        assert!(
            name == "main" || main.contains(&format!("mod {name};")),
            "tests/palmrest/{name}.rs is not included in main.rs"
        );
    }
}
