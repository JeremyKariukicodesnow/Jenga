use std::{env, fs, path::PathBuf};
fn main() {
    let source =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../resources/fonts/newstroke_font.cpp");
    println!("cargo:rerun-if-changed={}", source.display());
    let text = fs::read_to_string(&source).expect("KiCad NewStroke font data");
    let mut generated = String::from(
        "// Generated from KiCad 10.0.0 NewStroke; upstream notices in resources/fonts.\npub const GLYPHS: &[&str] = &[\n",
    );
    let mut count = 0;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('"')
            && let Some(end) = line.rfind("\",")
        {
            generated.push_str(&line[..end + 1]);
            generated.push_str(",\n");
            count += 1;
        }
    }
    assert!(count > 60000, "Unexpected NewStroke glyph count: {count}");
    generated.push_str("];\n");
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("newstroke.rs"),
        generated,
    )
    .unwrap();
}
