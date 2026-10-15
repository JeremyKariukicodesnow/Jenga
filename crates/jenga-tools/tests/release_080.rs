use jenga_core::{
    document::Document,
    pcb::{self, Board},
    sexpr::Node,
};
use jenga_tools::engine::Engine;
use serde_json::json;
use std::{
    fs,
    time::{Duration, Instant},
};
fn engine() -> (tempfile::TempDir, Engine) {
    let d = tempfile::tempdir().unwrap();
    let mut e = Engine::new(d.path().into());
    let mut doc = Document::new();
    doc.save(&d.path().join("Test.kicad_sch")).unwrap();
    e.doc = Some(doc);
    (d, e)
}
#[test]
fn engineering_review_flags_exact_voltage_and_peak_budget_without_claiming_hardware_validation() {
    let (_dir, mut e) = engine();
    let r=e.request("engineering_review",&json!({"record":{"function":"network terminal","rails":[{"name":"5V","min_v":4.9,"max_v":5.2,"capacity_ma":1000}],"components":[{"reference":"U2","part_number":"EXACT-7V-MIN","rail":"5V","min_v":7.,"max_v":18.,"peak_ma":1200.}]}})).unwrap();
    assert_eq!(r["hardware_validated"], false);
    assert_eq!(r["review_complete"], false);
    assert!(
        r["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["kind"] == "voltage_mismatch")
    );
    assert!(
        r["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["kind"] == "power_budget_exceeded")
    );
    assert!(
        e.request("engineering_review", &json!({"record":{}}))
            .is_err()
    );
    assert!(
        e.request(
            "engineering_review",
            &json!({"expected_revision":r["revision"],"record":{}})
        )
        .is_ok()
    );
}
#[test]
fn background_preview_keeps_current_document_unchanged_and_conflicts_on_later_edits() {
    let (_dir, mut e) = engine();
    let old = jenga_tools::journal::revision(e.doc.as_ref().unwrap());
    let id = uuid::Uuid::new_v4().to_string();
    let r=e.request("preview_edit",&json!({"background":true,"operation_id":id,"actions":[{"op":"text","text":"background","at":[20,20]}]})).unwrap();
    assert_eq!(r["status"], "pending");
    let start = Instant::now();
    let done = loop {
        let s = e
            .request("operation_status", &json!({"operation_id":id}))
            .unwrap();
        if s["status"] != "pending" {
            break s;
        }
        assert!(start.elapsed() < Duration::from_secs(20));
        std::thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(done["status"], "completed", "{done}");
    assert_eq!(old, jenga_tools::journal::revision(e.doc.as_ref().unwrap()));
    e.request(
        "edit",
        &json!({"actions":[{"op":"text","text":"human edit","at":[40,40]}]}),
    )
    .unwrap();
    assert!(
        e.request(
            "commit_preview",
            &json!({"preview_id":done["result"]["preview_id"]})
        )
        .is_err()
    );
}
#[test]
fn cancellation_is_acknowledged_at_checkpoint_and_never_replays() {
    let id = uuid::Uuid::new_v4().to_string();
    let p = json!({"operation_id":id});
    jenga_tools::operations::begin(&id, "pcb_preview", &p).unwrap();
    jenga_tools::operations::cancel(&id).unwrap();
    let id2 = id.clone();
    std::thread::spawn(move || {
        jenga_tools::operations::context(&id2, 10000);
        assert!(jenga_tools::operations::checkpoint().is_err());
    })
    .join()
    .unwrap();
    jenga_tools::operations::finish(&id, &Err(anyhow::anyhow!("cancelled"))).unwrap();
    assert_eq!(
        jenga_tools::operations::begin(&id, "pcb_preview", &p)
            .unwrap()
            .unwrap()["status"],
        "failed"
    );
}
#[test]
fn project_copy_move_and_imported_assets_preserve_original_files() {
    let dir = tempfile::tempdir().unwrap();
    let mut e = Engine::new(dir.path().into());
    e.request(
        "create_project",
        &json!({"name":"Source","folder":dir.path()}),
    )
    .unwrap();
    let source = dir.path().join("Source");
    fs::create_dir(source.join("models")).unwrap();
    fs::write(source.join("models/a.wrl"), "#VRML V2.0 utf8\n").unwrap();
    let imported = e
        .request(
            "import_project",
            &json!({"path":source.join("Source.kicad_pro"),"folder":dir.path().join("copies")}),
        )
        .unwrap();
    let copied = std::path::PathBuf::from(imported["folder"].as_str().unwrap());
    assert!(source.join("Source.kicad_sch").is_file());
    assert!(copied.join("models/a.wrl").is_file());
    let moved = e
        .request("move_project", &json!({"folder":dir.path().join("moved")}))
        .unwrap();
    assert!(!copied.exists());
    assert!(std::path::Path::new(moved["recovery_folder"].as_str().unwrap()).is_dir());
    let independent = dir.path().join("Independent.kicad_sch");
    Document::new().save(&independent).unwrap();
    let r = e
        .request("import_document", &json!({"path":independent}))
        .unwrap();
    assert!(std::path::Path::new(r["path"].as_str().unwrap()).is_file());
    assert_eq!(r["linked_schematic"], false);
}
#[test]
fn user_asset_import_retains_source_and_registers_local_library() {
    let (dir, mut e) = engine();
    let source = dir.path().join("supplied.kicad_sym");
    fs::write(&source,"(kicad_symbol_lib (version 20250114) (generator kicad_symbol_editor) (symbol \"Part\" (property \"Reference\" \"U\" (at 0 0 0))))").unwrap();
    let r=e.request("import_asset",&json!({"path":source,"name":"Imported","license":"Example license","attribution":"Example manufacturer"})).unwrap();
    assert!(dir.path().join("sym-lib-table").is_file());
    assert!(
        std::path::Path::new(&format!("{}.source.json", r["path"].as_str().unwrap())).is_file()
    );
}
#[test]
fn native_pcb_clipboard_remaps_ids_references_and_nets_atomically() {
    let mut a = Board::new(80., 60.).unwrap();
    let net = a.ensure_net("SIGNAL");
    let mut f = pcb::node(
        "footprint",
        vec![
            Node::string("Library:Part"),
            pcb::uid(),
            pcb::xy("at", (10., 10.)),
            pcb::node(
                "pad",
                vec![
                    Node::string("1"),
                    Node::atom("thru_hole"),
                    Node::atom("circle"),
                    pcb::xy("at", (0., 0.)),
                    pcb::xy("size", (2., 2.)),
                    pcb::node("layers", vec![Node::string("*.Cu")]),
                    pcb::node("net", vec![Node::atom(net), Node::string("SIGNAL")]),
                    pcb::uid(),
                ],
            ),
        ],
    );
    pcb::set_field(&mut f, "Reference", "J1");
    a.root.items_mut().push(f);
    a.segment((10., 10.), (20., 10.), 0.25, "F.Cu", net)
        .unwrap();
    let text = jenga_core::pcb_clipboard::copy(&a, None).unwrap();
    let mut b = Board::new(100., 100.).unwrap();
    b.ensure_net("OTHER");
    jenga_core::pcb_clipboard::paste(&mut b, &text, Some((30., 30.)), false).unwrap();
    assert_eq!(b.pads()[0].at, (30., 30.));
    assert_eq!(b.pads()[0].net_name, "SIGNAL");
    assert_ne!(a.pads()[0].uuid, b.pads()[0].uuid);
    let revision = b.revision();
    assert!(jenga_core::pcb_clipboard::paste(&mut b, &text, None, false).is_err());
    assert_eq!(revision, b.revision());
    let r = jenga_core::pcb_clipboard::paste(&mut b, &text, None, true).unwrap();
    assert_eq!(r["renamed_references"][0]["new"], "J2");
}
#[test]
fn pcb_clipboard_supports_modern_named_nets_and_rejects_unsupported_objects() {
    let mut b = Board::new(80., 60.).unwrap();
    let text = "(kicad_pcb (version 20260206) (segment (start 20 20) (end 30 20) (width 0.25) (layer \"F.Cu\") (net \"TEST\") (uuid \"11111111-1111-4111-8111-111111111111\")))";
    jenga_core::pcb_clipboard::paste(&mut b, text, None, false).unwrap();
    let segment = b.root.children("segment").next().unwrap();
    assert_eq!(b.nets()[&pcb::net_code(segment)], "TEST");
    let before = b.revision();
    assert!(
        jenga_core::pcb_clipboard::paste(
            &mut b,
            "(kicad_pcb (dimension (uuid \"unsupported\")))",
            None,
            false
        )
        .is_err()
    );
    assert_eq!(b.revision(), before);
}
#[test]
fn document_import_namespaces_libraries_and_copies_model_dependencies_once() {
    let (dir, mut e) = engine();
    let source = dir.path().join("source");
    fs::create_dir(&source).unwrap();
    fs::create_dir(source.join("Parts.pretty")).unwrap();
    fs::create_dir(source.join("models")).unwrap();
    fs::write(source.join("models/Test.wrl"), "#VRML V2.0 utf8\n").unwrap();
    fs::write(
        source.join("Parts.pretty/Part.kicad_mod"),
        "(footprint \"Part\" (model \"${KIPRJMOD}/models/Test.wrl\" (offset (xyz 0 0 0))))",
    )
    .unwrap();
    fs::write(
        source.join("fp-lib-table"),
        "(fp_lib_table (lib (name \"Parts\") (type \"KiCad\") (uri \"${KIPRJMOD}/Parts.pretty\")))",
    )
    .unwrap();
    let mut b = Board::new(80., 60.).unwrap();
    b.root.items_mut().push(pcb::node(
        "footprint",
        vec![
            Node::string("Parts:Part"),
            pcb::xy("at", (20., 20.)),
            pcb::uid(),
            pcb::node("model", vec![Node::string("${KIPRJMOD}/models/Test.wrl")]),
        ],
    ));
    b.save(&source.join("Source.kicad_pcb")).unwrap();
    e.request(
        "import_document",
        &json!({"path":source.join("Source.kicad_pcb"),"name":"Imported"}),
    )
    .unwrap();
    let imported = fs::read_to_string(dir.path().join("Imported.kicad_pcb")).unwrap();
    assert!(imported.contains("Imported__Parts:Part"));
    assert!(imported.contains("${KIPRJMOD}/imports/Imported/models/Test.wrl"));
    assert!(!imported.contains("imports/Imported/imports/Imported"));
    assert!(
        dir.path()
            .join("imports/Imported/Parts.pretty/Part.kicad_mod")
            .is_file()
    );
    assert!(
        dir.path()
            .join("imports/Imported/models/Test.wrl")
            .is_file()
    );
}
#[test]
fn previews_respect_native_netclass_even_when_local_rules_are_looser() {
    let (dir, mut e) = engine();
    let mut b = Board::new(80., 60.).unwrap();
    b.path = Some(dir.path().join("Test.kicad_pcb"));
    b.rules.clearance = 0.1;
    fs::write(
        dir.path().join("Test.kicad_pro"),
        json!({"net_settings":{"classes":[{"name":"Default","clearance":0.3}]}}).to_string(),
    )
    .unwrap();
    b.ensure_net("A");
    b.ensure_net("B");
    e.board = Some(b);
    e.request("pcb_edit",&json!({"actions":[{"op":"track","net":"A","points":[[20,20],[50,20]],"layer":"F.Cu","width_mm":0.25}]})).unwrap();
    let r=e.request("pcb_preview",&json!({"actions":[{"op":"track","net":"B","points":[[20,20.45],[50,20.45]],"layer":"F.Cu","width_mm":0.25}]})).unwrap();
    assert_eq!(r["can_commit"], false);
}
