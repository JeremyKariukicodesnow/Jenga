//! Stable stdio bridge. Never replay an interrupted engineering request.
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::{self, BufRead, Write},
    process::{Command, Stdio},
    sync::mpsc,
    time::Duration,
};
enum Event {
    Input(String),
    Output(u64, String),
    Closed(u64),
    End,
}
/// Do not relaunch the old worker while an installer is replacing it. This
/// also works with pre-0.8 installers that do not know about the bridge.
#[cfg(windows)]
fn maintenance_running() -> bool {
    #[repr(C)]
    struct Entry {
        size: u32,
        usage: u32,
        pid: u32,
        heap: usize,
        module: u32,
        threads: u32,
        parent: u32,
        priority: i32,
        flags: u32,
        name: [u16; 260],
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateToolhelp32Snapshot(flags: u32, pid: u32) -> *mut std::ffi::c_void;
        fn Process32FirstW(h: *mut std::ffi::c_void, e: *mut Entry) -> i32;
        fn Process32NextW(h: *mut std::ffi::c_void, e: *mut Entry) -> i32;
        fn CloseHandle(h: *mut std::ffi::c_void) -> i32;
    }
    unsafe {
        let h = CreateToolhelp32Snapshot(2, 0);
        if h as isize == -1 {
            return false;
        }
        let mut e: Entry = std::mem::zeroed();
        e.size = std::mem::size_of::<Entry>() as u32;
        let mut ok = Process32FirstW(h, &mut e);
        let mut found = false;
        while ok != 0 {
            let end = e.name.iter().position(|x| *x == 0).unwrap_or(260);
            let name = String::from_utf16_lossy(&e.name[..end]).to_lowercase();
            if name.starts_with("jenga-") && name.ends_with("-setup.exe") {
                found = true;
                break;
            }
            ok = Process32NextW(h, &mut e);
        }
        CloseHandle(h);
        found
    }
}
#[cfg(not(windows))]
fn maintenance_running() -> bool {
    false
}
fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let (tx, rx) = mpsc::channel();
    let input = tx.clone();
    std::thread::spawn(move || {
        for line in io::stdin().lock().lines().map_while(Result::ok) {
            if input.send(Event::Input(line)).is_err() {
                return;
            }
        }
        let _ = input.send(Event::End);
    });
    let mut child: Option<std::process::Child> = None;
    let mut generation = 0;
    let mut initialize: Option<Value> = None;
    let mut pending = BTreeMap::<String, (Value, Option<String>)>::new();
    let mut ready = false;
    let mut next_attempt = std::time::Instant::now();
    let mut out = io::stdout().lock();
    loop {
        if child.is_none() && std::time::Instant::now() >= next_attempt && !maintenance_running() {
            next_attempt = std::time::Instant::now() + Duration::from_secs(2);
            // Resolve the installed worker on every restart, including relocated installs.
            let exe = jenga_tools::setup::worker_executable();
            let mut cmd = Command::new(exe);
            cmd.args(&args)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit());
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(0x08000000);
            }
            if let Ok(mut worker) = cmd.spawn() {
                generation += 1;
                let epoch = generation;
                let stdout = worker.stdout.take().unwrap();
                let sender = tx.clone();
                std::thread::spawn(move || {
                    for line in io::BufReader::new(stdout).lines().map_while(Result::ok) {
                        if sender.send(Event::Output(epoch, line)).is_err() {
                            return;
                        }
                    }
                    let _ = sender.send(Event::Closed(epoch));
                });
                if let Some(init) = &initialize {
                    let mut init = init.clone();
                    init["id"] = json!("jenga:reinitialize");
                    writeln!(worker.stdin.as_mut().unwrap(), "{init}")?;
                    ready = false;
                } else {
                    ready = true;
                }
                child = Some(worker);
            }
        }
        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(Event::End) => break,
            Ok(Event::Closed(epoch)) if epoch == generation => {
                if let Some(mut c) = child.take() {
                    let _ = c.wait();
                }
                ready = false;
                next_attempt = std::time::Instant::now() + Duration::from_secs(2);
                for (_, (id, operation_id)) in std::mem::take(&mut pending) {
                    writeln!(
                        out,
                        "{}",
                        json!({"jsonrpc":"2.0","id":id,"error":{"data":{"operation_id":operation_id},"code":-32001,"message":"Jenga restarted or updated. Request was NOT replayed. Inspect operation_status using its operation_id before retrying; reopen the intended saved document if needed."}})
                    )?;
                }
                out.flush()?;
            }
            Ok(Event::Output(epoch, line)) if epoch == generation => {
                let v: Value = serde_json::from_str(&line)?;
                if v["id"] == "jenga:reinitialize" {
                    ready = v.get("result").is_some();
                    if ready {
                        if let Some(c) = child.as_mut() {
                            writeln!(
                                c.stdin.as_mut().unwrap(),
                                "{}",
                                json!({"jsonrpc":"2.0","method":"notifications/initialized"})
                            )?;
                        }
                        writeln!(
                            out,
                            "{}",
                            json!({"jsonrpc":"2.0","method":"notifications/tools/list_changed"})
                        )?;
                    }
                } else {
                    if let Some(id) = v.get("id") {
                        pending.remove(&id.to_string());
                    }
                    let mut v = v;
                    if v["result"]["capabilities"]["tools"].is_object() {
                        v["result"]["capabilities"]["tools"]["listChanged"] = json!(true);
                    }
                    writeln!(out, "{v}")?;
                }
                out.flush()?;
            }
            Ok(Event::Input(line)) => {
                let mut v: Value = match serde_json::from_str(&line) {
                    Ok(v) => v,
                    Err(_) => {
                        writeln!(
                            out,
                            "{}",
                            json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"Parse error"}})
                        )?;
                        out.flush()?;
                        continue;
                    }
                };
                if let Some(method) = v["params"]["name"]
                    .as_str()
                    .and_then(|s| s.strip_prefix("jenga_"))
                    .map(str::to_owned)
                {
                    if jenga_tools::operations::mutation(&method)
                        || matches!(method.as_str(), "pcb_verify" | "pcb_3d" | "preview_edit")
                    {
                        if v["params"]["arguments"].is_null() {
                            v["params"]["arguments"] = json!({});
                        }
                        if let Some(args) = v["params"]["arguments"].as_object_mut() {
                            args.entry("operation_id")
                                .or_insert_with(|| json!(uuid::Uuid::new_v4().to_string()));
                        }
                    }
                }
                if v["method"] == "initialize" {
                    initialize = Some(v.clone());
                }
                if ready && child.is_some() {
                    if let Some(id) = v.get("id") {
                        if pending.contains_key(&id.to_string()) {
                            writeln!(
                                out,
                                "{}",
                                json!({"jsonrpc":"2.0","id":id,"error":{"code":-32600,"message":"Request ID already pending; no duplicate request accepted"}})
                            )?;
                            out.flush()?;
                            continue;
                        }
                        pending.insert(
                            id.to_string(),
                            (
                                id.clone(),
                                v["params"]["arguments"]["operation_id"]
                                    .as_str()
                                    .map(str::to_owned),
                            ),
                        );
                    }
                    // A write failure is indeterminate: never retry this line on another worker.
                    if writeln!(child.as_mut().unwrap().stdin.as_mut().unwrap(), "{v}").is_err() {
                        if let Some(mut c) = child.take() {
                            let _ = c.kill();
                            let _ = c.wait();
                        }
                        ready = false;
                        for (_, (id, operation_id)) in std::mem::take(&mut pending) {
                            writeln!(
                                out,
                                "{}",
                                json!({"jsonrpc":"2.0","id":id,"error":{"data":{"operation_id":operation_id},"code":-32001,"message":"Worker connection was lost. Request was not replayed; inspect its operation_id and saved revisions before retrying."}})
                            )?;
                        }
                        out.flush()?;
                    }
                } else if let Some(id) = v.get("id") {
                    writeln!(
                        out,
                        "{}",
                        json!({"jsonrpc":"2.0","id":id,"error":{"code":-32002,"message":"Jenga is restarting or being installed. No request accepted; retry after installation completes."}})
                    )?;
                    out.flush()?;
                }
            }
            _ => {}
        }
    }
    if let Some(mut c) = child {
        let _ = c.kill();
        let _ = c.wait();
    }
    Ok(())
}
