#![allow(dead_code)]

use std::path::PathBuf;

fn fixtures_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Read a fixture by its path relative to `tests/fixtures/`.
pub fn fixture(name: &str) -> String {
    let path = fixtures_root().join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("could not read fixture {}: {e}", path.display()))
}

/// Names of every output fixture in one subdirectory, sorted, relative to
/// `tests/fixtures/` so they can be passed straight to [`fixture`].
pub fn fixtures_in(dir: &str) -> Vec<String> {
    let root = fixtures_root().join(dir);
    let mut names: Vec<String> = std::fs::read_dir(&root)
        .unwrap_or_else(|e| panic!("could not read dir {}: {e}", root.display()))
        .map(|entry| entry.expect("dir entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "out" || e == "wout"))
        .map(|p| format!("{dir}/{}", p.file_name().unwrap().to_string_lossy()))
        .collect();
    names.sort();
    names
}
