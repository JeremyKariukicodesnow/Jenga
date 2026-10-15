//! Durable receipts: a lost response must never cause a second mutation.
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::path::PathBuf;
static WORKERS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
pub struct WorkerSlot;
impl Drop for WorkerSlot {
    fn drop(&mut self) {
        WORKERS.fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
    }
}
#[allow(deprecated)]
pub fn acquire() -> Result<WorkerSlot> {
    WORKERS.fetch_update(std::sync::atomic::Ordering::AcqRel,std::sync::atomic::Ordering::Acquire,|n|(n<2).then_some(n+1)).map_err(|_|anyhow::anyhow!("Two background calculations are already running; inspect operations or cancel a job before starting another"))?;
    Ok(WorkerSlot)
}
thread_local! { static CONTEXT: std::cell::RefCell<Option<(String, std::time::Instant)>> = const { std::cell::RefCell::new(None) }; }
pub fn context(id: &str, timeout_ms: u64) {
    CONTEXT.with(|c| {
        *c.borrow_mut() = Some((
            id.into(),
            std::time::Instant::now()
                + std::time::Duration::from_millis(timeout_ms.clamp(100, 600_000)),
        ))
    });
}
pub fn checkpoint() -> Result<()> {
    CONTEXT.with(|c| {
        if let Some((id, deadline)) = c.borrow().as_ref() {
            ensure!(std::time::Instant::now() < *deadline, "Operation time budget exhausted; no candidate applied. Inspect remaining connections and use a smaller batch.");
            ensure!(!path(id)?.with_extension("cancel").exists(), "Operation cancelled; no candidate applied");
        }
        Ok(())
    })
}
pub fn progress(phase: &str, visited: usize) {
    CONTEXT.with(|c| {
        if let Some((id, _)) = c.borrow().as_ref()
            && let Ok(p) = path(id)
        {
            let _ = jenga_core::document::atomic_write(
                &p.with_extension("progress"),
                json!({"phase":phase,"visited_states":visited,"updated_ms":crate::journal::now()})
                    .to_string()
                    .as_bytes(),
                false,
            );
        }
    });
}
pub fn cancel(id: &str) -> Result<Value> {
    let receipt = status(id)?;
    ensure!(
        receipt["status"] == "pending",
        "Only pending operations can be cancelled"
    );
    jenga_core::document::atomic_write(&path(id)?.with_extension("cancel"), b"cancel", false)?;
    Ok(
        json!({"operation_id":id,"status":"pending","cancel_requested":true,"message":"Cooperative cancellation requested; poll status until the worker acknowledges it."}),
    )
}
pub fn list() -> Result<Value> {
    let dir = crate::catalog::directory().join("operations");
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    files.sort_by_key(|p| std::cmp::Reverse(p.metadata().and_then(|m| m.modified()).ok()));
    let receipts: Vec<_> = files
        .into_iter()
        .take(100)
        .filter_map(|p| status(p.file_stem()?.to_str()?).ok())
        .map(|mut v| {
            v.as_object_mut().unwrap().remove("result");
            v
        })
        .collect();
    Ok(json!({"operations":receipts,"limit":100}))
}

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
    let mut v: Value = serde_json::from_slice(&std::fs::read(&p).context("Unknown operation_id")?)?;
    if v["status"] == "pending" {
        if v["pid"]
            .as_u64()
            .is_some_and(|pid| crate::transport::process_alive(pid as u32) == Some(false))
        {
            v["status"] = json!("interrupted");
            v["message"] = json!(
                "Worker exited. Outcome may be indeterminate; inspect saved revisions/history. This operation will not be replayed."
            );
        }
        if let Ok(bytes) = std::fs::read(p.with_extension("progress"))
            && let Ok(progress) = serde_json::from_slice::<Value>(&bytes)
        {
            v["progress"] = progress;
        }
        v["cancel_requested"] = json!(p.with_extension("cancel").exists());
    }
    v.as_object_mut().unwrap().remove("request");
    Ok(v)
}
pub fn admit(id: &str, method: &str, params: &Value) -> Result<Option<Value>> {
    let p = path(id)?;
    std::fs::create_dir_all(p.parent().unwrap())?;
    let request = json!({"method":method,"params":params});
    let receipt = json!({"operation_id":id,"method":method,"actor":params["actor"],"status":"pending","request":request,"started_ms":crate::journal::now(),"pid":std::process::id()});
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
                | "import_asset"
                | "engineering_review"
                | "import_project"
                | "move_project"
                | "import_document"
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
