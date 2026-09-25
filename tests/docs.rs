//! Keeps the generated documentation honest.
//! `docs/LEVEL_CATALOG.md` is rendered from the real level data; if a level changes, regenerate it with
//! `cargo run --release --bin catalog > docs/LEVEL_CATALOG.md`.

#[test]
fn level_catalog_is_up_to_date() {
    let committed = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/LEVEL_CATALOG.md")).expect("docs/LEVEL_CATALOG.md must exist");
    // Normalise line endings so a Windows checkout with autocrlf does not fail the comparison.
    let committed = committed.replace("\r\n", "\n");
    let fresh = vfield::catalog::render();
    assert!(
        committed == fresh,
        "docs/LEVEL_CATALOG.md is stale. Regenerate it with:\n    cargo run --release --bin catalog > docs/LEVEL_CATALOG.md"
    );
}

#[test]
fn catalog_lists_every_level_once() {
    let text = vfield::catalog::render();
    for l in vfield::levels::all() {
        let header = format!("## {} - {} (", l.id, l.title);
        assert_eq!(text.matches(&header).count(), 1, "{} must appear exactly once", l.id);
    }
}
