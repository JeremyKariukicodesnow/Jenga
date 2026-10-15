//! Verify symbol operation using only a release's bundled resource directories.
use anyhow::Result;
use jenga_libraries::{Libraries, Source};
use std::path::PathBuf;

fn main() -> Result<()> {
    let root = PathBuf::from(
        std::env::args()
            .nth(1)
            .expect("pass packaged libraries directory"),
    );
    let mut libraries = Libraries::discover(&[], None);
    // Exclude every installed/global/project source from this test.
    libraries.sources = vec![Source {
        name: "Jenga bundled".into(),
        path: root.join("symbols"),
        kind: "symbols".into(),
    }];
    libraries.index()?;
    assert!(libraries.entries.len() > 20_000);
    assert!(
        libraries
            .entries
            .iter()
            .all(|entry| entry.file.starts_with(&root))
    );
    assert!(libraries.warnings.is_empty());
    for id in ["Device:R", "Device:LED", "power:GND", "power:+5V"] {
        let symbol = libraries.symbol(id)?;
        assert!(symbol.children("symbol").next().is_some());
    }
    println!(
        "Bundled-only check passed: {} symbols; no installed KiCad sources used",
        libraries.entries.len()
    );
    Ok(())
}
