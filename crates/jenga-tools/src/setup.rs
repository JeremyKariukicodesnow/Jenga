//! Client registration helpers. No assistant account, inference or credentials.
use anyhow::{Context, Result, bail, ensure};
use jenga_core::document::atomic_write;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
pub const SKILL: &str = include_str!("../../../docs/assistant-skill/jenga-design/SKILL.md");
pub fn executable() -> PathBuf {
    std::env::current_exe()
        .unwrap_or_default()
        .with_file_name("jenga-cli.exe")
}
pub fn home() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_default()
}
pub fn config_path(client: &str, home: &Path) -> Result<PathBuf> {
    Ok(home.join(match client {
        "Codex" => ".codex/config.toml",
        "Claude Code" => ".claude.json",
        "Cursor" => ".cursor/mcp.json",
        "Antigravity" => ".gemini/config/mcp_config.json",
        _ => {
            bail!("For other clients, copy the generic stdio configuration into their MCP settings")
        }
    }))
}
pub fn snippet(client: &str, mode: &str, workspace: &Path, exe: &Path) -> Result<String> {
    ensure!(
        mode == "headless" || mode == "live",
        "Choose headless or live"
    );
    let args = if mode == "live" {
        json!(["mcp", "--connect", "auto"])
    } else {
        json!(["mcp", "--workspace", workspace])
    };
    let name = if mode == "live" {
        "jenga_live"
    } else {
        "jenga"
    };
    if client == "Codex" {
        Ok(format!(
            "[mcp_servers.{name}]\ncommand = {}\nargs = {}\n",
            json!(exe),
            args
        ))
    } else {
        let mut entry = json!({"command":exe,"args":args});
        if client == "Cursor" {
            entry["type"] = json!("stdio");
        }
        Ok(serde_json::to_string_pretty(
            &json!({"mcpServers":{name:entry}}),
        )?)
    }
}
/// Installs one entry only; preserve other client settings and refuse conflicting entries.
pub fn install(
    client: &str,
    mode: &str,
    workspace: &Path,
    exe: &Path,
    home: &Path,
) -> Result<PathBuf> {
    ensure!(exe.is_file(), "Jenga CLI is missing next to the app");
    ensure!(
        mode == "live" || workspace.is_dir(),
        "Choose an existing workspace folder"
    );
    let path = config_path(client, home)?;
    let snippet = snippet(client, mode, workspace, exe)?;
    let existing = if path.exists() {
        std::fs::read_to_string(&path)?
    } else {
        String::new()
    };
    let name = if mode == "live" {
        "jenga_live"
    } else {
        "jenga"
    };
    let data = if client == "Codex" {
        let parsed: toml::Table = existing
            .parse()
            .context("Existing Codex configuration is invalid TOML; no changes made")?;
        ensure!(
            parsed
                .get("mcp_servers")
                .and_then(|v| v.get(name))
                .is_none(),
            "{name} is already configured; review its entry instead of overwriting it"
        );
        let header = format!("[mcp_servers.{name}]");
        ensure!(
            !existing.contains(&header),
            "{name} is already configured. Review its entry instead of overwriting it"
        );
        let result = format!("{existing}\n{snippet}");
        result
            .parse::<toml::Table>()
            .context("Generated configuration did not validate")?;
        result
    } else {
        let mut config: Value = if existing.trim().is_empty() {
            json!({})
        } else {
            serde_json::from_str(&existing)
                .context("Existing client configuration is invalid JSON; no changes made")?
        };
        ensure!(
            config.is_object(),
            "Client configuration must be a JSON object"
        );
        if config.get("mcpServers").is_none() {
            config["mcpServers"] = json!({});
        }
        ensure!(
            config["mcpServers"].is_object(),
            "mcpServers must be an object"
        );
        ensure!(
            config["mcpServers"].get(name).is_none(),
            "{name} is already configured; no entry was overwritten"
        );
        config["mcpServers"][name] =
            serde_json::from_str::<Value>(&snippet)?["mcpServers"][name].clone();
        serde_json::to_string_pretty(&config)?
    };
    atomic_write(&path, data.as_bytes(), true)?;
    Ok(path)
}
pub fn install_skill(client: &str, home: &Path) -> Result<PathBuf> {
    let parent = match client {
        "Codex" => ".codex/skills",
        "Claude Code" => ".claude/skills",
        "Cursor" => ".cursor/skills",
        _ => bail!("Export the skill folder and install it in your client's skills directory"),
    };
    let path = home.join(parent).join("jenga-design/SKILL.md");
    ensure!(
        !path.exists(),
        "jenga-design skill already exists; no custom instructions overwritten"
    );
    atomic_write(&path, SKILL.as_bytes(), false)?;
    Ok(path)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn merges_without_losing_client_settings_or_overwriting_entries() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("jenga-cli.exe");
        std::fs::write(&exe, "").unwrap();
        let path = config_path("Cursor", dir.path()).unwrap();
        atomic_write(
            &path,
            br#"{"theme":"light","mcpServers":{"other":{"command":"other"}}}"#,
            false,
        )
        .unwrap();
        install("Cursor", "headless", dir.path(), &exe, dir.path()).unwrap();
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(v["theme"], "light");
        assert_eq!(v["mcpServers"]["other"]["command"], "other");
        assert!(install("Cursor", "headless", dir.path(), &exe, dir.path()).is_err());
        install("Cursor", "live", dir.path(), &exe, dir.path()).unwrap();
        install("Codex", "headless", dir.path(), &exe, dir.path()).unwrap();
        install_skill("Codex", dir.path()).unwrap();
        assert!(install_skill("Codex", dir.path()).is_err());
    }
}
