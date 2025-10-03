use std::time::Duration;
use winit::event_loop::EventLoopProxy;

use crate::app::AppMessage;
mod client;

/// Read GlazeWM state by invoking the `glazewm` CLI if available.
/// Falls back to empty/default state when unavailable.
pub fn read_state() -> anyhow::Result<crate::state::State> {
    if let Ok(text) = client::query_state_text() {
        let head = text.lines().next().unwrap_or("").trim();
        tracing::debug!(target: "glazewm", "raw text len={}, head='{}'", text.len(), head);
        // Prefer grouped parsing that yields per-monitor workspace sets
        if let Ok(state) = parse_state_json_grouped(&text) {
            let count0 = state.monitors.get(0).map(|m| m.workspaces.len()).unwrap_or(0);
            tracing::debug!(target: "glazewm", "parsed grouped JSON monitors={}, ws[0]={}", state.monitors.len(), count0);
            return Ok(state);
        }
        if let Ok(state) = parse_state_json(&text) {
            let count = state.monitors.get(0).map(|m| m.workspaces.len()).unwrap_or(0);
            tracing::debug!(target: "glazewm", "parsed JSON workspaces={}", count);
            return Ok(state);
        }
        // Fallback: try parsing as plain text lines of workspace names
        if let Ok(state) = parse_plain_workspaces(&text) {
            let count = state.monitors.get(0).map(|m| m.workspaces.len()).unwrap_or(0);
            tracing::debug!(target: "glazewm", "parsed plain workspaces={}", count);
            return Ok(state);
        }
    }

    Ok(Default::default())
}

