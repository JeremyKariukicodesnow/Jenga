//! Structured native KiCad symbol authoring, shared by UI and external tools.
use anyhow::{Context, Result, ensure};
use jenga_core::sexpr::{Node, parse};
use serde_json::Value;
pub fn personal_folder() -> Result<std::path::PathBuf> {
    Ok(
        std::path::PathBuf::from(std::env::var_os("APPDATA").context("APPDATA unavailable")?)
            .join("Jenga"),
    )
}
pub fn symbol(p: &Value) -> Result<Node> {
    if let Some(source) = p.get("native_source").and_then(Value::as_str) {
        ensure!(
            source.len() <= 16 * 1024 * 1024,
            "Symbol source exceeds 16 MiB"
        );
        let mut symbol = parse(source)?;
        ensure!(
            symbol.name() == "symbol" && symbol.value(1) == crate::engine::string(p, "name")?,
            "Native source must be one symbol matching name"
        );
        ensure!(
            symbol
                .children("symbol")
                .any(|s| s.children("pin").next().is_some()),
            "Symbol requires pins"
        );
        let graphics = shape_nodes(p)?;
        if !graphics.is_empty() {
            let graphics_name = format!("{}_0_1", symbol.value(1));
            symbol.items_mut().push(Node::list(
                "symbol",
                std::iter::once(Node::string(graphics_name))
                    .chain(graphics)
                    .collect(),
            ));
        }
        for (key, param) in [
            ("Description", "description"),
            ("Footprint", "footprint"),
            ("Reference", "prefix"),
            ("Value", "value"),
        ] {
            if let Some(value) = p.get(param).and_then(Value::as_str)
                && let Some(field) = symbol
                    .items_mut()
                    .iter_mut()
                    .find(|n| n.name() == "property" && n.value(1) == key)
            {
                field.items_mut()[2] = Node::string(value);
            }
        }
        if p.get("replace_pins") == Some(&Value::Bool(true)) {
            let mut structured = p.clone();
            structured.as_object_mut().unwrap().remove("native_source");
            structured["draw_body"] = serde_json::json!(false);
            structured["shapes"] = serde_json::json!([]);
            structured["graphics"] = serde_json::json!("");
            let mut generated = self::symbol(&structured)?;
            // Retain native pin details unless the caller explicitly changes them.
            for child in generated
                .items_mut()
                .iter_mut()
                .filter(|n| n.name() == "symbol")
            {
                let suffix = child
                    .value(1)
                    .rsplit('_')
                    .take(2)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect::<Vec<_>>()
                    .join("_");
                if let Some(old_unit) = symbol
                    .children("symbol")
                    .find(|n| n.value(1).ends_with(&format!("_{suffix}")))
                {
                    for pin in child.items_mut().iter_mut().filter(|n| n.name() == "pin") {
                        let number = pin.child("number").unwrap().value(1).to_owned();
                        if let Some(old) = old_unit.children("pin").find(|n| {
                            n.child("number").map(|x| x.value(1)) == Some(number.as_str())
                        }) {
                            let supplied = p["pins"].as_array().unwrap().iter().find(|x| {
                                x["number"].as_str() == Some(number.as_str())
                                    && x.get("unit")
                                        .and_then(Value::as_u64)
                                        .unwrap_or(1)
                                        .to_string()
                                        == suffix.split('_').next().unwrap()
                            });
                            for key in ["name", "number"] {
                                if let Some(effects) =
                                    old.child(key).and_then(|n| n.child("effects"))
                                {
                                    let field = pin
                                        .items_mut()
                                        .iter_mut()
                                        .find(|n| n.name() == key)
                                        .unwrap();
                                    field.items_mut().retain(|n| n.name() != "effects");
                                    field.items_mut().push(effects.clone());
                                }
                            }
                            for node in old.items().iter().skip(3) {
                                let key = node.name();
                                if ["at", "name", "number"].contains(&key) {
                                    continue;
                                }
                                if key == "length"
                                    && supplied.is_some_and(|v| v.get("length_mm").is_some())
                                {
                                    continue;
                                }
                                if node.text() == "hide"
                                    && supplied.is_some_and(|v| v.get("hidden").is_some())
                                {
                                    continue;
                                }
                                if !key.is_empty() {
                                    pin.items_mut().retain(|n| n.name() != key);
                                }
                                if !pin.items().contains(node) {
                                    pin.items_mut().push(node.clone());
                                }
                            }
                        }
                    }
                }
            }
            for child in symbol
                .items_mut()
                .iter_mut()
                .filter(|n| n.name() == "symbol")
            {
                child.items_mut().retain(|n| n.name() != "pin");
            }
            for child in generated
                .children("symbol")
                .filter(|n| n.children("pin").next().is_some())
            {
                if let Some(target) = symbol
                    .items_mut()
                    .iter_mut()
                    .find(|n| n.name() == "symbol" && n.value(1) == child.value(1))
                {
                    target.items_mut().extend(child.children("pin").cloned());
                } else {
                    symbol.items_mut().push(child.clone());
                }
            }
        }
        normalize_units(&mut symbol);
        return Ok(symbol);
    }
    let name = crate::engine::string(p, "name")?;
    crate::projects::valid_name(name)?;
    let prefix = p.get("prefix").and_then(Value::as_str).unwrap_or("U");
    ensure!(
        !prefix.is_empty() && prefix.chars().all(|c| c.is_ascii_alphabetic()),
        "Prefix must contain letters"
    );
    let pins = p
        .get("pins")
        .and_then(Value::as_array)
        .context("pins array required")?;
    ensure!(!pins.is_empty() && pins.len() <= 1000, "Use 1–1000 pins");
    let w = p.get("width_mm").and_then(Value::as_f64).unwrap_or(15.24);
    let h = p.get("height_mm").and_then(Value::as_f64).unwrap_or(20.32);
    ensure!(
        w.is_finite() && h.is_finite() && (2.54..=500.).contains(&w) && (2.54..=500.).contains(&h),
        "Invalid body dimensions"
    );
    let field = |key: &str, value: &str, y: f64, hide: bool| {
        parse(&format!(
            "(property {} {} (at 0 {y} 0) (effects (font (size 1.27 1.27)) {}))",
            serde_json::json!(key),
            serde_json::json!(value),
            if hide { "(hide yes)" } else { "" }
        ))
    };
    let mut s = Node::list(
        "symbol",
        vec![
            Node::string(name),
            Node::list("in_bom", vec![Node::atom("yes")]),
            Node::list("on_board", vec![Node::atom("yes")]),
            field("Reference", prefix, h / 2. + 2.54, false)?,
            field(
                "Value",
                p.get("value")
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
                    .unwrap_or(name),
                -h / 2. - 2.54,
                false,
            )?,
            field(
                "Footprint",
                p.get("footprint").and_then(Value::as_str).unwrap_or(""),
                0.,
                true,
            )?,
            field(
                "Description",
                p.get("description").and_then(Value::as_str).unwrap_or(""),
                0.,
                true,
            )?,
        ],
    );
    if p.get("draw_body").and_then(Value::as_bool).unwrap_or(true) {
        s.items_mut().push(parse(&format!("(symbol {} (rectangle (start {} {}) (end {} {}) (stroke (width 0.254) (type default)) (fill (type background))))",serde_json::json!(format!("{name}_0_1")),-w/2.,h/2.,w/2.,-h/2.))?);
    }
    let mut units = std::collections::BTreeMap::<u64, Vec<Node>>::new();
    let mut numbers = std::collections::HashSet::new();
    for pin in pins {
        let number = crate::engine::string(pin, "number")?;
        let unit = pin.get("unit").and_then(Value::as_u64).unwrap_or(1);
        ensure!(
            unit <= 100 && !number.is_empty() && numbers.insert((unit, number.to_owned())),
            "Pin numbers must be unique within a unit; use units 0 (common) to 100"
        );
        let kind = pin.get("kind").and_then(Value::as_str).unwrap_or("passive");
        ensure!(
            [
                "input",
                "output",
                "bidirectional",
                "tri_state",
                "passive",
                "free",
                "unspecified",
                "power_in",
                "power_out",
                "open_collector",
                "open_emitter",
                "no_connect"
            ]
            .contains(&kind),
            "Unknown pin type"
        );
        let at = crate::engine::point(pin, "at")?;
        let angle = pin.get("angle").and_then(Value::as_i64).unwrap_or(0);
        ensure!(angle % 90 == 0, "Use pin angles in multiples of 90");
        let length = pin.get("length_mm").and_then(Value::as_f64).unwrap_or(5.08);
        ensure!(
            length.is_finite() && (0.0..=100.).contains(&length),
            "Invalid pin length"
        );
        let shape = pin.get("shape").and_then(Value::as_str).unwrap_or("line");
        ensure!(
            [
                "line",
                "inverted",
                "clock",
                "inverted_clock",
                "input_low",
                "clock_low",
                "output_low",
                "edge_clock_high",
                "non_logic"
            ]
            .contains(&shape),
            "Invalid pin graphic shape"
        );
        let mut node = parse(&format!(
            "(pin {kind} {shape} (at {} {} {}) (length {length}) (name {} (effects (font (size 1 1)))) (number {} (effects (font (size 1 1)))))",
            at.0,
            at.1,
            angle.rem_euclid(360),
            serde_json::json!(pin.get("name").and_then(Value::as_str).unwrap_or("")),
            serde_json::json!(number)
        ))?;
        if pin.get("hidden").and_then(Value::as_bool) == Some(true) {
            node.items_mut().push(Node::atom("hide"));
        }
        units.entry(unit).or_default().push(node);
    }
    for (unit, pins) in units {
        s.items_mut().push(Node::list(
            "symbol",
            std::iter::once(Node::string(format!("{name}_{unit}_1")))
                .chain(pins)
                .collect(),
        ));
    }
    if let Some(graphics) = p.get("graphics").and_then(Value::as_str) {
        let mut root = parse(&format!(
            "(symbol {} {graphics})",
            serde_json::json!(format!("{name}_0_1"))
        ))?;
        ensure!(
            root.items()
                .iter()
                .skip(2)
                .all(|n| ["rectangle", "polyline", "circle", "arc", "text"].contains(&n.name())),
            "Unsupported graphics"
        );
        s.items_mut()
            .push(std::mem::replace(&mut root, Node::atom("")));
    }
    let graphics = shape_nodes(p)?;
    if !graphics.is_empty() {
        s.items_mut().push(Node::list(
            "symbol",
            std::iter::once(Node::string(format!("{name}_0_1")))
                .chain(graphics)
                .collect(),
        ));
    }
    normalize_units(&mut s);
    Ok(s)
}

