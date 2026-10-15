# AI assistants and Jenga

Jenga exposes deterministic engineering tools. The external assistant provides the model, reasoning and any provider credentials. Using Jenga manually requires none of those. No specific model vendor is required, and naming a client here is not an endorsement or proof that every client version was tested.

## Supported connection approaches

| Client type | Connection |
| --- | --- |
| Codex desktop/CLI | Built-in Codex setup helper; local MCP stdio tools and installable Jenga skill. |
| Claude Code | Built-in Claude Code configuration helper; client must permit the local MCP process. |
| Cursor | Built-in Cursor configuration helper for local MCP stdio. |
| Antigravity | Built-in Antigravity configuration helper; verify the installed host's MCP support/settings. |
| Other local MCP clients | Generate generic configuration and place it in the client's MCP settings. Client must support launching a stdio server. |
| Agents with shell/tool execution | Invoke the CLI or newline JSON API directly, using discovered schemas and explicit project targets. |
| Browser-only ChatGPT or another web chat | Cannot automatically launch a local Windows process or access your files. A compatible local client/connector is required; Jenga does not supply a public internet MCP endpoint. |

These are integration mechanisms, not guarantees about an assistant's electronics expertise or a model's subscription availability.

## Setup

Open **Connect Assistant** in the desktop application. It locates the installed executable and offers configuration/skill setup. Or, from PowerShell, invoke the installed CLI by its full path:

```powershell
$jengaCli = "$env:LOCALAPPDATA\Programs\Jenga\jenga-cli.exe"
& $jengaCli setup --client Codex --workspace "$env:USERPROFILE\Documents" --install --skill
```

Replace the client value with `"Claude Code"`, `Cursor` or `Antigravity` for the corresponding helper. Choose an existing workspace folder. Configuration updates preserve other entries and make backups; conflicting non-Jenga registrations are not overwritten. Inspect the result in the client before enabling tool access.

For other clients, omit `--install` and request a generic snippet:

```powershell
& $jengaCli setup --client Other --workspace "$env:USERPROFILE\Documents"
```

Use the generated absolute `jenga-connect.exe` path and argument array. Do not paste a temporary/versioned build path, and do not assume environment variables inside a JSON command are expanded by every host.

The bridge lives outside the replaced app directory. One restart adopts it for an existing assistant registration. Later upgrades/downgrades automatically restore the worker without repasting configuration or routinely asking the user to reconnect. Hosts without MCP tool-list refresh may still require a host restart. A terminated assistant/bridge process cannot be resurrected by Jenga; the host must start its configured server.

After interruption, the agent should wait for readiness, inspect capabilities and the retained operation ID, and reopen the intended saved document as necessary. Never repeat an uncertain edit or discard unsaved human work just to resume. Downgrading cannot preserve newer tool capabilities that the older version does not implement.

## A useful first task

> Use Jenga to inspect the current project and available tools. Identify the intended schematic or PCB explicitly. Preview a small change, report affected nets, and commit/save only after verification at the expected revision. Use the Jenga skill. Preserve human edits and disclose unknown component specifications.

Start with `capabilities`, `ui_state`, the installed schemas and the skill; names/availability vary by version. The tool service also works headlessly on explicit saved files. Long calculations can run in the assistant process while the desktop remains usable or closes. Use operation status/progress/cancellation instead of retry loops.

## Privacy and engineering limits

Jenga stores project data locally, but tool responses and selected files can be sent to the external AI provider by your client. Review that client's permissions, provider terms and data handling before sharing private designs. Never embed model-provider keys in project files.

The skill asks assistants to find existing/manufacturer assets before recreating parts, retain licensing/source information, verify exact part numbers/pin maps/power budgets, draw full visible wires and use clear labels. None of this turns an unverified model into an engineer or certifies a PCB. Inspect native ERC/DRC, models, datasheets and physical prototypes yourself.