fn parse_state_json(text: &str) -> anyhow::Result<crate::state::State> {
    let v: serde_json::Value = serde_json::from_str(text)?;
    // Attempt to parse a flexible layout: either `{ workspaces: [...] }` or `[ ... ]`
    let mut workspaces = Vec::new();
    if let Some(ws) = v.get("workspaces").and_then(|x| x.as_array()) {
        workspaces = ws.clone();
    } else if let Some(ws) = v
        .get("data")
        .and_then(|d| d.get("workspaces"))
        .and_then(|x| x.as_array())
    {
        workspaces = ws.clone();
    } else if let Some(arr) = v.as_array() {
        workspaces = arr.clone();
    }

    // Try to obtain a focused workspace index from various possible fields
    let focused_idx_top = v
        .get("workspaces")
        .and_then(|w| w.get("focused").or_else(|| w.get("focusedIndex")))
        .and_then(|x| x.as_u64())
        .map(|u| u as usize)
        .or_else(|| v.get("focusedWorkspaceIndex").and_then(|x| x.as_u64()).map(|u| u as usize));

    let mut k_workspaces: Vec<crate::state::Workspace> = Vec::new();
    for (idx, w) in workspaces.into_iter().enumerate() {
        let name = if w.is_string() {
            w.as_str().unwrap_or("").to_string()
        } else {
            w.get("name")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| (idx + 1).to_string())
        };
        let focused = w
            .get("focused")
            .or_else(|| w.get("isActive"))
            .or_else(|| w.get("active"))
            .or_else(|| w.get("is_focused"))
            .and_then(|x| x.as_bool())
            .unwrap_or_else(|| focused_idx_top.map(|f| f == idx).unwrap_or(false));

        let is_empty = if w.is_string() {
            false
        } else if let Some(b) = w.get("isEmpty").and_then(|x| x.as_bool()) {
            b
        } else if let Some(b) = w.get("empty").and_then(|x| x.as_bool()) {
            b
        } else if let Some(b) = w.get("hasWindows").and_then(|x| x.as_bool()) {
            !b
        } else {
            false
        };

        k_workspaces.push(crate::state::Workspace {
            name,
            index: idx,
            focused,
            is_empty,
        });
    }

    // If we still have no workspaces, attempt a deep scan over the JSON tree
    if k_workspaces.is_empty() {
        fn collect_workspaces(value: &serde_json::Value, out: &mut Vec<(String, bool, bool)>) {
            match value {
                serde_json::Value::Object(map) => {
                    let ty = map.get("type").and_then(|x| x.as_str()).unwrap_or("");
                    if ty.eq_ignore_ascii_case("workspace") {
                        let name = map
                            .get("name")
                            .and_then(|x| x.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();
                        let focused = map
                            .get("hasFocus")
                            .or_else(|| map.get("focused"))
                            .and_then(|x| x.as_bool())
                            .unwrap_or(false);
                        // Consider non-empty if there is at least one child window
                        let mut is_empty = true;
                        if let Some(children) = map.get("children").and_then(|x| x.as_array()) {
                            for child in children {
                                if child
                                    .get("type")
                                    .and_then(|x| x.as_str())
                                    .map(|s| s.eq_ignore_ascii_case("window"))
                                    .unwrap_or(false)
                                {
                                    is_empty = false;
                                    break;
                                }
                            }
                        }
                        out.push((name, focused, is_empty));
                    }
                    // Recurse object fields
                    for (_, v) in map.iter() {
                        collect_workspaces(v, out);
                    }
                }
                serde_json::Value::Array(arr) => {
                    for v in arr {
                        collect_workspaces(v, out);
                    }
                }
                _ => {}
            }
        }

        let mut collected: Vec<(String, bool, bool)> = Vec::new();
        collect_workspaces(&v, &mut collected);
        for (idx, (name, focused, is_empty)) in collected.into_iter().enumerate() {
            k_workspaces.push(crate::state::Workspace {
                name: if name.is_empty() { (idx + 1).to_string() } else { name },
                index: idx,
                focused,
                is_empty,
            });
        }
    }

    // Ensure workspaces are in a consistent ascending order by numeric name, then lexicographic.
    // Reassign indices after sorting while preserving `focused` and `is_empty` flags.
    if !k_workspaces.is_empty() {
        fn parse_num(name: &str) -> Option<i32> {
            // Extract leading number, e.g., "1", "02", "7"; otherwise None
            let trimmed = name.trim();
            if trimmed.is_empty() { return None; }
            let mut end = 0;
            for (i, ch) in trimmed.char_indices() {
                if ch.is_ascii_digit() { end = i + 1; } else { break; }
            }
            if end == 0 { return None; }
            trimmed[..end].parse::<i32>().ok()
        }

        k_workspaces.sort_by(|a, b| {
            match (parse_num(&a.name), parse_num(&b.name)) {
                (Some(na), Some(nb)) => na.cmp(&nb),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => a.name.cmp(&b.name),
            }
        });

        for (i, ws) in k_workspaces.iter_mut().enumerate() {
            ws.index = i;
        }
    }

    let monitor = crate::state::Monitor {
        name: "Monitor".into(),
        index: 0,
        id: "glazewm-default".into(),
        workspaces: k_workspaces,
        rect: windows::Win32::Foundation::RECT::default(),
    };

    Ok(crate::state::State {
        monitors: vec![monitor],
        ..Default::default()
    })
}

