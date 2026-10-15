use jenga_core::{document::Document, scene::Scene};
use jenga_libraries::{Libraries, Source};
use jenga_tools::{
    engine::{Engine, edit},
    journal,
};
use serde_json::{Value, json};

fn symbol(name: &str, number: &str, y: f64) -> jenga_core::sexpr::Node {
    jenga_tools::symbol_editor::symbol(&json!({"name":name,"draw_body":false,"pins":[{"number":number,"name":"Signal","at":[-5.08,y],"length_mm":2.54}]})).unwrap()
}
fn fixture() -> (tempfile::TempDir, Engine, Vec<Value>) {
    let dir = tempfile::tempdir().unwrap();
    let mut e = Engine::new(dir.path().into());
    let mut d = Document::new();
    d.path = Some(dir.path().join("Circuit.kicad_sch"));
    for (j, (x, y)) in [
        (30.48, 30.48),
        (60.96, 30.48),
        (30.48, 60.96),
        (60.96, 60.96),
    ]
    .iter()
    .enumerate()
    {
        let i = d.add_symbol("Test:Old", &symbol("Old", "1", 0.), *x, *y, 1);
        d.set_property(i, "Reference", &format!("U{}", j + 1));
    }
    let endpoints = Scene::from_document(&d)
        .pins
        .iter()
        .map(|p| json!({"reference":p.reference,"number":p.number}))
        .collect();
    e.doc = Some(d);
    e.libraries = Some(Libraries::discover(&[], None));
    (dir, e, endpoints)
}
#[test]
fn whole_block_preview_commit_and_idempotent_receipt() {
    let (_dir, mut e, p) = fixture();
    let base = journal::revision(e.document().unwrap());
    let request = json!({"connections":[{"from":p[0],"to":p[1]},{"from":p[2],"to":p[3]}],"dry_run":true,"expected_revision":base});
    let preview = e.request("connect_block", &request).unwrap();
    assert_eq!(preview["can_commit"], true, "{preview}");
    assert!(std::path::Path::new(preview["preview_image_path"].as_str().unwrap()).is_file());
    assert_eq!(journal::revision(e.document().unwrap()), base);
    assert!(!preview["changes"]["added"].as_array().unwrap().is_empty());
    let id = uuid::Uuid::new_v4().to_string();
    let commit = json!({"preview_id":preview["preview_id"],"operation_id":id});
    let result = e.request("commit_preview", &commit).unwrap();
    assert_eq!(result["status"], "completed");
    let after = journal::revision(e.document().unwrap());
    assert_ne!(after, base);
    let repeated = e.request("commit_preview", &commit).unwrap();
    assert_eq!(repeated, result);
    assert_eq!(journal::revision(e.document().unwrap()), after);
    assert_eq!(
        e.request("operation_status", &json!({"operation_id":id}))
            .unwrap()["result"]["revision"],
        after
    );
    assert_eq!(e.request("verify",&json!({"expected_connections":request["connections"],"forbidden_connections":[{"from":p[0],"to":p[2]}]})).unwrap()["connections"]["passed"],true);
    assert!(
        e.request(
            "commit_preview",
            &json!({"preview_id":preview["preview_id"]})
        )
        .is_err()
    );
}
#[test]
fn batch_failure_is_atomic_and_preview_reports_each_exact_pin() {
    let (_dir, mut e, p) = fixture();
    let before = journal::revision(e.document().unwrap());
    let mut request = json!({"connections":[{"from":p[0],"to":p[1]},{"from":{"reference":"MissingA","number":"9"},"to":p[2]},{"from":{"reference":"MissingB","number":"8"},"to":p[3]}]});
    assert!(e.request("connect_block", &request).is_err());
    assert_eq!(journal::revision(e.document().unwrap()), before);
    request["dry_run"] = json!(true);
    let preview = e.request("connect_block", &request).unwrap();
    assert_eq!(preview["can_commit"], false);
    assert_eq!(preview["failures"][0]["index"], 1);
    assert_eq!(preview["failures"][1]["index"], 2);
    assert!(preview["failures"].to_string().contains("MissingA"));
    assert!(
        e.request(
            "commit_preview",
            &json!({"preview_id":preview["preview_id"]})
        )
        .is_err()
    );
    assert_eq!(journal::revision(e.document().unwrap()), before);
}
#[test]
fn named_short_preview_cannot_be_committed() {
    let (_dir, mut e, _) = fixture();
    let d = e.doc.as_mut().unwrap();
    d.add_text("label", "VCC", 10.16, 10.16);
    d.add_text("label", "GND", 20.32, 10.16);
    let before = journal::revision(d);
    let preview = e
        .request(
            "preview_edit",
            &json!({"actions":[{"op":"wire","points":[[10.16,10.16],[20.32,10.16]]}]}),
        )
        .unwrap();
    assert_eq!(preview["can_commit"], false);
    assert_eq!(preview["failures"][0]["kind"], "unintended_short");
    assert_eq!(journal::revision(e.document().unwrap()), before);
}
#[test]
fn receipt_survives_new_engine_and_rejects_different_request() {
    let (dir, mut e, _) = fixture();
    let id = uuid::Uuid::new_v4().to_string();
    let p = json!({"operation_id":id,"actions":[{"op":"text","text":"one","at":[10.16,10.16]}]});
    let result = e.request("edit", &p).unwrap();
    let mut second = Engine::new(dir.path().into());
    assert_eq!(second.request("edit", &p).unwrap(), result);
    assert!(second.doc.is_none());
    let mut different = p.clone();
    different["actions"][0]["text"] = json!("two");
    assert!(second.request("edit", &different).is_err());
    let failid = uuid::Uuid::new_v4().to_string();
    assert!(
        e.request(
            "edit",
            &json!({"operation_id":failid,"actions":[{"op":"bogus"}]})
        )
        .is_err()
    );
    assert_eq!(
        second
            .request("operation_status", &json!({"operation_id":failid}))
            .unwrap()["status"],
        "failed"
    );
}
#[test]
fn replacement_maps_moved_pin_preserves_wires_and_other_instances() {
    let (dir, mut e, p) = fixture();
    e.request(
        "connect_block",
        &json!({"connections":[{"from":p[0],"to":p[1]}]}),
    )
    .unwrap();
    let other = Scene::from_document(e.document().unwrap())
        .pins
        .iter()
        .find(|p| p.reference == "U2")
        .unwrap()
        .at;
    let path = dir.path().join("Test.kicad_sym");
    std::fs::write(
        &path,
        format!(
            "(kicad_symbol_lib (version 20231120) {})",
            symbol("New", "7", 2.54).encoded()
        ),
    )
    .unwrap();
    let libs = e.libraries.as_mut().unwrap();
    libs.sources = vec![Source {
        name: "Test".into(),
        path,
        kind: "symbols".into(),
    }];
    libs.index().unwrap();
    let result=e.request("edit",&json!({"actions":[{"op":"replace_symbol","reference":"U1","library_id":"Test:New","pin_map":{"1":"7"}}]})).unwrap();
    assert_eq!(result["results"][0]["pin_map"]["1"], "7");
    let count = e
        .document()
        .unwrap()
        .root
        .child("lib_symbols")
        .unwrap()
        .children("symbol")
        .count();
    e.request(
        "edit",
        &json!({"actions":[{"op":"refresh_symbol","reference":"U1"}]}),
    )
    .unwrap();
    assert_eq!(
        e.document()
            .unwrap()
            .root
            .child("lib_symbols")
            .unwrap()
            .children("symbol")
            .count(),
        count
    );
    assert_eq!(
        e.request(
            "verify_connections",
            &json!({"expected_connections":[{"from":{"reference":"U1","number":"7"},"to":p[1]}]})
        )
        .unwrap()["passed"],
        true
    );
    assert_eq!(
        Scene::from_document(e.document().unwrap())
            .pins
            .iter()
            .find(|p| p.reference == "U2")
            .unwrap()
            .at,
        other
    );
}
#[test]
fn ambiguous_replacement_rolls_back_and_reports_pin() {
    let (dir, mut e, _) = fixture();
    let def=jenga_tools::symbol_editor::symbol(&json!({"name":"Ambiguous","draw_body":false,"pins":[{"number":"7","name":"Signal","at":[-5.08,0]},{"number":"8","name":"Signal","at":[-5.08,5.08]}]})).unwrap();
    let path = dir.path().join("Test.kicad_sym");
    std::fs::write(
        &path,
        format!("(kicad_symbol_lib (version 20231120) {})", def.encoded()),
    )
    .unwrap();
    let libs = e.libraries.as_mut().unwrap();
    libs.sources = vec![Source {
        name: "Test".into(),
        path,
        kind: "symbols".into(),
    }];
    libs.index().unwrap();
    let before = journal::revision(e.document().unwrap());
    let error=e.request("edit",&json!({"actions":[{"op":"replace_symbol","reference":"U1","library_id":"Test:Ambiguous"}]})).unwrap_err().to_string();
    assert!(error.contains("pin 1"), "{error}");
    assert_eq!(journal::revision(e.document().unwrap()), before);
}
#[test]
fn compact_inspection_delta_and_combined_verification() {
    let (_dir, mut e, p) = fixture();
    let base = journal::revision(e.document().unwrap());
    let compact = e
        .request(
            "inspect",
            &json!({"references":["U2"],"include_pins":false}),
        )
        .unwrap();
    assert_eq!(compact["objects"].as_array().unwrap().len(), 1);
    assert!(compact["objects"][0].get("pins").is_none());
    e.request(
        "edit",
        &json!({"actions":[{"op":"wire","points":[[10.16,10.16],[20.32,10.16]]}]}),
    )
    .unwrap();
    let delta = e
        .request("inspect", &json!({"since_revision":base}))
        .unwrap();
    assert!(delta.get("objects").is_none());
    assert_eq!(delta["changes"]["added"].as_array().unwrap().len(), 1);
    let verify=e.request("verify",&json!({"expected_connections":[{"from":p[0],"to":{"reference":"Missing","number":"1"}}]})).unwrap();
    assert_eq!(verify["passed"], false);
    assert_eq!(verify["dangling_wires"].as_array().unwrap().len(), 2);
    assert_eq!(verify["kicad"]["native_roundtrip"], true);
    assert!(verify["connections"]["findings"][0].get("error").is_some());
}
#[test]
fn cleanup_preserves_branch_connectivity_and_removes_only_orphans() {
    let (_dir, mut e, p) = fixture();
    e.request(
        "connect_block",
        &json!({"connections":[{"from":p[0],"to":p[1]}]}),
    )
    .unwrap();
    let d = e.doc.as_mut().unwrap();
    let wire = d.root.children("wire").next().unwrap().clone();
    d.root.items_mut().push(wire);
    d.add_point("junction", 200., 200.);
    d.add_wire((100.33, 100.33), (110.49, 100.33));
    d.add_wire((110.49, 100.33), (120.65, 100.33));
    let before = jenga_tools::net_guard::signature(d);
    let mut libs = Libraries::discover(&[], None);
    let result = edit(d, &mut libs, &json!({"actions":[{"op":"cleanup"}]})).unwrap();
    assert_eq!(result["results"][0]["removed"], 2);
    assert!(result["results"][0]["segments_merged"].as_u64().unwrap() >= 1);
    assert_eq!(before, jenga_tools::net_guard::signature(d));
}
#[test]
fn reserved_corridor_is_respected() {
    let d = Document::new();
    let scene = Scene::from_document(&d);
    let path = jenga_tools::routing::route_options(
        &d,
        &scene,
        (10.16, 10.16),
        (40.64, 10.16),
        true,
        &json!({"reserved_corridors":[{"from":[20.,5.],"to":[30.,15.]}]}),
    )
    .unwrap();
    assert!(path.iter().any(|p| p.1 < 5. || p.1 > 15.));
}
#[test]
fn label_connected_pins_still_receive_visible_routes() {
    let (_dir, mut e, p) = fixture();
    let pins = Scene::from_document(e.document().unwrap()).pins;
    for pin in &pins[..2] {
        e.doc
            .as_mut()
            .unwrap()
            .add_text("label", "Signal", pin.at.0, pin.at.1);
    }
    assert_eq!(
        e.request("trace_connection", &json!({"from":p[0],"to":p[1]}))
            .unwrap()["connected"],
        true
    );
    e.request(
        "connect_block",
        &json!({"connections":[{"from":p[0],"to":p[1]}]}),
    )
    .unwrap();
    assert!(e.document().unwrap().root.children("wire").count() > 0);
}
#[test]
fn project_recovery_refuses_collision_then_restores_native_files() {
    let dir = tempfile::tempdir().unwrap();
    let d = jenga_core::document::create_project(dir.path(), "Recovery").unwrap();
    let root = d.path.as_ref().unwrap().clone();
    let archive = jenga_tools::projects::archive_project(&d).unwrap();
    assert!(!root.exists());
    std::fs::create_dir_all(root.parent().unwrap()).unwrap();
    std::fs::write(&root, "collision").unwrap();
    let p = json!({"restore_manifest":archive["restore_manifest"]});
    assert!(jenga_tools::projects::restore_project(&p).is_err());
    std::fs::remove_file(&root).unwrap();
    std::fs::remove_dir(root.parent().unwrap()).unwrap();
    jenga_tools::projects::restore_project(&p).unwrap();
    assert_eq!(Document::open(&root).unwrap().root_uuid(), d.root_uuid());
    assert!(root.with_extension("kicad_pro").is_file());
    assert!(jenga_tools::projects::restore_project(&p).is_err());
}