fn normalize_units(symbol: &mut Node) {
    let name = symbol.value(1).to_owned();
    let mut other = Vec::new();
    let mut units = std::collections::BTreeMap::<String, Node>::new();
    for mut child in symbol.children("symbol").cloned() {
        let parts: Vec<_> = child.value(1).rsplit('_').take(2).collect();
        if parts.len() == 2 && parts.iter().all(|p| p.parse::<u64>().is_ok()) {
            let id = format!("{name}_{}_{}", parts[1], parts[0]);
            child.items_mut()[1] = Node::string(&id);
            if let Some(existing) = units.get_mut(&id) {
                existing
                    .items_mut()
                    .extend(child.items().iter().skip(2).cloned());
            } else {
                units.insert(id, child);
            }
        } else {
            other.push(child);
        }
    }
    symbol.items_mut().retain(|n| n.name() != "symbol");
    symbol.items_mut().extend(units.into_values());
    symbol.items_mut().extend(other);
}

pub fn shape_nodes(p: &Value) -> Result<Vec<Node>> {
    let shapes = p
        .get("shapes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    ensure!(shapes.len() <= 1000, "Use at most 1000 graphic shapes");
    let mut out = vec![];
    for s in shapes {
        let kind = crate::engine::string(&s, "kind")?;
        let width = s.get("stroke_mm").and_then(Value::as_f64).unwrap_or(0.254);
        ensure!(
            width.is_finite() && (0.0..=5.).contains(&width),
            "Invalid graphic stroke"
        );
        let fill = s.get("fill").and_then(Value::as_str).unwrap_or("none");
        ensure!(
            ["none", "background", "outline"].contains(&fill),
            "Invalid graphic fill"
        );
        let appearance = format!("(stroke (width {width}) (type default)) (fill (type {fill}))");
        let source = match kind {
            "rectangle" => {
                let a = crate::engine::point(&s, "start")?;
                let b = crate::engine::point(&s, "end")?;
                format!(
                    "(rectangle (start {} {}) (end {} {}) {appearance})",
                    a.0, a.1, b.0, b.1
                )
            }
            "circle" => {
                let a = crate::engine::point(&s, "center")?;
                let r = s
                    .get("radius_mm")
                    .and_then(Value::as_f64)
                    .context("Circle needs radius_mm")?;
                ensure!(
                    r.is_finite() && (0.01..=1000.).contains(&r),
                    "Invalid circle radius"
                );
                format!(
                    "(circle (center {} {}) (radius {r}) {appearance})",
                    a.0, a.1
                )
            }
            "arc" => {
                let a = crate::engine::point(&s, "start")?;
                let b = crate::engine::point(&s, "mid")?;
                let c = crate::engine::point(&s, "end")?;
                ensure!(
                    ((b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)).abs() > 1e-6,
                    "Arc points must not be collinear"
                );
                format!(
                    "(arc (start {} {}) (mid {} {}) (end {} {}) {appearance})",
                    a.0, a.1, b.0, b.1, c.0, c.1
                )
            }
            "line" | "polyline" | "polygon" => {
                let ps = s
                    .get("points")
                    .and_then(Value::as_array)
                    .context("Shape needs points")?;
                ensure!(
                    ps.len() >= 2 && ps.len() <= 1000,
                    "Invalid polyline point count"
                );
                let mut ps = ps
                    .iter()
                    .map(|p| crate::engine::point(&serde_json::json!({"at":p}), "at"))
                    .collect::<Result<Vec<_>>>()?;
                if kind == "polygon" && ps.first() != ps.last() {
                    ps.push(ps[0]);
                }
                format!(
                    "(polyline (pts {}) {appearance})",
                    ps.iter()
                        .map(|p| format!("(xy {} {})", p.0, p.1))
                        .collect::<Vec<_>>()
                        .join(" ")
                )
            }
            "text" => {
                let a = crate::engine::point(&s, "at")?;
                let text = crate::engine::string(&s, "text")?;
                let size = s.get("size_mm").and_then(Value::as_f64).unwrap_or(1.27);
                ensure!(
                    size.is_finite() && (0.3..=20.).contains(&size),
                    "Invalid label size"
                );
                format!(
                    "(text {} (at {} {} 0) (effects (font (size {size} {size}))))",
                    serde_json::json!(text),
                    a.0,
                    a.1
                )
            }
            _ => anyhow::bail!("Unknown graphic kind {kind}"),
        };
        out.push(parse(&source)?);
    }
    Ok(out)
}
