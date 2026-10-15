use anyhow::Result;
use jenga_core::document::Document;
use serde_json::{Value, json};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
pub fn now() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}
/// Stable content revision, used for conflict detection (not an authentication digest).
pub fn revision(doc: &Document) -> String {
    revision_text(&doc.root.encoded())
}
pub fn revision_text(text: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in text.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}
pub fn path(doc: &Document) -> Option<PathBuf> {
    let base = doc
        .path
        .as_ref()
        .and_then(|p| p.parent())
        .map(|p| p.join(".jenga"))
        .unwrap_or_else(|| {
            std::env::var_os("LOCALAPPDATA")
                .map(PathBuf::from)
                .unwrap_or_else(std::env::temp_dir)
                .join("Jenga")
                .join("unsaved")
        });
    Some(
        base.join("history")
            .join(format!("{}.jsonl", doc.root_uuid())),
    )
}
pub fn record(before: &Document, after: &Document, actor: &str, operation: &str) -> Result<()> {
    if revision(before) == revision(after) && operation != "save" && operation != "rename" {
        return Ok(());
    }
    let Some(path) = path(after) else {
        return Ok(());
    };
    fs::create_dir_all(path.parent().unwrap())?;
    // Move pre-save/Save-As history with the document without discarding the original journal.
    if let Some(previous) = self::path(before)
        && previous != path
        && previous.exists()
        && !path.exists()
    {
        fs::copy(previous, &path)?;
    }
    let value = json!({"timestamp_ms":now(),"actor":actor,"operation":operation,"before_revision":revision(before),"after_revision":revision(after),"file":after.path,"before":before.root.document_text(),"after":after.root.document_text()});
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    serde_json::to_writer(&mut file, &value)?;
    file.write_all(b"\n")?;
    file.sync_data()?;
    Ok(())
}
pub fn read(doc: &Document, limit: usize) -> Result<Vec<Value>> {
    let Some(path) = path(doc) else {
        return Ok(vec![]);
    };
    if !path.exists() {
        return Ok(vec![]);
    }
    let mut entries = Vec::new();
    for line in std::io::BufRead::lines(std::io::BufReader::new(fs::File::open(path)?)) {
        let mut item: Value = serde_json::from_str(&line?)?;
        item.as_object_mut().unwrap().remove("before");
        item.as_object_mut().unwrap().remove("after");
        entries.push(item);
        if entries.len() > limit {
            entries.remove(0);
        }
    }
    Ok(entries)
}
pub fn record_resource(
    doc: &Document,
    resource: &str,
    before: &str,
    after: &str,
    actor: &str,
    operation: &str,
) -> Result<()> {
    let path = path(doc).unwrap();
    fs::create_dir_all(path.parent().unwrap())?;
    let value = json!({"timestamp_ms":now(),"actor":actor,"operation":operation,"resource":resource,"before_revision":revision_text(before),"after_revision":revision_text(after),"before":before,"after":after});
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    serde_json::to_writer(&mut file, &value)?;
    file.write_all(b"\n")?;
    file.sync_data()?;
    Ok(())
}
