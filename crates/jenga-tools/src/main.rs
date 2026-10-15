use anyhow::{Context, Result, ensure};
use jenga_tools::{
    engine::{Engine, files},
    protocol, transport,
};
use serde_json::{Value, json};
use std::{
    io::{self, BufRead, Write},
    path::PathBuf,
};
fn flag(args: &[String], name: &str) -> Option<String> {
    args.windows(2).find(|w| w[0] == name).map(|w| w[1].clone())
}
fn json_input(s: &str) -> Result<Value> {
    if s.trim_start().starts_with('{') {
        Ok(serde_json::from_str(s)?)
    } else {
        Ok(serde_json::from_str(&std::fs::read_to_string(s)?)?)
    }
}
fn main() {
    if let Err(e) = run() {
        eprintln!("Jenga: {e:#}");
        std::process::exit(1);
    }
}
fn run() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let command = args.first().map(String::as_str).unwrap_or("help");
    if command == "help" || command == "--help" {
        if args.iter().any(|s| s == "--json") || flag(&args, "--topic").is_some() {
            println!(
                "{}",
                serde_json::to_string_pretty(&jenga_tools::help::get(
                    &flag(&args, "--topic").unwrap_or("tools".into())
                )?)?
            );
            return Ok(());
        }
        println!(
            "Jenga tools {}\n\nNo built-in AI: deterministic CLI, MCP and JSON for external clients.\n\n  jenga-cli mcp [--workspace DIR] [--connect auto|PID]\n  jenga-cli json [--workspace DIR] [--connect auto|PID]\n  jenga-cli run --request REQUEST.json [--file FILE] [--save] [--connect auto|PID]\n  jenga-cli inspect|erc|history --file FILE\n  jenga-cli search_symbols --query resistor\n  jenga-cli export|screenshot --file FILE --output FILE [--format pdf]\n  jenga-cli batch --folder DIR --request EDIT.json [--dry-run]\n  jenga-cli sessions\n\njenga-cli help --json or help --topic editing\n  jenga-cli setup --client Codex --workspace DIR [--mode headless|live] [--install] [--skill]\n\nSee AUTOMATION.md for schemas and client configurations.",
            env!("CARGO_PKG_VERSION")
        );
        return Ok(());
    }
    if command == "--version" {
        println!("{}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if command == "sessions" {
        let entries=transport::endpoints().iter().map(|e|json!({"pid":e.pid,"address":e.address,"version":e.version,"responding":transport::call(e,"ui_state",&json!({})).is_ok()})).collect::<Vec<_>>();
        println!("{}", serde_json::to_string_pretty(&entries)?);
        return Ok(());
    }
    if command == "setup" {
        let client = flag(&args, "--client").unwrap_or("Other".into());
        let mode = flag(&args, "--mode").unwrap_or("headless".into());
        let workspace = flag(&args, "--workspace")
            .map(PathBuf::from)
            .unwrap_or(std::env::current_dir()?);
        let exe = jenga_tools::setup::executable();
        let home = jenga_tools::setup::home();
        if args.iter().any(|s| s == "--install") {
            println!(
                "Installed {}",
                jenga_tools::setup::install(&client, &mode, &workspace, &exe, &home)?.display()
            );
        } else {
            println!(
                "{}",
                jenga_tools::setup::snippet(&client, &mode, &workspace, &exe)?
            );
        }
        if args.iter().any(|s| s == "--skill") {
            println!(
                "Skill installed at {}",
                jenga_tools::setup::install_skill(&client, &home)?.display()
            );
        }
        return Ok(());
    }
    let workspace = flag(&args, "--workspace")
        .map(PathBuf::from)
        .unwrap_or(std::env::current_dir()?);
    ensure!(workspace.is_dir(), "Workspace must exist");
    let mut engine = Engine::new(workspace);
    if let Some(file) = flag(&args, "--file") {
        engine.request("open", &json!({"path":file}))?;
    }
    if command == "batch" {
        let folder = PathBuf::from(flag(&args, "--folder").context("batch requires --folder")?);
        let mut request =
            json_input(&flag(&args, "--request").context("batch requires --request")?)?;
        if request.get("method").is_some() {
            ensure!(request["method"] == "edit", "Batch request must be edit");
            request = request["params"].clone();
        }
        request
            .as_object_mut()
            .context("Request must be object")?
            .entry("actor")
            .or_insert(json!("external:batch"));
        let mut staged = vec![];
        for path in files(&folder, 0)?
            .into_iter()
            .filter(|p| p.extension().is_some_and(|e| e == "kicad_sch"))
        {
            engine.request("open", &json!({"path":path,"discard_changes":true}))?;
            let before = engine.document()?.clone();
            engine.load_libraries()?;
            let result = jenga_tools::engine::edit(
                engine.doc.as_mut().unwrap(),
                engine.libraries.as_mut().unwrap(),
                &request,
            )?;
            let after = engine.document()?.clone();
            staged.push((before, after, result));
        }
        let dry = args.iter().any(|s| s == "--dry-run");
        let mut report = vec![];
        for (before, mut after, result) in staged {
            let path = after.path.clone().unwrap();
            if !dry {
                ensure!(
                    jenga_tools::journal::revision(&jenga_core::document::Document::open(&path)?)
                        == jenga_tools::journal::revision(&before),
                    "File changed during batch: {}",
                    path.display()
                );
                after.save(&path)?;
                jenga_tools::journal::record(
                    &before,
                    &after,
                    request["actor"].as_str().unwrap_or("external:batch"),
                    "batch",
                )?;
            }
            report.push(json!({"file":path,"dry_run":dry,"edit":result,"erc":jenga_tools::engine::findings(&after)}));
        }
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }
    let endpoint = flag(&args, "--connect")
        .map(|s| transport::connect(&s))
        .transpose()?;
    let mut execute = |method: &str, p: &Value| -> Result<Value> {
        if let Some(e) = &endpoint {
            transport::call(e, method, p)
        } else {
            engine.request(method, p)
        }
    };
    match command {
        "mcp" => protocol::mcp(io::stdin().lock(), io::stdout().lock(), execute)?,
        "json" => {
            for line in io::stdin().lock().lines() {
                let line = line?;
                if line.len() > 1_048_576 {
                    anyhow::bail!("Request exceeds 1 MiB");
                }
                let response = match serde_json::from_str::<Value>(&line) {
                    Ok(v) => {
                        let method = v["method"].as_str().unwrap_or("");
                        let p = v.get("params").cloned().unwrap_or(json!({}));
                        match execute(method, &p) {
                            Ok(result) => json!({"id":v.get("id"),"result":result}),
                            Err(e) => json!({"id":v.get("id"),"error":e.to_string()}),
                        }
                    }
                    Err(e) => json!({"error":e.to_string()}),
                };
                println!("{response}");
                io::stdout().flush()?;
            }
        }
        "run" => {
            let v = json_input(
                &flag(&args, "--request").context("run requires --request JSON or path")?,
            )?;
            let method = v["method"].as_str().unwrap_or("edit");
            let p = v.get("params").unwrap_or(&v);
            let out = execute(method, p)?;
            if args.iter().any(|s| s == "--save") {
                execute(
                    "save",
                    &json!({"actor":p.get("actor").unwrap_or(&json!("external:cli"))}),
                )?;
            }
            println!("{}", serde_json::to_string_pretty(&out)?);
        }
        "inspect" | "erc" | "history" => println!(
            "{}",
            serde_json::to_string_pretty(&execute(command, &json!({}))?)?
        ),
        "search_symbols" => println!(
            "{}",
            serde_json::to_string_pretty(&execute(
                command,
                &json!({"query":flag(&args,"--query").unwrap_or_default()})
            )?)?
        ),
        "export" | "screenshot" => println!(
            "{}",
            execute(
                command,
                &json!({"path":flag(&args,"--output").context("Output path required")?,"format":flag(&args,"--format").unwrap_or_else(||if command=="screenshot"{"png".into()}else{"svg".into()})})
            )?
        ),
        _ => anyhow::bail!("Unknown command: {command}; run --help"),
    }
    Ok(())
}
