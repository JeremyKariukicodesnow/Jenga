use jenga_core::{
    document::{Document, at},
    scene::Scene,
    sexpr::parse,
};
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
