//! Durable receipts: a lost response must never cause a second mutation.
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::path::PathBuf;

fn path(id: &str) -> Result<PathBuf> {
    ensure!(
        uuid::Uuid::parse_str(id).is_ok(),
        "operation_id must be a UUID"
    );
    Ok(crate::catalog::directory()
        .join("operations")
        .join(format!("{id}.json")))
}
pub fn status(id: &str) -> Result<Value> {
    let p = path(id)?;
    let mut v: Value = serde_json::from_slice(&std::fs::read(p).context("Unknown operation_id")?)?;
    v.as_object_mut().unwrap().remove("request");
    Ok(v)
}
pub fn admit(id: &str, method: &str, params: &Value) -> Result<Option<Value>> {
    let p = path(id)?;
    std::fs::create_dir_all(p.parent().unwrap())?;
    let request = json!({"method":method,"params":params});
    let receipt = json!({"operation_id":id,"status":"pending","request":request,"started_ms":crate::journal::now(),"pid":std::process::id()});
    use std::io::Write;
    let mut file = tempfile::NamedTempFile::new_in(p.parent().unwrap())?;
    file.write_all(serde_json::to_string(&receipt)?.as_bytes())?;
    file.as_file().sync_all()?;
    match file.persist_noclobber(&p) {
        Ok(_) => Ok(None),
        Err(e) if e.error.kind() == std::io::ErrorKind::AlreadyExists => {
            let prior: Value = serde_json::from_slice(&std::fs::read(&p)?)?;
            ensure!(
                prior["request"] == request,
                "operation_id already belongs to a different request"
            );
            Ok(Some(status(id)?))
        }
        Err(e) => Err(e.error.into()),
    }
}
pub fn begin(id: &str, method: &str, params: &Value) -> Result<Option<Value>> {
    if let Some(receipt) = admit(id, method, params)? {
        if receipt["status"] != "pending" {
            return Ok(Some(receipt));
        }
        if receipt["pid"] != std::process::id() {
            return Ok(Some(receipt));
        }
    }
    // An exclusive execution claim survives process exit. A claimed pending
    // operation cannot replay a partially completed mutation after a crash.
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path(id)?.with_extension("claim"))
    {
        Ok(mut file) => {
            use std::io::Write;
            file.write_all(std::process::id().to_string().as_bytes())?;
            file.sync_all()?;
            Ok(None)
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(Some(status(id)?)),
        Err(e) => Err(e.into()),
    }
}
pub fn reject(id: &str, message: &str) -> Value {
    let _ = finish(id, &Err(anyhow::anyhow!("{message}")));
    json!({"error":format!("Operation {id} failed: {message}"),"operation_id":id,"status":"failed"})
}
pub fn finish(id: &str, result: &Result<Value>) -> Result<Value> {
    let p = path(id)?;
    let mut receipt: Value = serde_json::from_slice(&std::fs::read(&p)?)?;
    receipt["finished_ms"] = json!(crate::journal::now());
    match result {
        Ok(v) => {
            receipt["status"] = json!("completed");
            receipt["result"] = v.clone();
        }
        Err(e) => {
            receipt["status"] = json!("failed");
            receipt["error"] = json!(format!("{e:#}"));
        }
    }
    jenga_core::document::atomic_write(&p, &serde_json::to_vec(&receipt)?, false)?;
    status(id)
}
pub fn mutation(method: &str) -> bool {
    crate::pcb_tools::mutating(method)
        || matches!(
            method,
            "edit"
                | "connect_block"
                | "commit_preview"
                | "paste_objects"
                | "save"
                | "create_project"
                | "create_page"
                | "delete_page"
                | "rename_page"
                | "reorder_pages"
                | "rename"
                | "save_project_as"
                | "delete_project"
                | "save_symbol"
                | "copy_symbol"
                | "manage_symbol"
                | "write_library"
                | "create_library"
                | "import_library"
                | "create_schematic"
                | "restore_project"
        )
}
