//! Minimal standards-based XLSX output with typed text cells (never formulas).
use crate::document::{Document, atomic_write, property};
use anyhow::{Result, ensure};
use std::{fmt::Write as _, path::Path};
fn xml(s: &str) -> String {
    s.chars()
        .filter(|c| *c == '\t' || *c == '\n' || *c == '\r' || *c >= ' ')
        .collect::<String>()
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
pub fn bom(doc: &Document, path: &Path) -> Result<()> {
    let mut rows = vec![vec![
        "Reference".to_owned(),
        "Value".into(),
        "Footprint".into(),
        "Datasheet".into(),
        "Description".into(),
    ]];
    let mut seen = std::collections::HashSet::new();
    for n in doc.root.children("symbol") {
        if n.child("in_bom").is_some_and(|v| v.value(1) == "no")
            || !seen.insert(property(n, "Reference"))
        {
            continue;
        }
        rows.push(
            [
                "Reference",
                "Value",
                "Footprint",
                "Datasheet",
                "Description",
            ]
            .iter()
            .map(|key| property(n, key).to_owned())
            .collect(),
        );
    }
    let count = rows.len();
    ensure!(count <= 1_048_576, "BOM exceeds Excel row limit");
    let mut sheet = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><sheetViews><sheetView workbookViewId=\"0\"><pane ySplit=\"1\" topLeftCell=\"A2\" activePane=\"bottomLeft\" state=\"frozen\"/></sheetView></sheetViews><cols><col min=\"1\" max=\"1\" width=\"16\" customWidth=\"1\"/><col min=\"2\" max=\"2\" width=\"24\" customWidth=\"1\"/><col min=\"3\" max=\"5\" width=\"45\" customWidth=\"1\"/></cols><sheetData>",
    );
    for (r, row) in rows.iter().enumerate() {
        write!(sheet, "<row r=\"{}\">", r + 1)?;
        for (c, value) in row.iter().enumerate() {
            write!(
                sheet,
                "<c r=\"{}{}\" t=\"inlineStr\" s=\"{}\"><is><t xml:space=\"preserve\">{}</t></is></c>",
                (b'A' + c as u8) as char,
                r + 1,
                if r == 0 { 1 } else { 0 },
                xml(value)
            )?;
        }
        sheet.push_str("</row>");
    }
    write!(
        sheet,
        "</sheetData><autoFilter ref=\"A1:E{count}\"/></worksheet>"
    )?;
    let files = [
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/><Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#,
        ),
        (
            "xl/workbook.xml",
            r#"<?xml version="1.0"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Bill of materials" sheetId="1" r:id="rId1"/></sheets></workbook>"#,
        ),
        (
            "xl/_rels/workbook.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#,
        ),
        (
            "xl/styles.xml",
            r#"<?xml version="1.0"?><styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><fonts count="2"><font><sz val="11"/><name val="Calibri"/></font><font><b/><sz val="11"/><name val="Calibri"/></font></fonts><fills count="2"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill></fills><borders count="1"><border/></borders><cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs><cellXfs count="2"><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/><xf numFmtId="0" fontId="1" fillId="0" borderId="0" xfId="0" applyFont="1"/></cellXfs><cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles></styleSheet>"#,
        ),
        ("xl/worksheets/sheet1.xml", sheet.as_str()),
    ];
    atomic_write(path, &zip(&files)?, true)
}
fn zip(files: &[(&str, &str)]) -> Result<Vec<u8>> {
    let mut out = vec![];
    let mut directory = vec![];
    for (name, source) in files {
        let bytes = source.as_bytes();
        ensure!(bytes.len() < u32::MAX as usize, "Workbook entry too large");
        let crc = bytes.iter().fold(!0_u32, |crc, b| {
            let mut c = crc ^ *b as u32;
            for _ in 0..8 {
                c = (c >> 1) ^ if c & 1 != 0 { 0xedb88320 } else { 0 };
            }
            c
        }) ^ !0;
        let offset = out.len() as u32;
        u32le(&mut out, 0x04034b50);
        for v in [20, 0, 0, 0, 33] {
            u16le(&mut out, v);
        }
        for v in [crc, bytes.len() as u32, bytes.len() as u32] {
            u32le(&mut out, v);
        }
        u16le(&mut out, name.len() as u16);
        u16le(&mut out, 0);
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(bytes);
        u32le(&mut directory, 0x02014b50);
        for v in [20, 20, 0, 0, 0, 33] {
            u16le(&mut directory, v);
        }
        for v in [crc, bytes.len() as u32, bytes.len() as u32] {
            u32le(&mut directory, v);
        }
        for v in [name.len() as u16, 0, 0, 0, 0] {
            u16le(&mut directory, v);
        }
        u32le(&mut directory, 0);
        u32le(&mut directory, offset);
        directory.extend_from_slice(name.as_bytes());
    }
    let offset = out.len() as u32;
    let size = directory.len() as u32;
    out.extend(directory);
    u32le(&mut out, 0x06054b50);
    for v in [0, 0, files.len() as u16, files.len() as u16] {
        u16le(&mut out, v);
    }
    u32le(&mut out, size);
    u32le(&mut out, offset);
    u16le(&mut out, 0);
    Ok(out)
}
fn u16le(out: &mut Vec<u8>, n: u16) {
    out.extend_from_slice(&n.to_le_bytes());
}
fn u32le(out: &mut Vec<u8>, n: u32) {
    out.extend_from_slice(&n.to_le_bytes());
}
