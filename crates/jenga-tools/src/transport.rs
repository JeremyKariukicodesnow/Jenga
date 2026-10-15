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
        Arc,
        atomic::{AtomicBool, Ordering},
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
}
pub struct Server {
    pub jobs: mpsc::Receiver<Job>,
    stop: Arc<AtomicBool>,
    path: PathBuf,
    pub endpoint: Endpoint,
}
pub fn session_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Jenga")
        .join("sessions")
}
impl Server {
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
        std::thread::spawn(move || {
            while !stop_thread.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, peer)) => {
                        if !peer.ip().is_loopback() {
                            continue;
                        }
                        // Accepted Winsock sockets can inherit the listener's nonblocking mode.
                        if stream.set_nonblocking(false).is_err() {
                            continue;
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
                                    let (rtx, rrx) = mpsc::channel();
                                    match tx.try_send(Job {
                                        request,
                                        reply: rtx,
                                    }) {
                                        Ok(()) => {
                                            wake();
                                            rrx.recv_timeout(Duration::from_secs(60)).unwrap_or_else(|_|json!({"error":"App response timed out; inspect revision before retrying a mutation"}))
                                        }
                                        Err(_) => json!({"error":"App request queue busy"}),
                                    }
                                }
                                _ => json!({"error":"Unauthorized or malformed request"}),
                            }
                        };
                        let _ = serde_json::to_writer(&mut stream, &response);
                        let _ = stream.write_all(b"\n");
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
            stop,
            path,
            endpoint,
        })
    }
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
                out.push(e);
            }
        }
    }
    out.sort_by_key(|e| std::cmp::Reverse(e.created_ms));
    out
}
pub fn call(endpoint: &Endpoint, method: &str, params: &Value) -> Result<Value> {
    let address: endpoint_address::Address = endpoint.address.parse()?;
    let mut stream = TcpStream::connect_timeout(&address.0, Duration::from_secs(2))?;
    stream.set_read_timeout(Some(Duration::from_secs(65)))?;
    serde_json::to_writer(
        &mut stream,
        &json!({"token":endpoint.token,"id":uuid::Uuid::new_v4().to_string(),"method":method,"params":params}),
    )?;
    stream.write_all(b"\n")?;
    let mut response = String::new();
    BufReader::new(stream).read_line(&mut response)?;
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
pub fn connect(which: &str) -> Result<Endpoint> {
    for e in endpoints() {
        if (which == "auto" || which == e.pid.to_string())
            && call(&e, "ui_state", &json!({})).is_ok()
        {
            return Ok(e);
        }
    }
    anyhow::bail!("No responding Jenga session; start Jenga or use headless mode")
}
#[cfg(test)]
mod tests {
    use super::*;
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
