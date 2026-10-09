//! The committed fixtures are exactly what `crate::support::scenarios` synthesises.
//! `PALMREST_REGEN_FIXTURES=1 cargo test --test fixtures_current` rewrites them.

use palmrest::recording;

#[test]
fn committed_fixtures_match_their_scenarios() {
    let regen = std::env::var("PALMREST_REGEN_FIXTURES").is_ok_and(|v| v == "1");
    for (name, records) in crate::support::scenarios::all() {
        let bytes = recording::to_bytes(&records);
        let path = crate::support::fixture_path(name);
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
