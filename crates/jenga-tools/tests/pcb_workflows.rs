use jenga_core::{
    mesh::Mesh,
    pcb::{self, Board},
    sexpr::Node,
};
use jenga_tools::engine::Engine;
use serde_json::json;
fn fixture() -> (tempfile::TempDir, Engine) {
    let dir = tempfile::tempdir().unwrap();
    let mut e = Engine::new(dir.path().into());
    e.board = Some(Board::new(80., 60.).unwrap());
    e.board.as_mut().unwrap().path = Some(dir.path().join("Board.kicad_pcb"));
    e.request("footprint_save",&json!({"name":"Pad","library":"Test","pads":[{"number":"1","kind":"thru_hole","shape":"circle","at":[0,0],"size":[2,2],"drill_mm":0.8}]})).unwrap();
    e.request("pcb_edit",&json!({"actions":[{"op":"add_footprint","library_id":"Test:Pad","reference":"J1","at":[20,25]},{"op":"add_footprint","library_id":"Test:Pad","reference":"J2","at":[60,25]},{"op":"pad_net","reference":"J1","number":"1","net":"SIGNAL"},{"op":"pad_net","reference":"J2","number":"1","net":"SIGNAL"}]})).unwrap();
    (dir, e)
}
#[test]
fn native_preserves_unknown_data_and_external_edits() {
    let (dir, mut e) = fixture();
    let b = e.board.as_mut().unwrap();
    b.root.items_mut().push(Node::list(
        "future_native_field",
        vec![Node::string("keep")],
    ));
    let p = dir.path().join("Board.kicad_pcb");
    b.save(&p).unwrap();
    let mut reopened = Board::open(&p).unwrap();
    assert!(reopened.root.child("future_native_field").is_some());
    std::fs::write(&p, format!("{}\n", reopened.root.document_text())).unwrap();
    reopened.dirty = true;
    assert!(reopened.save(&p).is_err());
}
#[test]
fn routing_preview_commit_is_atomic_and_idempotent() {
    let (_dir, mut e) = fixture();
    let pair = json!({"from":{"reference":"J1","number":"1"},"to":{"reference":"J2","number":"1"}});
    let base = e.board.as_ref().unwrap().revision();
    let v = e
        .request("pcb_connect", &json!({"connections":[pair],"dry_run":true}))
        .unwrap();
    assert_eq!(e.board.as_ref().unwrap().revision(), base);
    assert_eq!(v["can_commit"], true, "{v}");
    let p = json!({"preview_id":v["preview_id"],"operation_id":uuid::Uuid::new_v4().to_string()});
    let first = e.request("pcb_commit", &p).unwrap();
    assert_eq!(e.request("pcb_commit", &p).unwrap(), first);
    let check = e
        .request("pcb_verify", &json!({"expected_connections":[pair]}))
        .unwrap();
    assert_eq!(check["passed"], true, "{check}");
    assert!(
        e.request("pcb_commit", &json!({"preview_id":v["preview_id"]}))
            .is_err()
    );
}
#[test]
fn conflicting_batch_keeps_original_board() {
    let (_dir, mut e) = fixture();
    let rev = e.board.as_ref().unwrap().revision();
    assert!(e.request("pcb_edit",&json!({"actions":[{"op":"text","text":"will rollback","at":[30,30]},{"op":"move","reference":"not-there","at":[0,0]}]})).is_err());
    assert_eq!(rev, e.board.as_ref().unwrap().revision());
    let bad = e
        .request(
            "pcb_preview",
            &json!({"actions":[{"op":"move","reference":"not-there","at":[0,0]}]}),
        )
        .unwrap();
    assert_eq!(bad["can_commit"], false);
}
#[test]
fn floating_pad_is_routing_obstacle() {
    let (_dir, mut e) = fixture();
    e.request("pcb_edit",&json!({"actions":[{"op":"add_footprint","library_id":"Test:Pad","reference":"J3","at":[40,25]}]})).unwrap();
    e.request("pcb_connect",&json!({"connections":[{"from":{"reference":"J1","number":"1"},"to":{"reference":"J2","number":"1"}}]})).unwrap();
    let b = e.board.as_ref().unwrap();
    assert!(b.root.children("segment").count() > 1);
    assert_eq!(e.request("pcb_verify", &json!({})).unwrap()["passed"], true);
}
#[test]
fn footprint_revision_and_safe_replacement_preserve_trace() {
    let (_dir, mut e) = fixture();
    e.request("pcb_connect",&json!({"connections":[{"from":{"reference":"J1","number":"1"},"to":{"reference":"J2","number":"1"}}]})).unwrap();
    e.request(
        "footprint_save",
        &json!({"name":"New","library":"Test","pads":[{"number":"7","at":[2,0],"size":[2,2]}]}),
    )
    .unwrap();
    let base = e.board.as_ref().unwrap().revision();
    assert!(e.request("pcb_edit",&json!({"actions":[{"op":"replace_footprint","reference":"J2","library_id":"Test:New"}]})).is_err());
    assert_eq!(e.board.as_ref().unwrap().revision(), base);
    e.request("pcb_edit",&json!({"actions":[{"op":"replace_footprint","reference":"J2","library_id":"Test:New","pad_map":{"1":"7"}}]})).unwrap();
    assert_eq!(
        e.request(
            "pcb_trace",
            &json!({"from":{"reference":"J1","number":"1"},"to":{"reference":"J2","number":"7"}})
        )
        .unwrap()["connected"],
        true
    );
}
#[test]
fn pcb_only_project_discovery_and_open() {
    let dir = tempfile::tempdir().unwrap();
    let mut e = Engine::new(dir.path().into());
    let v = e
        .request(
            "pcb_create_project",
            &json!({"name":"OnlyBoard","folder":dir.path()}),
        )
        .unwrap();
    let path = std::path::PathBuf::from(v["file"].as_str().unwrap());
    let project =
        jenga_tools::project_workspace::discover(&path.with_extension("kicad_pro")).unwrap();
    assert!(project.pages.is_empty());
    assert_eq!(project.boards.len(), 1);
    let mut e = Engine::new(dir.path().into());
    assert_eq!(
        e.request("open", &json!({"path":path.with_extension("kicad_pro")}))
            .unwrap()["kind"],
        "pcb"
    );
}
#[test]
fn mesh_units_and_stl_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let mut m = Mesh::default();
    m.box_solid([0., 0., 0.], [10., 20., 3.], [100, 100, 100], None);
    let p = dir.path().join("model.stl");
    m.export_stl(&p).unwrap();
    let read = Mesh::read(&p).unwrap();
    assert_eq!(read.triangles.len(), 12);
    assert_eq!(read.bounds(), ([0., 0., 0.], [10., 20., 3.]));
}
#[test]
fn native_backend_drc_and_3d_if_available() {
    if jenga_tools::pcb_backend::executable().is_err() {
        return;
    }
    let (dir, mut e) = fixture();
    e.request("pcb_connect",&json!({"connections":[{"from":{"reference":"J1","number":"1"},"to":{"reference":"J2","number":"1"}}]})).unwrap();
    let v = e.request("pcb_verify", &json!({"external":true})).unwrap();
    assert_eq!(v["external_kicad_checked"], true, "{v}");
    let p = dir.path().join("board.glb");
    let v = e.request("pcb_3d", &json!({"path":p})).unwrap();
    assert!(v["mesh"]["triangle_count"].as_u64().unwrap() > 12);
    let mesh = jenga_core::mesh::Mesh::read(&p).unwrap();
    let (lo, hi) = mesh.bounds();
    assert!(
        hi[2] - lo[2] < 5.,
        "Native GLB thickness must be Z-up: {lo:?} {hi:?}"
    );
    assert!(hi[1] - lo[1] > 20., "Native GLB board plane must be XY");
    e.request("pcb_fill_zones", &json!({})).unwrap();
    assert_eq!(
        e.request(
            "pcb_trace",
            &json!({"from":{"reference":"J1","number":"1"},"to":{"reference":"J2","number":"1"}})
        )
        .unwrap()["connected"],
        true
    );
    e.request(
        "pcb_edit",
        &json!({"actions":[{"op":"stackup","copper_layers":4}]}),
    )
    .unwrap();
    assert_eq!(
        e.request("pcb_verify", &json!({"external":true})).unwrap()["external_kicad_checked"],
        true
    );
    assert!(
        e.board
            .as_ref()
            .unwrap()
            .root
            .children("footprint")
            .any(|n| pcb::field(n, "Reference") == "J1")
    );
}
#[test]
fn one_call_build_keeps_board_dimensions_out_of_track_options() {
    let (dir, mut e) = fixture();
    e.board = None;
    e.request("new", &json!({})).unwrap();
    e.doc.as_mut().unwrap().path = Some(dir.path().join("Circuit.kicad_sch"));
    e.request("save_symbol",&json!({"name":"Single","library":"Test","scope":"project","prefix":"U","pins":[{"number":"1","name":"P","kind":"passive","at":[-10.16,0],"length_mm":2.54}]})).unwrap();
    e.request("edit",&json!({"actions":[{"op":"add_symbol","library_id":"Test:Single","at":[50.8,50.8],"properties":{"Reference":"U1","Footprint":"Test:Pad"}},{"op":"add_symbol","library_id":"Test:Single","at":[101.6,50.8],"properties":{"Reference":"U2","Footprint":"Test:Pad"}},{"op":"connect_pins","from":{"reference":"U1","number":"1"},"to":{"reference":"U2","number":"1"}}]})).unwrap();
    let preview = e
        .request(
            "build_pcb_from_schematic",
            &json!({"width_mm":80,"height_mm":60}),
        )
        .unwrap();
    assert_eq!(preview["can_commit"], true, "{preview}");
    e.request("pcb_commit", &json!({"preview_id":preview["preview_id"]}))
        .unwrap();
    assert!(
        e.board.as_ref().unwrap().root.children("segment").all(|n| n
            .child("width")
            .unwrap()
            .number(1)
            == 0.25)
    );
}
#[test]
fn default_page_does_not_add_sheet_rectangle() {
    let dir = tempfile::tempdir().unwrap();
    let mut e = Engine::new(dir.path().into());
    e.request(
        "create_project",
        &json!({"name":"Pages","folder":dir.path()}),
    )
    .unwrap();
    e.request("create_page", &json!({"name":"Separate"}))
        .unwrap();
    assert_eq!(e.doc.as_ref().unwrap().root.children("sheet").count(), 0);
}
#[test]
fn undo_restores_rules_and_link_state() {
    let (_dir, mut e) = fixture();
    let base = e.board.as_ref().unwrap().revision();
    e.request("pcb_edit", &json!({"rules":{"clearance":0.4}}))
        .unwrap();
    e.request("pcb_undo", &json!({})).unwrap();
    assert_eq!(base, e.board.as_ref().unwrap().revision());
}
