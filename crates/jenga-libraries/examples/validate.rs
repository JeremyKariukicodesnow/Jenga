//! Developer validation: cargo run -p jenga-libraries --example validate -- <output>
use anyhow::Result;
use jenga_core::{document::Document, export, scene::Scene};
use jenga_libraries::Libraries;
use std::path::PathBuf;
fn main() -> Result<()> {
    let output = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(".cache/validation"));
    std::fs::create_dir_all(&output)?;
    let mut libs = Libraries::discover(&[], None);
    libs.index()?;
    println!(
        "Indexed {} symbols from {} sources ({} warnings)",
        libs.entries.len(),
        libs.sources.len(),
        libs.warnings.len()
    );
    let mut doc = Document::new();
    doc.path = Some(output.join("Jenga-demo.kicad_sch"));
    let r = libs.symbol("Device:R")?;
    let led = libs.symbol("Device:LED")?;
    let ground = libs.symbol("power:GND")?;
    let supply = libs.symbol("power:+5V")?;
    let r_index = doc.add_symbol("Device:R", &r, 101.6, 76.2, 1);
    doc.set_property(r_index, "Value", "330");
    let led_index = doc.add_symbol("Device:LED", &led, 101.6, 101.6, 1);
    doc.rotate(led_index);
    doc.add_symbol("power:+5V", &supply, 101.6, 63.5, 1);
    doc.add_symbol("power:GND", &ground, 101.6, 114.3, 1);
    // Wire the real pin coordinates rather than approximate symbol extents.
    let scene = Scene::from_document(&doc);
    let mut pins = scene.pins.clone();
    pins.sort_by(|a, b| a.at.1.total_cmp(&b.at.1));
    for pair in pins.windows(2) {
        if pair[0].object != pair[1].object {
            doc.add_wire(pair[0].at, pair[1].at);
        }
    }
    doc.add_text("text", "Jenga · LED circuit", 80., 45.);
    doc.add_text("label", "LED_SUPPLY", 101.6, 69.85);
    let path = output.join("Jenga-demo.kicad_sch");
    doc.save(&path)?;
    let loaded = Document::open(&path)?;
    assert_eq!(loaded.root, doc.root);
    assert_eq!(Scene::from_document(&loaded).pins.len(), 6);
    export::svg(&loaded, &output.join("Jenga-demo.svg"))?;
    export::pdf(&loaded, &output.join("Jenga-demo.pdf"))?;
    export::bom(&loaded, &output.join("Jenga-demo.csv"))?;
    let report = jenga_core::checks::run(&loaded);
    println!(
        "Demo generated: {} scene primitives, {} pins, {} basic ERC issue(s)",
        Scene::from_document(&loaded).draws.len(),
        Scene::from_document(&loaded).pins.len(),
        report.len()
    );
    // Exercise real KiCad-authored files, retaining their entire semantic trees.
    let demo_root = PathBuf::from("C:/Program Files/KiCad/10.0/share/kicad/demos");
    let mut tested = 0;
    fn scan(path: &std::path::Path, files: &mut Vec<PathBuf>) {
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    scan(&p, files);
                } else if p.extension().is_some_and(|s| s == "kicad_sch") {
                    files.push(p);
                }
            }
        }
    }
    let mut files = vec![];
    scan(&demo_root, &mut files);
    for file in files.iter().take(12) {
        let mut d = Document::open(file)?;
        let root = d.root.clone();
        let out = output.join(format!("fixture-{tested}.kicad_sch"));
        d.save(&out)?;
        assert_eq!(Document::open(&out)?.root, root);
        let scene = Scene::from_document(&d);
        println!(
            "Round trip {}: {} shapes / {} missing symbols",
            file.file_name().unwrap().to_string_lossy(),
            scene.draws.len(),
            scene.missing.len()
        );
        tested += 1;
    }
    println!("{tested} real KiCad fixtures preserved without semantic data loss");
    Ok(())
}
