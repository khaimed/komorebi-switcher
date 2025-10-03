use anyhow::{anyhow, Context, Result};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Try to fetch GlazeWM state via CLI and return the raw stdout as text.
static ARGS_SUCCESS: OnceLock<Vec<&'static str>> = OnceLock::new();

pub fn query_state_text() -> Result<String> {
    // Candidate commands that may output JSON state/workspaces
    let candidates: &[&[&str]] = &[
        // Prefer explicit JSON outputs first
        &["query", "workspaces", "--json"],
        &["query", "state", "--json"],
        &["status", "--json"],
        // Fall back to plain text outputs
        &["query", "workspaces"],
        &["query", "state"],
        &["status"],
    ];

    // Try the last known successful args first to avoid spawning multiple processes.
    if let Some(saved) = ARGS_SUCCESS.get() {
        let mut cmd = Command::new("glazewm");
        cmd.args(saved);
        #[cfg(windows)]
        {
            cmd.creation_flags(CREATE_NO_WINDOW)
                .stdin(Stdio::null())
                .stderr(Stdio::null());
        }
        if let Ok(output) = cmd.output() {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout).to_string();
                let first_line = text.lines().next().unwrap_or("").trim();
                tracing::debug!(
                    target: "glazewm",
                    "CLI succeeded (cached): args={:?}, len={}, first_line='{}'",
                    saved,
                    text.len(),
                    first_line
                );
                if !text.trim().is_empty() {
                    return Ok(text);
                }
            }
        }
    }

    for args in candidates {
        let mut cmd = Command::new("glazewm");
        cmd.args(*args);
        #[cfg(windows)]
        {
            cmd.creation_flags(CREATE_NO_WINDOW)
                .stdin(Stdio::null())
                .stderr(Stdio::null());
        }

        if let Ok(output) = cmd.output() {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout).to_string();
                let first_line = text.lines().next().unwrap_or("").trim();
                tracing::debug!(
                    target: "glazewm",
                    "CLI succeeded: args={:?}, len={}, first_line='{}'",
                    args,
                    text.len(),
                    first_line
                );
                if !text.trim().is_empty() {
                    // Cache the successful args to use next time.
                    let _ = ARGS_SUCCESS.set(args.to_vec());
                    return Ok(text);
                }
            } else {
                let code = output.status.code().unwrap_or(-1);
                tracing::warn!(target: "glazewm", "CLI failed: args={:?}, code={}", args, code);
            }
        }
    }

    Err(anyhow!("Unable to query GlazeWM state via CLI"))
}

/// Attempt to focus/change the workspace using GlazeWM CLI.
pub fn focus_workspace(workspace_idx_zero_based: usize) -> Result<()> {
    // GlazeWM workspaces are 1-based. Map our zero-based index to 1-based.
    let n = (workspace_idx_zero_based + 1).to_string();
    // GlazeWM v3 CLI expects invoking commands via `command <...>`
    // Correct CLI: `glazewm command focus --workspace N`
    let mut cmd = Command::new("glazewm");
    cmd.args(["command", "focus", "--workspace", &n]);
    #[cfg(windows)]
    {
        cmd.creation_flags(CREATE_NO_WINDOW)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
    }
    let status = cmd
        .status()
        .context("failed to invoke glazewm command focus --workspace")?;

    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("glazewm command focus --workspace failed"))
    }
}