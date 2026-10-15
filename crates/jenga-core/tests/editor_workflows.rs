use jenga_core::{
    checks,
    document::{Document, position, property},
    export,
    scene::Scene,
    sexpr::{Node, parse},
};
fn symbol(kind: &str) -> Node {
    parse(&format!("(symbol \"Part\" (property \"Reference\" \"U\" (at 0 0 90)) (property \"Value\" \"Part\" (at 0 0 90)) (symbol \"Part_1_1\" (rectangle (start -2 2) (end 2 -2)) (pin {kind} line (at -4 0 0) (length 2) (name \"IN\") (number \"1\"))))")).unwrap()
}
#[test]
fn labels_keep_distant_pins_connected_without_false_unconnected_warnings() {
    let mut doc = Document::new();
    let def = symbol("passive");
    doc.add_symbol("test:Part", &def, 20., 20., 1);
    doc.add_symbol("test:Part", &def, 80., 20., 1);
    doc.add_text("label", "NET", 16., 20.);
    doc.add_text("label", "NET", 76., 20.);
    assert!(
        !checks::run(&doc)
            .iter()
            .any(|i| i.message.contains("Unconnected"))
    );
}
#[test]
fn flags_conflicting_power_drivers_and_detached_markers() {
    let mut doc = Document::new();
    let def = symbol("power_out");
    doc.add_symbol("test:Part", &def, 20., 20., 1);
    doc.add_symbol("test:Part", &def, 40., 20., 1);
    doc.add_wire((16., 20.), (36., 20.));
    doc.add_point("no_connect", 90., 90.);
    let issues = checks::run(&doc);
    assert!(
        issues
            .iter()
            .any(|i| i.severity == "Error" && i.message.contains("Multiple output"))
    );
    assert!(issues.iter().any(|i| i.message.contains("not attached")));
}
#[test]
fn auto_places_new_fields_and_preserves_pins_through_rotation() {
    let def = symbol("passive");
    let mut doc = Document::new();
    let index = doc.add_symbol("test:Part", &def, 30., 40., 1);
    let instance = &doc.root.items()[index];
    let reference = instance
        .children("property")
        .find(|p| p.value(1) == "Reference")
        .unwrap();
    assert_eq!(position(reference).2, 0.);
    assert_ne!(position(reference).1, 40.);
    let before = Scene::from_document(&doc).pins[0].at;
    for _ in 0..4 {
        doc.rotate(index);
    }
    let after = Scene::from_document(&doc).pins[0].at;
    assert!((before.0 - after.0).abs() < 1e-6 && (before.1 - after.1).abs() < 1e-6);
}
#[test]
fn multiunit_parts_share_reference_and_bom_has_one_component() {
    let mut def = symbol("passive");
    def.items_mut()
        .push(parse("(symbol \"Part_2_1\" (rectangle (start -2 2) (end 2 -2)))").unwrap());
    let mut doc = Document::new();
    let a = doc.add_symbol("test:Part", &def, 30., 40., 1);
    let b = doc.add_symbol("test:Part", &def, 50., 40., 2);
    assert_eq!(
        property(&doc.root.items()[a], "Reference"),
        property(&doc.root.items()[b], "Reference")
    );
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bom.csv");
    export::bom(&doc, &path).unwrap();
    assert_eq!(std::fs::read_to_string(path).unwrap().lines().count(), 2);
}
#[test]
fn exports_respect_paper_size_and_reject_missing_geometry() {
    let mut doc = Document::new();
    doc.root
        .set_child(parse("(paper \"A3\" portrait)").unwrap());
    doc.add_symbol("test:Part", &symbol("passive"), 30., 40., 1);
    let dir = tempfile::tempdir().unwrap();
    let svg = dir.path().join("sheet.svg");
    let pdf = dir.path().join("sheet.pdf");
    export::svg(&doc, &svg).unwrap();
    export::pdf(&doc, &pdf).unwrap();
    assert!(
        std::fs::read_to_string(svg)
            .unwrap()
            .contains("viewBox=\"0 0 297 420\"")
    );
    let text = std::fs::read_to_string(&pdf).unwrap();
    let box_text = text
        .split("/MediaBox [")
        .nth(1)
        .unwrap()
        .split(']')
        .next()
        .unwrap();
    let dimensions: Vec<f64> = box_text
        .split_whitespace()
        .map(|n| n.parse().unwrap())
        .collect();
    assert!((dimensions[2] - 841.8898).abs() < 0.001 && (dimensions[3] - 1190.5512).abs() < 0.001);
    doc.root
        .child_mut("lib_symbols")
        .unwrap()
        .items_mut()
        .truncate(1);
    assert!(export::pdf(&doc, &pdf).is_err());
}
#[test]
fn footprints_produce_pad_and_outline_geometry() {
    let root=parse("(footprint \"Test\" (fp_rect (start -2 -2) (end 2 2) (layer \"F.SilkS\")) (pad \"1\" thru_hole circle (at 0 0) (size 2 2) (drill 1)))").unwrap();
    let scene = Scene::footprint_preview(&root);
    assert!(scene.draws.len() >= 3);
    assert!(scene.missing.is_empty());
}
