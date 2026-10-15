use jenga_core::{document::Document, sexpr::parse};
use jenga_tools::{
    engine::{Engine, edit},
    journal, setup,
};
use serde_json::json;

#[test]
fn native_child_pages_and_selected_pdf_export() {
    let temp = tempfile::tempdir().unwrap();
    let mut e = Engine::new(temp.path().into());
    let root = e
        .request(
            "create_project",
            &json!({"name":"House","folder":temp.path()}),
        )
        .unwrap();
    assert_eq!(
        e.request("project_info", &json!({})).unwrap()["pages"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let revision = root["revision"].as_str().unwrap();
    let page = e
        .request(
            "create_page",
            &json!({"name":"Power","hierarchical":true,"expected_revision":revision}),
        )
        .unwrap();
    assert!(std::path::Path::new(page["file"].as_str().unwrap()).is_file());
    let info = e.request("project_info", &json!({})).unwrap();
    assert_eq!(info["pages"].as_array().unwrap().len(), 2);
    let pdf = temp.path().join("all.pdf");
    e.request("export_pages", &json!({"path":pdf})).unwrap();
    let text = String::from_utf8_lossy(&std::fs::read(&pdf).unwrap()).into_owned();
    assert!(text.contains("/Count 2"));
    let separate = temp.path().join("separate");
    e.request(
        "export_pages",
        &json!({"path":separate,"format":"pdf","separate":true}),
    )
    .unwrap();
    assert_eq!(std::fs::read_dir(&separate).unwrap().count(), 2);
    for f in std::fs::read_dir(&separate).unwrap() {
        assert!(
            String::from_utf8_lossy(&std::fs::read(f.unwrap().path()).unwrap())
                .contains("/Count 1")
        );
    }
    let one = temp.path().join("one.pdf");
    e.request("export_pages", &json!({"path":one,"pages":["Power"]}))
        .unwrap();
    assert!(String::from_utf8_lossy(&std::fs::read(one).unwrap()).contains("/Count 1"));
    assert!(
        e.request(
            "export_pages",
            &json!({"path":temp.path().join("bad.pdf"),"pages":["DoesNotExist"]})
        )
        .is_err()
    );
    assert!(
        e.request("create_page", &json!({"name":"../outside"}))
            .is_err()
    );
}
#[test]
fn text_formatting_preserves_identity_and_unknown_effects() {
    let mut d = Document::new();
    let i = d.add_text("text", "Example", 25.4, 25.4);
    let id = d.root.items()[i].child("uuid").unwrap().value(1).to_owned();
    let mut libs = jenga_libraries::Libraries::discover(&[], None);
    edit(&mut d,&mut libs,&json!({"actions":[{"op":"format_text","uuid":id,"size_mm":3.5,"align":"right","angle":180,"upright":true}]})).unwrap();
    let n = &d.root.items()[i];
    assert_eq!(n.child("uuid").unwrap().value(1), id);
    assert_eq!(n.child("at").unwrap().number(3), 0.);
    assert_eq!(
        n.child("effects")
            .unwrap()
            .child("font")
            .unwrap()
            .child("size")
            .unwrap()
            .number(1),
        3.5
    );
    let before = journal::revision(&d);
    assert!(
        edit(
            &mut d,
            &mut libs,
            &json!({"actions":[{"op":"format_text","uuid":id,"size_mm":-1}]})
        )
        .is_err()
    );
    assert_eq!(before, journal::revision(&d));
}
#[test]
fn repair_preserves_assistant_arguments_and_other_servers() {
    let t = tempfile::tempdir().unwrap();
    let exe = t.path().join("jenga-cli.exe");
    std::fs::write(&exe, "").unwrap();
    let folder = t.path().join(".codex");
    std::fs::create_dir(&folder).unwrap();
    let config = folder.join("config.toml");
    std::fs::write(&config,"[mcp_servers.jenga]\ncommand = 'C:/old/release/jenga-cli.exe'\nargs = ['mcp','--workspace','D:/Circuits','--connect','auto']\n[mcp_servers.other]\ncommand = 'other.exe'\n").unwrap();
    let result = setup::repair_connections(t.path(), &exe).unwrap();
    assert_eq!(result["updated_clients"], json!(["Codex"]), "{result}");
    let text = std::fs::read_to_string(&config).unwrap();
    let v: toml::Table = text.parse().unwrap();
    assert_eq!(
        v["mcp_servers"]["jenga"]["args"][2].as_str(),
        Some("D:/Circuits")
    );
    assert_eq!(
        v["mcp_servers"]["other"]["command"].as_str(),
        Some("other.exe")
    );
    assert_eq!(
        setup::repair_connections(t.path(), &exe).unwrap()["updated_clients"],
        json!([])
    );
}
#[test]
fn native_symbol_authoring_validates_physical_pin_identity() {
    let p = json!({"name":"Sensor","prefix":"U","pins":[{"number":"1","name":"VCC","kind":"power_in","at":[-12.7,5.08]},{"number":"2","name":"GND","kind":"power_in","at":[-12.7,-5.08]}]});
    let symbol = jenga_tools::symbol_editor::symbol(&p).unwrap();
    assert_eq!(parse(&symbol.encoded()).unwrap(), symbol);
    let mut bad = p.clone();
    bad["pins"][1]["number"] = json!("1");
    assert!(jenga_tools::symbol_editor::symbol(&bad).is_err());
}
#[test]
fn forbidden_connection_rolls_back_entire_edit() {
    let p = json!({"name":"Test","pins":[{"number":"1","name":"P","at":[-12.7,0]}]});
    let def = jenga_tools::symbol_editor::symbol(&p).unwrap();
    let mut d = Document::new();
    d.add_symbol("Test:Test", &def, 40., 40., 1);
    d.add_symbol("Test:Test", &def, 80., 40., 1);
    let scene = jenga_core::scene::Scene::from_document(&d);
    let a = &scene.pins[0];
    let b = &scene.pins[1];
    let before = journal::revision(&d);
    let request = json!({"actions":[{"op":"wire","points":[a.at,b.at]}],"forbidden_connections":[{"from":{"reference":a.reference,"number":"1"},"to":{"reference":b.reference,"number":"1"}}]});
    let mut libs = jenga_libraries::Libraries::discover(&[], None);
    assert!(edit(&mut d, &mut libs, &request).is_err());
    assert_eq!(journal::revision(&d), before);
}

#[test]
fn mixed_symbol_shapes_and_imported_pin_details_survive_editing() {
    let p = json!({"name":"Controller","draw_body":false,"pins":[{"number":"1","name":"Enable","kind":"input","shape":"inverted","at":[-12.7,0],"hidden":true,"length_mm":2.54}],"shapes":[{"kind":"rectangle","start":[-5,5],"end":[5,-5]},{"kind":"circle","center":[0,0],"radius_mm":1.27},{"kind":"arc","start":[0,0],"mid":[1,1],"end":[2,0]},{"kind":"polygon","points":[[0,0],[2,0],[1,2]]},{"kind":"text","at":[0,8],"text":"Gate driver","size_mm":2.54}]});
    let original = jenga_tools::symbol_editor::symbol(&p).unwrap();
    assert_eq!(original.children("symbol").count(), 2);
    let pin = original
        .children("symbol")
        .flat_map(|u| u.children("pin"))
        .next()
        .unwrap();
    assert!(pin.items().iter().any(|n| n.text() == "hide"));
    let mut edit = json!({"name":"Controller","native_source":original.encoded(),"replace_pins":true,"pins":[{"number":"1","name":"New name","kind":"power_in","at":[-12.7,0]}]});
    let modified = jenga_tools::symbol_editor::symbol(&edit).unwrap();
    let pin = modified
        .children("symbol")
        .flat_map(|u| u.children("pin"))
        .next()
        .unwrap();
    assert_eq!(pin.value(1), "power_in");
    assert_eq!(pin.child("length").unwrap().number(1), 2.54);
    assert!(pin.items().iter().any(|n| n.text() == "hide"));
    assert_eq!(pin.child("name").unwrap().value(1), "New name");
    assert!(
        modified
            .children("symbol")
            .flat_map(|u| u.children("text"))
            .any(|t| t.value(1) == "Gate driver")
    );
    edit["pins"][0]["hidden"] = json!(false);
    let modified = jenga_tools::symbol_editor::symbol(&edit).unwrap();
    assert!(
        !modified
            .children("symbol")
            .flat_map(|u| u.children("pin"))
            .next()
            .unwrap()
            .items()
            .iter()
            .any(|n| n.text() == "hide")
    );
    let mut bad = p;
    bad["shapes"][2]["mid"] = json!([1, 0]);
    assert!(jenga_tools::symbol_editor::symbol(&bad).is_err());
}
#[test]
fn antigravity_and_generic_skill_installation_succeed() {
    let dir = tempfile::tempdir().unwrap();
    for client in ["Antigravity", "Other"] {
        let path = setup::install_skill(client, dir.path()).unwrap();
        assert!(
            std::fs::read_to_string(path)
                .unwrap()
                .contains("Full explicit wiring")
        );
    }
}
