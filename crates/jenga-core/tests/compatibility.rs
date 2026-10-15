use jenga_core::{
    document::{Document, at},
    scene::Scene,
    sexpr::parse,
};
#[test]
fn off_grid_erc_reports_exact_endpoint_and_fix_without_moving_it() {
    let mut doc = Document::new();
    doc.add_wire((10.16, 10.16), (113.665, 10.16));
    let before = doc.root.clone();
    let issues = jenga_core::checks::findings(&doc);
    let issue = issues
        .iter()
        .find(|i| i.message.contains("Off connection grid"))
        .unwrap();
    assert_eq!(issue.location, Some((113.665, 10.16)));
    assert!(issue.uuid.is_some());
    assert!(issue.suggestion.contains("1.27"));
    assert_eq!(doc.root, before);
}
#[test]
fn clipboard_preserves_kicad_local_library_alias() {
    let mut doc = Document::new();
    let def=parse("(symbol \"Local\" (symbol \"Local_1_1\" (pin passive line (at 0 0 0) (length 0) (number \"1\"))))").unwrap();
    doc.root
        .child_mut("lib_symbols")
        .unwrap()
        .items_mut()
        .push(def);
    let mut n=parse("(symbol (lib_name \"Local\") (lib_id \"Library:Different\") (unit 1) (property \"Reference\" \"U1\") (property \"Value\" \"Local\"))").unwrap();
    n.set_child(at(10.16, 10.16, 0.));
    n.set_child(jenga_core::document::id());
    doc.root.items_mut().push(n);
    let text = jenga_core::clipboard::copy(&doc, None).unwrap();
    let mut dest = Document::new();
    jenga_core::clipboard::paste(&mut dest, &text, None).unwrap();
    assert!(Scene::from_document(&dest).missing.is_empty());
    assert_eq!(Scene::from_document(&dest).pins.len(), 1);
}
#[test]
fn embedded_alias_overrides_installed_library_identifier() {
    let mut doc = Document::new();
    let def=parse("(symbol \"GND_1\" (power) (property \"Reference\" \"#PWR\" (at 0 0 0)) (symbol \"GND_1_1_1\" (pin power_in line (at 0 0 270) (length 0) (name \"GND\") (number \"1\"))))").unwrap();
    doc.root
        .child_mut("lib_symbols")
        .unwrap()
        .items_mut()
        .push(def);
    let mut instance=parse("(symbol (lib_name \"GND_1\") (lib_id \"power:GND\") (unit 1) (property \"Reference\" \"#PWR1\") (future_metadata yes))").unwrap();
    instance.set_child(at(10., 20., 0.));
    doc.root.items_mut().push(instance);
    let scene = Scene::from_document(&doc);
    assert!(scene.missing.is_empty());
    assert_eq!(scene.pins.len(), 1);
    assert_eq!(scene.pins[0].at, (10., 20.));
    assert_eq!(parse(&doc.root.document_text()).unwrap(), doc.root);
}
#[test]
fn save_repairs_inherited_body_names_without_changing_uuid_or_unknown_fields() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Alias.kicad_sch");
    let mut doc = Document::new();
    let uuid = doc.root_uuid();
    doc.root.child_mut("lib_symbols").unwrap().items_mut().push(parse("(symbol \"Timer:Alias\" (future_metadata yes) (symbol \"Base_1_1\" (rectangle (start -2 2) (end 2 -2))))").unwrap());
    doc.save(&path).unwrap();
    let loaded = Document::open(&path).unwrap();
    let def = loaded
        .root
        .child("lib_symbols")
        .unwrap()
        .children("symbol")
        .next()
        .unwrap();
    assert_eq!(def.children("symbol").next().unwrap().value(1), "Alias_1_1");
    assert!(def.child("future_metadata").is_some());
    assert_eq!(loaded.root_uuid(), uuid);
}
#[test]
fn save_blocks_external_overwrite_and_allows_separate_copy() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("shared.kicad_sch");
    let mut first = Document::new();
    first.save(&path).unwrap();
    let mut second = Document::open(&path).unwrap();
    first.add_text("text", "external", 10., 20.);
    first.save(&path).unwrap();
    second.add_text("text", "human", 20., 20.);
    assert!(second.save(&path).is_err());
    assert!(std::fs::read_to_string(&path).unwrap().contains("external"));
    second.save(&dir.path().join("separate.kicad_sch")).unwrap();
}
