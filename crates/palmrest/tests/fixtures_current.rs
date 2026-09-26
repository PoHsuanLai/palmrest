//! The committed fixtures are exactly what `support::scenarios` synthesises.
//! `PALMREST_REGEN_FIXTURES=1 cargo test --test fixtures_current` rewrites them.

mod support;

use palmrest::recording;

#[test]
fn committed_fixtures_match_their_scenarios() {
    let regen = std::env::var("PALMREST_REGEN_FIXTURES").is_ok_and(|v| v == "1");
    for (name, records) in support::scenarios::all() {
        let bytes = recording::to_bytes(&records);
        let path = support::fixture_path(name);
        if regen {
            std::fs::write(&path, &bytes).unwrap();
            continue;
        }
        let on_disk =
            std::fs::read(&path).unwrap_or_else(|e| panic!("{name}: {e}; regenerate the fixtures"));
        assert!(
            on_disk == bytes,
            "{name}.bin differs from its scenario; regenerate the fixtures"
        );
    }
}
