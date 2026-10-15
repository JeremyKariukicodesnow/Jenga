//! Native KiCad page metadata and a shared default worksheet renderer.
use crate::{
    document::Document,
    scene::{BODY, Scene},
    sexpr::Node,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct TitleBlock {
    pub title: String,
    pub date: String,
    pub revision: String,
    pub company: String,
    pub comments: Vec<String>,
}
impl TitleBlock {
    pub fn read(doc: &Document) -> Self {
        let block = doc.root.child("title_block");
        let field = |name| {
            block
                .and_then(|b| b.child(name))
                .map(|n| n.value(1))
                .unwrap_or("")
                .to_owned()
        };
        let mut comments = vec![String::new(); 9];
        if let Some(block) = block {
            for n in block.children("comment") {
                if let Ok(i @ 1..=9) = n.value(1).parse::<usize>() {
                    comments[i - 1] = n.value(2).into();
                }
            }
        }
        Self {
            title: field("title"),
            date: field("date"),
            revision: field("rev"),
            company: field("company"),
            comments,
        }
    }
    pub fn write(&self, doc: &mut Document) {
        let mut block = doc
            .root
            .child("title_block")
            .cloned()
            .unwrap_or_else(|| Node::list("title_block", vec![]));
        for (key, value) in [
            ("title", &self.title),
            ("date", &self.date),
            ("rev", &self.revision),
            ("company", &self.company),
        ] {
            block.set_child(Node::list(key, vec![Node::string(value)]));
        }
        block.items_mut().retain(|n| n.name() != "comment");
        for (i, value) in self.comments.iter().take(9).enumerate() {
            if !value.is_empty() {
                block.items_mut().push(Node::list(
                    "comment",
                    vec![Node::atom(i + 1), Node::string(value)],
                ));
            }
        }
        doc.root.set_child(block);
        doc.dirty = true;
    }
}
/// Draw the default worksheet. Native metadata remains in the schematic, not a Jenga-only format.
pub fn worksheet(doc: &Document, scene: &mut Scene) {
    let (w, h) = crate::export::paper_size(doc);
    let (l, t, r, b) = (10., 10., w - 10., h - 10.);
    let mut line = |a, b| scene.path(vec![a, b], false, None, BODY, 0.15, None);
    for inset in [0., 1.5] {
        for (a, z) in [
            ((l + inset, t + inset), (r - inset, t + inset)),
            ((r - inset, t + inset), (r - inset, b - inset)),
            ((r - inset, b - inset), (l + inset, b - inset)),
            ((l + inset, b - inset), (l + inset, t + inset)),
        ] {
            line(a, z);
        }
    }
    let cols = ((r - l) / 50.).ceil() as usize;
    let rows = ((b - t) / 50.).ceil() as usize;
    for i in 1..cols {
        let x = l + (r - l) * i as f64 / cols as f64;
        line((x, t), (x, t + 1.5));
        line((x, b - 1.5), (x, b));
    }
    for i in 1..rows {
        let y = t + (b - t) * i as f64 / rows as f64;
        line((l, y), (l + 1.5, y));
        line((r - 1.5, y), (r, y));
    }
    for i in 0..cols {
        let x = l + (r - l) * (i as f64 + 0.5) / cols as f64;
        for y in [t + 0.75, b - 0.75] {
            scene.text((x, y), (i + 1).to_string(), 0.65, 0., BODY, false, None);
        }
    }
    for i in 0..rows {
        let y = t + (b - t) * (i as f64 + 0.5) / rows as f64;
        let name = ((b'A' + i as u8) as char).to_string();
        for x in [l + 0.75, r - 0.75] {
            scene.text((x, y), name.clone(), 0.65, 0., BODY, false, None);
        }
    }
    let meta = TitleBlock::read(doc);
    let right = r - 1.5;
    let bottom = b - 1.5;
    let left = (right - 110.).max(l + 1.5);
    let used = meta
        .comments
        .iter()
        .rposition(|s| !s.is_empty())
        .map(|i| i + 1)
        .unwrap_or(0);
    let top = bottom - 26. - used as f64 * 3.;
    scene.path(
        vec![(left, top), (right, top), (right, bottom), (left, bottom)],
        true,
        None,
        BODY,
        0.15,
        None,
    );
    let mut y = top + 2.;
    for value in meta.comments.iter().take(used).rev() {
        scene.text((left + 1., y), value.clone(), 1., 0., BODY, true, None);
        y += 3.;
    }
    for (label, value, height, size) in [
        ("", meta.company, 6., 1.5),
        ("Title: ", meta.title, 5., 1.5),
        (
            "File: ",
            doc.path
                .as_ref()
                .and_then(|p| p.file_name())
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            4.,
            1.,
        ),
    ] {
        scene.text(
            (left + 1., y + height / 2. - 1.),
            format!("{label}{value}"),
            size,
            0.,
            BODY,
            true,
            None,
        );
        y += height;
        scene.path(vec![(left, y), (right, y)], false, None, BODY, 0.15, None);
    }
    let paper = doc.root.child("paper").map(|n| n.value(1)).unwrap_or("A4");
    let page = doc
        .root
        .child("sheet_instances")
        .and_then(|n| n.child("path"))
        .and_then(|n| n.child("page"))
        .map(|n| n.value(1))
        .unwrap_or("1");
    let total = doc
        .page_context
        .map(|(_, total)| total)
        .unwrap_or(1 + doc.root.children("sheet").count());
    let page = doc
        .page_context
        .map(|(page, _)| page.to_string())
        .unwrap_or_else(|| page.to_owned());
    scene.text(
        (left + 1., y + 1.6),
        format!("Size: {paper}    Date: {}", meta.date),
        1.,
        0.,
        BODY,
        true,
        None,
    );
    scene.text(
        (right - 24., y + 1.6),
        format!("Rev: {}", meta.revision),
        1.,
        0.,
        BODY,
        true,
        None,
    );
    scene.path(
        vec![(left, y + 4.), (right, y + 4.)],
        false,
        None,
        BODY,
        0.15,
        None,
    );
    scene.path(
        vec![(right - 25., y), (right - 25., bottom)],
        false,
        None,
        BODY,
        0.15,
        None,
    );
    scene.text(
        (left + 1., bottom - 1.5),
        "Jenga • KiCad schematic".into(),
        1.,
        0.,
        BODY,
        true,
        None,
    );
    scene.text(
        (right - 24., bottom - 1.5),
        format!("Sheet: {page}/{total}"),
        1.,
        0.,
        BODY,
        true,
        None,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn metadata_roundtrip_preserves_unknown_fields() {
        let mut d = Document::new();
        d.root.set_child(Node::list(
            "title_block",
            vec![Node::list("future", vec![Node::string("keep")])],
        ));
        let t = TitleBlock {
            title: "音声 amplifier".into(),
            company: "Jenga".into(),
            comments: vec!["A comment".into()],
            ..Default::default()
        };
        t.write(&mut d);
        let out = TitleBlock::read(&d);
        assert_eq!(out.title, t.title);
        assert_eq!(out.comments[0], "A comment");
        assert!(
            d.root
                .child("title_block")
                .unwrap()
                .child("future")
                .is_some()
        );
        let scene = Scene::page_document(&d);
        assert!(scene.draws.len() > 100);
    }
}