/// Parse GlazeWM JSON and group workspaces by their `parentId` (monitor container),
/// producing multiple monitors where possible. Preserves numeric indices derived from names.
fn parse_state_json_grouped(text: &str) -> anyhow::Result<crate::state::State> {
    let v: serde_json::Value = serde_json::from_str(text)?;

    // Locate workspaces array in typical v3 shapes
    let workspaces: Vec<serde_json::Value> = if let Some(ws) = v.get("data").and_then(|d| d.get("workspaces")).and_then(|x| x.as_array()) {
        ws.clone()
    } else if let Some(ws) = v.get("workspaces").and_then(|x| x.as_array()) {
        ws.clone()
    } else if let Some(arr) = v.as_array() {
        arr.clone()
    } else {
        Vec::new()
    };

    if workspaces.is_empty() {
        anyhow::bail!("no workspaces in JSON")
    }

    use std::collections::BTreeMap;
    let mut groups: BTreeMap<String, Vec<crate::state::Workspace>> = BTreeMap::new();

    fn parse_num(s: &str) -> Option<i32> { s.trim().parse::<i32>().ok() }

    // Top-level focus fallback
    let focused_idx_top = v
        .get("workspaces")
        .and_then(|w| w.get("focused").or_else(|| w.get("focusedIndex")))
        .and_then(|x| x.as_u64())
        .map(|u| u as usize)
        .or_else(|| v.get("focusedWorkspaceIndex").and_then(|x| x.as_u64()).map(|u| u as usize));

    for (idx, w) in workspaces.into_iter().enumerate() {
        let name = if w.is_string() {
            w.as_str().unwrap_or("").to_string()
        } else {
            w.get("name").and_then(|x| x.as_str()).map(|s| s.to_string()).unwrap_or_else(|| (idx + 1).to_string())
        };
        let focused = w
            .get("focused")
            .or_else(|| w.get("isActive")).or_else(|| w.get("hasFocus"))
            .and_then(|x| x.as_bool())
            .unwrap_or_else(|| focused_idx_top.map(|f| f == idx).unwrap_or(false));
        let ws_index = parse_num(&name).map(|n| (n - 1).max(0) as usize).unwrap_or(idx);
        let parent = w.get("parentId").and_then(|x| x.as_str()).unwrap_or("glazewm-default").to_string();

        groups.entry(parent).or_default().push(crate::state::Workspace {
            name,
            index: ws_index,
            focused,
            is_empty: false,
        });
    }

    // Sort each group's workspaces by numeric name, then lexicographic.
    for ws in groups.values_mut() {
        ws.sort_by(|a, b| match (parse_num(&a.name), parse_num(&b.name)) {
            (Some(na), Some(nb)) => na.cmp(&nb),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.name.cmp(&b.name),
        });
        // Do not overwrite indices; keep numeric mapping for CLI focus.
    }

    // Build monitors; leave rect empty (we map by index to taskbars later when rect is empty)
    let mut monitors: Vec<crate::state::Monitor> = Vec::new();
    for (i, (parent_id, workspaces)) in groups.into_iter().enumerate() {
        monitors.push(crate::state::Monitor {
            name: format!("Monitor {}", i + 1),
            index: i,
            id: format!("glazewm-{}", parent_id),
            workspaces,
            rect: windows::Win32::Foundation::RECT::default(),
        });
    }

    Ok(crate::state::State { monitors, ..Default::default() })
}

fn parse_plain_workspaces(text: &str) -> anyhow::Result<crate::state::State> {
    // Parse non-JSON output: assume each non-empty trimmed line is a workspace name.
    // Detect focus markers like "*" or ">" prefix.
    let mut names = Vec::new();
    let mut focused_idx: Option<usize> = None;
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let mut name = line.to_string();
        if let Some(stripped) = name.strip_prefix("*") { // e.g., "*1" or "* Workspace 1"
            focused_idx = Some(names.len());
            name = stripped.trim().to_string();
        } else if let Some(stripped) = name.strip_prefix(">") { // e.g., "> 1"
            focused_idx = Some(names.len());
            name = stripped.trim().to_string();
        }
        names.push(name);
    }

    if names.is_empty() {
        anyhow::bail!("no workspaces in plain text")
    }

    let mut k_workspaces: Vec<crate::state::Workspace> = Vec::new();
    fn parse_num(name: &str) -> Option<i32> { name.trim().parse::<i32>().ok() }
    for (idx, name) in names.into_iter().enumerate() {
        // Preserve numeric indices when possible (1-based -> 0-based)
        let mapped_index = parse_num(name.as_str()).map(|n| (n - 1).max(0) as usize).unwrap_or(idx);
        k_workspaces.push(crate::state::Workspace {
            name,
            index: mapped_index,
            focused: focused_idx.map(|f| f == idx).unwrap_or(idx == 0),
            is_empty: false,
        });
    }

    let monitor = crate::state::Monitor {
        name: "Monitor".into(),
        index: 0,
        id: "glazewm-default".into(),
        workspaces: k_workspaces,
        rect: windows::Win32::Foundation::RECT::default(),
    };

    Ok(crate::state::State {
        monitors: vec![monitor],
        ..Default::default()
    })
}

pub fn change_workspace(monitor_idx: usize, workspace_idx: usize) {
    let _ = monitor_idx;
    let _ = client::focus_workspace(workspace_idx);
}

pub fn listen_for_state(proxy: EventLoopProxy<AppMessage>) {
    // Polling loop since we don't know GlazeWM's subscription API here
    loop {
        if let Ok(state) = read_state() {
            let _ = proxy.send_event(AppMessage::UpdateState(state));
        }
        std::thread::sleep(Duration::from_millis(750));
    }
}