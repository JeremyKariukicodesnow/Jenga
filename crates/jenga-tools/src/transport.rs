//! Authenticated loopback JSON channel; mutations are delivered to the UI thread.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
    time::Duration,
};
#[derive(Clone, Serialize, Deserialize)]
pub struct Endpoint {
    pub pid: u32,
    pub address: String,
    pub token: String,
    pub created_ms: u128,
    pub version: String,
}
pub struct Job {
    pub request: Value,
    pub reply: mpsc::Sender<Value>,
    pub expires_ms: u128,
    pub started: Arc<AtomicBool>,
}
struct WorkerGuard(Arc<AtomicUsize>);
impl Drop for WorkerGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}
#[derive(Clone)]
struct Snapshot {
    doc: Option<jenga_core::document::Document>,
    state: Value,
    at: u128,
}
pub struct Server {
    snapshot: Arc<RwLock<Option<Snapshot>>>,
    pub jobs: mpsc::Receiver<Job>,
    stop: Arc<AtomicBool>,
    path: PathBuf,
    pub endpoint: Endpoint,
}
pub fn session_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("JENGA_SESSION_DIR") {
        return PathBuf::from(dir);
    }
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Jenga")
        .join("sessions")
}
impl Server {
    pub fn snapshot_age(&self) -> u128 {
        self.snapshot
            .read()
            .ok()
            .and_then(|s| {
                s.as_ref()
                    .map(|s| crate::journal::now().saturating_sub(s.at))
            })
            .unwrap_or(u128::MAX)
    }
    pub fn publish(&self, doc: Option<jenga_core::document::Document>, state: Value) {
        if let Ok(mut s) = self.snapshot.write() {
            *s = Some(Snapshot {
                doc,
                state,
                at: crate::journal::now(),
            });
        }
    }
    pub fn start(wake: Arc<dyn Fn() + Send + Sync>) -> Result<Self> {
        Self::start_at(wake, session_dir())
    }
    fn start_at(wake: Arc<dyn Fn() + Send + Sync>, directory: PathBuf) -> Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        listener.set_nonblocking(true)?;
        let endpoint = Endpoint {
            pid: std::process::id(),
            address: listener.local_addr()?.to_string(),
            token: uuid::Uuid::new_v4().to_string(),
            created_ms: crate::journal::now(),
            version: env!("CARGO_PKG_VERSION").into(),
        };
        fs::create_dir_all(&directory)?;
        let path = directory.join(format!("{}.json", endpoint.pid));
        jenga_core::document::atomic_write(
            &path,
            serde_json::to_string(&endpoint)?.as_bytes(),
            false,
        )?;
        let (tx, jobs) = mpsc::sync_channel::<Job>(32);
        let stop = Arc::new(AtomicBool::new(false));
        let stop_thread = stop.clone();
        let token = endpoint.token.clone();
        let snapshot = Arc::new(RwLock::new(None::<Snapshot>));
        let reader_snapshot = snapshot.clone();
        let active = Arc::new(AtomicUsize::new(0));
        std::thread::spawn(move || {
            while !stop_thread.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, peer)) => {
                        if !peer.ip().is_loopback() {
                            continue;
                        }
                        if active.fetch_add(1, Ordering::Relaxed) >= 40 {
                            active.fetch_sub(1, Ordering::Relaxed);
                            continue;
                        }
                        let guard = WorkerGuard(active.clone());
                        let snapshot = reader_snapshot.clone();
                        let tx = tx.clone();
                        let wake = wake.clone();
                        let token = token.clone();
                        std::thread::spawn(move || {
                            let _guard = guard;
                            // Accepted Winsock sockets can inherit the listener's nonblocking mode.
                            if stream.set_nonblocking(false).is_err() {
                                return;
                            }
                            let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
                            let _ = stream.set_write_timeout(Some(Duration::from_secs(3)));
                            let mut reader = BufReader::new(&mut stream);
                            let mut bytes = vec![];
                            let mut oversize = false;
                            while let Ok(buf) = reader.fill_buf() {
                                if buf.is_empty() {
                                    break;
                                }
                                let n = buf
                                    .iter()
                                    .position(|b| *b == b'\n')
                                    .map(|i| i + 1)
                                    .unwrap_or(buf.len());
                                if bytes.len() + n > 1_048_576 {
                                    oversize = true;
                                    break;
                                }
                                bytes.extend_from_slice(&buf[..n]);
                                reader.consume(n);
                                if bytes.last() == Some(&b'\n') {
                                    break;
                                }
                            }
                            drop(reader);
                            let response = if oversize {
                                json!({"error":"Request exceeds 1 MiB"})
                            } else {
                                match serde_json::from_slice::<Value>(&bytes) {
                                    Ok(request)
                                        if request.get("token").and_then(Value::as_str)
                                            == Some(&token) =>
                                    {
                                        if request["method"] == "ping" {
                                            let _ = serde_json::to_writer(
                                                &mut stream,
                                                &json!({"result":{"responding":true}}),
                                            );
                                            let _ = stream.write_all(b"\n");
                                            return;
                                        }
                                        if let Some(s) =
                                            snapshot.read().ok().and_then(|s| s.clone())
                                        {
                                            let method = request["method"].as_str().unwrap_or("");
                                            if let Some(result) = snapshot_read(
                                                &s,
                                                method,
                                                request.get("params").unwrap_or(&json!({})),
                                            ) {
                                                let response = match result {
                                                    Ok(v) => json!({"result":v}),
                                                    Err(e) => json!({"error":e.to_string()}),
                                                };
                                                let _ =
                                                    serde_json::to_writer(&mut stream, &response);
                                                let _ = stream.write_all(b"\n");
                                                return;
                                            }
                                            if crate::journal::now().saturating_sub(s.at) > 3_000 {
                                                let _ = serde_json::to_writer(
                                                    &mut stream,
                                                    &json!({"error":"Desktop UI is inactive or modal-blocked; no edit accepted. Restore the window or close its dialog. Read-only inspect/diagnose still work; do not kill a desktop with unsaved edits."}),
                                                );
                                                let _ = stream.write_all(b"\n");
                                                return;
                                            }
                                        }
                                        let (rtx, rrx) = mpsc::channel();
                                        let expires_ms = crate::journal::now() + 3_000;
                                        let started = Arc::new(AtomicBool::new(false));
                                        match tx.try_send(Job {
                                            request,
                                            reply: rtx,
                                            expires_ms,
                                            started: started.clone(),
                                        }) {
                                            Ok(()) => {
                                                wake();
                                                match rrx.recv_timeout(Duration::from_secs(3)){
                                                Ok(v)=>v,
                                                Err(_)if !started.load(Ordering::Acquire)=>json!({"error":"Desktop UI did not start the request; no edit accepted. Close its modal dialog or restore the window; read-only inspect/diagnose remain available."}),
                                                Err(_)=>rrx.recv_timeout(Duration::from_secs(57)).unwrap_or_else(|_|json!({"error":"App response timed out after execution started; inspect revision/history before retrying. No operation was replayed."})),
                                            }
                                            }
                                            Err(_) => json!({"error":"App request queue busy"}),
                                        }
                                    }
                                    _ => json!({"error":"Unauthorized or malformed request"}),
                                }
                            };
                            let _ = serde_json::to_writer(&mut stream, &response);
                            let _ = stream.write_all(b"\n");
                        });
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(40))
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(Self {
            jobs,
            snapshot,
            stop,
            path,
            endpoint,
        })
    }
}
fn snapshot_read(s: &Snapshot, method: &str, p: &Value) -> Option<Result<Value>> {
    if ![
        "ui_state",
        "inspect",
        "erc",
        "connectivity",
        "diagnose",
        "trace_connection",
        "verify_connections",
    ]
    .contains(&method)
    {
        return None;
    }
    Some((|| {
        if method == "ui_state" {
            let mut state = s.state.clone();
            state["document"] = s
                .doc
                .as_ref()
                .map(crate::engine::inspect)
                .unwrap_or(Value::Null);
            state["snapshot_age_ms"] = json!(crate::journal::now().saturating_sub(s.at));
            state["read_only_snapshot"] = json!(true);
            return Ok(state);
        }
        let d = s.doc.as_ref().context("No document open")?;
        Ok(match method {
            "inspect" => crate::engine::inspect(d),
            "erc" => crate::engine::findings(d),
            "connectivity" => jenga_core::connectivity::inspect(d),
            "trace_connection" => crate::net_guard::trace(d, p)?,
            "verify_connections" => crate::net_guard::verify(d, p)?,
            _ => {
                json!({"revision":crate::journal::revision(d),"dirty":d.dirty,"snapshot_age_ms":crate::journal::now().saturating_sub(s.at),"file":d.path,"erc":{"issues":crate::engine::findings(d)},"connectivity":jenga_core::connectivity::inspect(d)})
            }
        })
    })())
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = fs::remove_file(&self.path);
    }
}
pub fn endpoints() -> Vec<Endpoint> {
    let mut out = vec![];
    if let Ok(entries) = fs::read_dir(session_dir()) {
        for entry in entries.flatten() {
            if let Ok(bytes) = fs::read(entry.path())
                && let Ok(e) = serde_json::from_slice::<Endpoint>(&bytes)
            {
                if process_alive(e.pid) != Some(false) {
                    out.push(e);
                }
            }
        }
    }
    out.sort_by_key(|e| std::cmp::Reverse(e.created_ms));
    out
}
pub fn call(endpoint: &Endpoint, method: &str, params: &Value) -> Result<Value> {
    let address: endpoint_address::Address = endpoint.address.parse()?;
    let mut stream = TcpStream::connect_timeout(&address.0, Duration::from_secs(2))?;
    stream.set_read_timeout(Some(Duration::from_secs(
        if matches!(method, "ping" | "ui_state") {
            2
        } else {
            65
        },
    )))?;
    serde_json::to_writer(
        &mut stream,
        &json!({"token":endpoint.token,"id":uuid::Uuid::new_v4().to_string(),"method":method,"params":params}),
    )?;
    stream.write_all(b"\n")?;
    let mut response = String::new();
    BufReader::new(stream).read_line(&mut response).context(if method=="ui_state"{"Desktop UI did not respond promptly. Close native modal dialogs or update/restart Jenga; use --headless only for an explicitly chosen saved file, with unsaved edits preserved"}else{"Desktop response lost; inspect revision/history before retrying. The operation was not replayed"})?;
    let v: Value = serde_json::from_str(&response)?;
    if let Some(e) = v.get("error") {
        anyhow::bail!("{e}");
    }
    Ok(v.get("result").cloned().unwrap_or(v))
}
mod endpoint_address {
    use super::*;
    pub struct Address(pub std::net::SocketAddr);
    impl std::str::FromStr for Address {
        type Err = anyhow::Error;
        fn from_str(s: &str) -> Result<Self> {
            let a: std::net::SocketAddr = s.parse().context("Invalid endpoint address")?;
            ensure!(
                a.ip().is_loopback(),
                "Only loopback endpoints are permitted"
            );
            Ok(Self(a))
        }
    }
}
/// Process liveness closes the short Windows socket-shutdown race. Unknown is not dead.
#[cfg(windows)]
pub fn process_alive(pid: u32) -> Option<bool> {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut std::ffi::c_void;
        fn GetExitCodeProcess(handle: *mut std::ffi::c_void, code: *mut u32) -> i32;
        fn CloseHandle(handle: *mut std::ffi::c_void) -> i32;
        fn GetLastError() -> u32;
    }
    unsafe {
        let h = OpenProcess(0x1000, 0, pid);
        if h.is_null() {
            return if GetLastError() == 87 {
                Some(false)
            } else {
                None
            };
        }
        let mut code = 0;
        let ok = GetExitCodeProcess(h, &mut code);
        CloseHandle(h);
        if ok == 0 { None } else { Some(code == 259) }
    }
}
#[cfg(not(windows))]
pub fn process_alive(_pid: u32) -> Option<bool> {
    None
}
pub fn connect(which: &str) -> Result<Endpoint> {
    for e in endpoints() {
        if process_alive(e.pid) == Some(false) {
            continue;
        }
        if (which == "auto" || which == e.pid.to_string()) && call(&e, "ping", &json!({})).is_ok() {
            return Ok(e);
        }
    }
    anyhow::bail!("No responding Jenga session; start Jenga or use headless mode")
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unstarted_ui_write_expires_without_execution() {
        let dir = tempfile::tempdir().unwrap();
        let server = Server::start_at(Arc::new(|| {}), dir.path().into()).unwrap();
        assert!(
            call(&server.endpoint, "edit", &json!({}))
                .unwrap_err()
                .to_string()
                .contains("no edit accepted")
        );
        let job = server.jobs.try_recv().unwrap();
        assert!(!job.started.load(Ordering::Acquire));
        assert!(crate::journal::now() >= job.expires_ms);
    }
    #[test]
    fn snapshot_inspection_does_not_require_a_ui_event() {
        let dir = tempfile::tempdir().unwrap();
        let server = Server::start_at(Arc::new(|| {}), dir.path().into()).unwrap();
        let doc = jenga_core::document::Document::new();
        server.publish(Some(doc), json!({"running_app":true,"indexing":false}));
        assert!(call(&server.endpoint, "inspect", &json!({})).unwrap()["revision"].is_string());
        assert_eq!(
            call(&server.endpoint, "ui_state", &json!({})).unwrap()["read_only_snapshot"],
            true
        );
        assert!(server.jobs.try_recv().is_err());
    }
    #[test]
    fn discovery_ping_does_not_wait_for_the_ui_thread() {
        let dir = tempfile::tempdir().unwrap();
        let server = Server::start_at(Arc::new(|| {}), dir.path().to_owned()).unwrap();
        assert_eq!(
            call(&server.endpoint, "ping", &json!({})).unwrap()["responding"],
            true
        );
        assert!(server.jobs.try_recv().is_err());
    }
    #[test]
    fn pending_ui_job_does_not_block_discovery() {
        let dir = tempfile::tempdir().unwrap();
        let server = Server::start_at(Arc::new(|| {}), dir.path().into()).unwrap();
        let endpoint = server.endpoint.clone();
        let client = std::thread::spawn(move || call(&endpoint, "inspect", &json!({})));
        let job = server.jobs.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(
            call(&server.endpoint, "ping", &json!({})).unwrap()["responding"],
            true
        );
        job.reply.send(json!({"result":true})).unwrap();
        assert_eq!(client.join().unwrap().unwrap(), true);
    }
    #[test]
    fn accepts_delayed_client_bytes_and_rejects_wrong_token() {
        let dir = tempfile::tempdir().unwrap();
        let server = Server::start_at(Arc::new(|| {}), dir.path().to_owned()).unwrap();
        let endpoint = server.endpoint.clone();
        let client = std::thread::spawn(move || call(&endpoint, "inspect", &json!({})));
        let job = server.jobs.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(job.request["method"], "inspect");
        job.reply.send(json!({"result":{"ok":true}})).unwrap();
        assert_eq!(client.join().unwrap().unwrap()["ok"], true);
        let mut wrong = server.endpoint.clone();
        wrong.token = "wrong".into();
        assert!(call(&wrong, "inspect", &json!({})).is_err());
        assert!(server.jobs.try_recv().is_err());
    }
}
