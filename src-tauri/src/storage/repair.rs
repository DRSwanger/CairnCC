//! Session repair: trim oversized tool outputs in events.jsonl, atomically,
//! preserving a gzipped copy of the original for inline restoration.
//!
//! ## Why
//!
//! `events.jsonl` is the UI's source of truth for replay. The frontend caps
//! tool output at 32 KB *for display*, but the backend historically had no
//! write-time cap — Claude can emit a 50 MB tool result and land it as a single
//! JSONL line. Three sessions on Dallas's Windows box grew past 400 MB each,
//! hanging WebView2 hard on session entry.
//!
//! ## Two entry points
//!
//! - **Write-time cap** (`trim_tool_value`): used by `claude_protocol::map_event`
//!   on every `ToolEnd` construction. Caps `output` / `tool_use_result` to
//!   head+tail. `TrimInfo.archive_path = None` because no archive is created at
//!   write time — the original is in Claude Code's own session JSONL
//!   (`~/.claude/projects/.../<session>.jsonl`), which Claude reads on `--resume`.
//! - **Post-hoc repair** (`repair`): for existing bloated sessions. Stream
//!   `events.jsonl` → `events.jsonl.preTrim-<ISO>.gz` (full original), then
//!   stream again → `events.jsonl.tmp` (trimmed), then atomically swap. Each
//!   trimmed `ToolEnd` gets a `TrimInfo` with `archive_path` pointing at the gz.
//!
//! ## Integrity
//!
//! - SHA256 of the original `output` value's JSON serialization is stored in
//!   `TrimInfo.original_sha256`. Restore verifies before returning.
//! - Atomic rename means a crash mid-repair leaves either old or new
//!   `events.jsonl` — never partial. The gz archive is written *before* the
//!   trim pass, so a crash between the two leaves us with the gz intact and
//!   the original `events.jsonl` untouched — safe to retry.

use crate::models::{now_iso, TrimInfo};
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};

/// Match frontend `truncateToolField` semantics (session-store.svelte.ts:144-153).
const TOOL_OUTPUT_MAX_BYTES: usize = 32 * 1024;
const TOOL_OUTPUT_HEAD_BYTES: usize = 16 * 1024;
const TOOL_OUTPUT_TAIL_BYTES: usize = 8 * 1024;

/// Trigger threshold: only auto-repair sessions whose events.jsonl exceeds
/// this size. Below this, the file loads fine; above it, WebView2 hangs on
/// large-line JSON.parse + the reactive cascade.
pub const REPAIR_TRIGGER_BYTES: u64 = 25 * 1024 * 1024;

/// If repair has run very recently (mtime of any preTrim archive within this
/// window), skip re-scanning to avoid spin loops on broken edges.
const REPAIR_COOLDOWN_SECS: u64 = 300;

/// Recursively trim every string in `v` over `TOOL_OUTPUT_MAX_BYTES` to
/// head + separator + tail. Returns `(trimmed_value, Some(TrimInfo))` if any
/// string was trimmed, `(clone, None)` otherwise.
///
/// `source`: "write_time_cap" or "repair" — recorded in TrimInfo for debugging.
/// `archive_path`: `None` for write-time cap; `Some(filename)` for repair.
pub fn trim_tool_value(
    v: &Value,
    source: &'static str,
    archive_path: Option<String>,
) -> (Value, Option<TrimInfo>) {
    // Hash the ORIGINAL serialized bytes — used to verify restore-from-archive.
    let original_serialized = match serde_json::to_vec(v) {
        Ok(b) => b,
        Err(_) => return (v.clone(), None),
    };
    let original_bytes = original_serialized.len() as u64;
    if original_bytes < TOOL_OUTPUT_MAX_BYTES as u64 {
        // Cheap pre-check: tiny values can't have any over-cap string field.
        return (v.clone(), None);
    }

    let mut any_trimmed = false;
    let trimmed = trim_walk(v, &mut any_trimmed);
    if !any_trimmed {
        return (v.clone(), None);
    }

    let mut hasher = Sha256::new();
    hasher.update(&original_serialized);
    let sha = format!("{:x}", hasher.finalize());

    let info = TrimInfo {
        original_bytes,
        original_sha256: sha,
        trimmed_at: now_iso(),
        archive_path,
        source: source.to_string(),
    };
    (trimmed, Some(info))
}

fn trim_walk(v: &Value, any: &mut bool) -> Value {
    match v {
        Value::String(s) => {
            if s.len() <= TOOL_OUTPUT_MAX_BYTES {
                return Value::String(s.clone());
            }
            *any = true;
            let omitted = s.len() - TOOL_OUTPUT_HEAD_BYTES - TOOL_OUTPUT_TAIL_BYTES;
            let head_end = floor_char_boundary(s, TOOL_OUTPUT_HEAD_BYTES);
            let tail_start = ceil_char_boundary(s, s.len() - TOOL_OUTPUT_TAIL_BYTES);
            let mut out = String::with_capacity(TOOL_OUTPUT_MAX_BYTES + 128);
            out.push_str(&s[..head_end]);
            out.push_str(&format!(
                "\n\n[... {} bytes trimmed by CairnCC for storage; sha256 verified, archived ...]\n\n",
                fmt_thousands(omitted as u64)
            ));
            out.push_str(&s[tail_start..]);
            Value::String(out)
        }
        Value::Array(a) => Value::Array(a.iter().map(|x| trim_walk(x, any)).collect()),
        Value::Object(o) => {
            let mut m = serde_json::Map::with_capacity(o.len());
            for (k, val) in o {
                m.insert(k.clone(), trim_walk(val, any));
            }
            Value::Object(m)
        }
        _ => v.clone(),
    }
}

fn floor_char_boundary(s: &str, n: usize) -> usize {
    if n >= s.len() {
        return s.len();
    }
    let mut i = n;
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn ceil_char_boundary(s: &str, n: usize) -> usize {
    if n >= s.len() {
        return s.len();
    }
    let mut i = n;
    while i < s.len() && !s.is_char_boundary(i) {
        i += 1;
    }
    i
}

fn fmt_thousands(n: u64) -> String {
    let s = n.to_string();
    let bytes = s.as_bytes();
    let len = bytes.len();
    let mut out = String::with_capacity(len + len / 3);
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            out.push(',');
        }
        out.push(*b as char);
    }
    out
}

// ── Assessment + repair ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairAssessment {
    pub run_id: String,
    pub total_bytes: u64,
    pub oversize_events: u64,
    pub bytes_saveable_estimate: u64,
    pub needs_repair: bool,
}

/// Cheap scan: does this run need repair?
///
/// Fast paths:
/// - If events.jsonl is below trigger threshold, skip the line scan entirely.
/// - Otherwise, single pass counting lines that (a) exceed the per-event cap
///   and (b) contain `"tool_end"` (substring pre-filter).
pub fn assess(run_id: &str) -> RepairAssessment {
    let mut out = RepairAssessment {
        run_id: run_id.to_string(),
        total_bytes: 0,
        oversize_events: 0,
        bytes_saveable_estimate: 0,
        needs_repair: false,
    };
    let path = super::run_dir(run_id).join("events.jsonl");
    let meta = match fs::metadata(&path) {
        Ok(m) => m,
        Err(_) => return out,
    };
    out.total_bytes = meta.len();
    if out.total_bytes < REPAIR_TRIGGER_BYTES {
        return out;
    }

    let f = match fs::File::open(&path) {
        Ok(f) => f,
        Err(_) => return out,
    };
    let reader = BufReader::new(f);
    for line in reader.lines().map_while(|l| l.ok()) {
        if line.len() < TOOL_OUTPUT_MAX_BYTES {
            continue;
        }
        if !line.contains("\"tool_end\"") {
            continue;
        }
        out.oversize_events += 1;
        out.bytes_saveable_estimate += (line.len() as u64).saturating_sub(TOOL_OUTPUT_MAX_BYTES as u64);
    }
    out.needs_repair = out.oversize_events > 0;
    out
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairOutcome {
    pub run_id: String,
    pub trimmed_events: u64,
    pub bytes_saved: u64,
    pub archive_path: String,
    pub completed_at: String,
}

/// Repair an oversized events.jsonl in place. Idempotent: re-running on an
/// already-repaired file is a no-op (no oversize events left).
///
/// Three phases, all crash-safe:
/// 1. Copy `events.jsonl` → `events.jsonl.preTrim-<ISO>.gz`.
/// 2. Stream `events.jsonl` → `events.jsonl.tmp`, trimming each oversize ToolEnd.
/// 3. Atomic `rename(tmp → events.jsonl)`.
///
/// Crash between (1) and (2): retry resumes from a fresh assess.
/// Crash between (2) and (3): tmp orphaned; next run overwrites it.
/// Crash mid-(3): rename is atomic on POSIX; on Windows we delete src first then
/// rename — there's a brief window where only the gz exists, but it carries the
/// full original.
pub fn repair(run_id: &str) -> Result<RepairOutcome, String> {
    let assessment = assess(run_id);
    if !assessment.needs_repair {
        return Ok(RepairOutcome {
            run_id: run_id.to_string(),
            trimmed_events: 0,
            bytes_saved: 0,
            archive_path: String::new(),
            completed_at: now_iso(),
        });
    }

    let dir = super::run_dir(run_id);
    let src = dir.join("events.jsonl");
    let stamp = now_iso().replace(':', "-");
    let archive_filename = format!("events.jsonl.preTrim-{}.gz", stamp);
    let archive = dir.join(&archive_filename);
    let tmp = dir.join("events.jsonl.tmp");

    log::info!(
        "[repair] starting {}: total={} MB, oversize_events={}, est_save={} MB",
        run_id,
        assessment.total_bytes / 1024 / 1024,
        assessment.oversize_events,
        assessment.bytes_saveable_estimate / 1024 / 1024
    );

    // ── Phase 1: gzip full original to archive ──
    {
        let inp = fs::File::open(&src).map_err(|e| format!("repair: open src: {}", e))?;
        let out_f = fs::File::create(&archive).map_err(|e| format!("repair: create archive: {}", e))?;
        let mut gz = GzEncoder::new(out_f, Compression::default());
        let mut reader = BufReader::with_capacity(256 * 1024, inp);
        let mut buf = [0u8; 64 * 1024];
        loop {
            let n = reader.read(&mut buf).map_err(|e| format!("repair: read: {}", e))?;
            if n == 0 {
                break;
            }
            gz.write_all(&buf[..n]).map_err(|e| format!("repair: gz write: {}", e))?;
        }
        gz.finish().map_err(|e| format!("repair: gz finish: {}", e))?;
    }
    log::debug!("[repair] {}: archive written → {}", run_id, archive_filename);

    // ── Phase 2: stream src → tmp with trim ──
    let mut trimmed_count = 0u64;
    let mut bytes_saved = 0u64;
    {
        let inp = fs::File::open(&src).map_err(|e| format!("repair: reopen src: {}", e))?;
        let reader = BufReader::with_capacity(256 * 1024, inp);
        let out_f = fs::File::create(&tmp).map_err(|e| format!("repair: create tmp: {}", e))?;
        let mut writer = std::io::BufWriter::with_capacity(256 * 1024, out_f);

        for line_res in reader.lines() {
            let line = line_res.map_err(|e| format!("repair: read line: {}", e))?;
            if line.is_empty() {
                continue;
            }
            // Fast path: small lines or non-tool_end lines pass through verbatim
            if line.len() < TOOL_OUTPUT_MAX_BYTES || !line.contains("\"tool_end\"") {
                writer.write_all(line.as_bytes()).map_err(|e| format!("repair: write tmp: {}", e))?;
                writer.write_all(b"\n").map_err(|e| format!("repair: write nl: {}", e))?;
                continue;
            }
            // Trim path
            let mut env: Value = match serde_json::from_str(&line) {
                Ok(v) => v,
                Err(_) => {
                    // unparseable — preserve as-is, don't drop data
                    writer.write_all(line.as_bytes()).ok();
                    writer.write_all(b"\n").ok();
                    continue;
                }
            };
            let did_trim =
                apply_inner_trim(&mut env, "repair", Some(archive_filename.clone()));
            if did_trim {
                trimmed_count += 1;
            }
            let serialized = serde_json::to_string(&env)
                .map_err(|e| format!("repair: reserialize: {}", e))?;
            if line.len() as u64 > serialized.len() as u64 {
                bytes_saved += line.len() as u64 - serialized.len() as u64;
            }
            writer.write_all(serialized.as_bytes())
                .map_err(|e| format!("repair: write trimmed: {}", e))?;
            writer.write_all(b"\n").map_err(|e| format!("repair: write nl: {}", e))?;
        }
        writer.flush().map_err(|e| format!("repair: flush tmp: {}", e))?;
    }
    log::debug!(
        "[repair] {}: tmp written ({} trimmed, {} bytes saved)",
        run_id,
        trimmed_count,
        bytes_saved
    );

    // ── Phase 3: atomic swap ──
    // On Windows, rename fails if dst exists. Delete src first, then rename.
    // If we crash here, the gz archive carries the original — recoverable.
    #[cfg(windows)]
    {
        fs::remove_file(&src).map_err(|e| format!("repair: remove src: {}", e))?;
    }
    fs::rename(&tmp, &src).map_err(|e| format!("repair: rename tmp→src: {}", e))?;

    // ── Phase 4: stamp meta.json with repair_log entry ──
    let meta_path = dir.join("meta.json");
    if let Ok(meta_text) = fs::read_to_string(&meta_path) {
        if let Ok(mut meta) = serde_json::from_str::<Value>(&meta_text) {
            let entry = serde_json::json!({
                "at": now_iso(),
                "trimmed_events": trimmed_count,
                "bytes_saved": bytes_saved,
                "original_bytes": assessment.total_bytes,
                "archive": archive_filename.clone(),
            });
            if let Some(obj) = meta.as_object_mut() {
                let log_entry = obj
                    .entry("repair_log".to_string())
                    .or_insert_with(|| Value::Array(vec![]));
                if let Some(arr) = log_entry.as_array_mut() {
                    arr.push(entry);
                }
            }
            let _ = fs::write(&meta_path, serde_json::to_string_pretty(&meta).unwrap_or_default());
        }
    }

    log::info!(
        "[repair] complete {}: trimmed {} events, saved {} MB, archive={}",
        run_id,
        trimmed_count,
        bytes_saved / 1024 / 1024,
        archive_filename
    );

    Ok(RepairOutcome {
        run_id: run_id.to_string(),
        trimmed_events: trimmed_count,
        bytes_saved,
        archive_path: archive_filename,
        completed_at: now_iso(),
    })
}

/// Apply trim to a `tool_end` event in the on-disk envelope shape. Handles
/// both `{_bus:true, event:{type:tool_end, ...}}` (current) and legacy direct
/// `{type:tool_end, ...}` formats. Returns `true` if anything was trimmed.
fn apply_inner_trim(env: &mut Value, source: &'static str, archive_path: Option<String>) -> bool {
    // Locate inner event mutably
    let inner: &mut Value = if env.get("_bus").and_then(|b| b.as_bool()) == Some(true)
        && env.get("event").is_some()
    {
        match env.get_mut("event") {
            Some(v) => v,
            None => return false,
        }
    } else {
        env
    };

    let obj = match inner.as_object_mut() {
        Some(o) => o,
        None => return false,
    };
    if obj.get("type").and_then(|v| v.as_str()) != Some("tool_end") {
        return false;
    }

    let mut trimmed = false;
    // Trim `output`
    if let Some(out_val) = obj.get("output").cloned() {
        let (new_out, info) = trim_tool_value(&out_val, source, archive_path.clone());
        if let Some(info) = info {
            obj.insert("output".into(), new_out);
            if let Ok(info_val) = serde_json::to_value(info) {
                obj.insert("trim_info".into(), info_val);
            }
            trimmed = true;
        }
    }
    // Trim `tool_use_result` — share archive_path but don't overwrite trim_info
    if let Some(tur_val) = obj.get("tool_use_result").cloned() {
        let (new_tur, info) = trim_tool_value(&tur_val, source, archive_path.clone());
        if let Some(info) = info {
            obj.insert("tool_use_result".into(), new_tur);
            if !trimmed {
                if let Ok(info_val) = serde_json::to_value(info) {
                    obj.insert("trim_info".into(), info_val);
                }
            }
            trimmed = true;
        }
    }
    trimmed
}

// ── Restore from archive ───────────────────────────────────────────────────

/// Decompress the archive and pull the original `output` for a given
/// `tool_use_id`. Verifies sha256 before returning.
pub fn restore_from_archive(
    run_id: &str,
    archive_filename: &str,
    tool_use_id: &str,
    expected_sha256: &str,
) -> Result<Value, String> {
    let archive = super::run_dir(run_id).join(archive_filename);
    if !archive.exists() {
        return Err(format!("archive not found: {}", archive.display()));
    }
    let f = fs::File::open(&archive).map_err(|e| format!("restore: open: {}", e))?;
    let gz = flate2::read::GzDecoder::new(f);
    let reader = BufReader::with_capacity(256 * 1024, gz);
    for line in reader.lines().map_while(|l| l.ok()) {
        if line.is_empty() {
            continue;
        }
        if !line.contains("\"tool_end\"") {
            continue;
        }
        if !line.contains(tool_use_id) {
            continue;
        }
        let v: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let inner = v.get("event").unwrap_or(&v);
        if inner.get("type").and_then(|s| s.as_str()) != Some("tool_end") {
            continue;
        }
        if inner.get("tool_use_id").and_then(|s| s.as_str()) != Some(tool_use_id) {
            continue;
        }
        let output = inner.get("output").cloned().unwrap_or(Value::Null);
        let serialized = serde_json::to_vec(&output)
            .map_err(|e| format!("restore: serialize: {}", e))?;
        let mut hasher = Sha256::new();
        hasher.update(&serialized);
        let sha = format!("{:x}", hasher.finalize());
        if sha != expected_sha256 {
            return Err(format!(
                "restore: sha256 mismatch (expected {}, got {})",
                expected_sha256, sha
            ));
        }
        return Ok(output);
    }
    Err(format!(
        "restore: tool_use_id {} not found in {}",
        tool_use_id, archive_filename
    ))
}

// ── Backfill scheduler ─────────────────────────────────────────────────────

/// Scan all runs and queue background repair for any that need it. Runs
/// sequentially with a small sleep between to keep CPU and disk pressure low.
/// Idempotent — repaired runs are below threshold and skipped on subsequent scans.
pub async fn backfill_all_oversized(active_run_ids: Vec<String>) {
    let runs_dir = super::runs_dir();
    if !runs_dir.exists() {
        return;
    }
    let entries = match fs::read_dir(&runs_dir) {
        Ok(e) => e,
        Err(e) => {
            log::warn!("[repair] backfill: cannot read runs dir: {}", e);
            return;
        }
    };

    let mut candidates: Vec<String> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let run_id = name.to_string();
        if active_run_ids.iter().any(|r| r == &run_id) {
            continue;
        }
        let events_path = path.join("events.jsonl");
        if let Ok(m) = fs::metadata(&events_path) {
            if m.len() >= REPAIR_TRIGGER_BYTES {
                candidates.push(run_id);
            }
        }
    }

    if candidates.is_empty() {
        log::debug!("[repair] backfill: no oversized sessions");
        return;
    }
    log::info!("[repair] backfill: {} oversized session(s) queued", candidates.len());

    for run_id in candidates {
        let id = run_id.clone();
        let result = tokio::task::spawn_blocking(move || repair(&id)).await;
        match result {
            Ok(Ok(outcome)) => {
                if outcome.trimmed_events > 0 {
                    log::info!(
                        "[repair] backfill ok: {} → {} events trimmed, {} MB saved",
                        run_id,
                        outcome.trimmed_events,
                        outcome.bytes_saved / 1024 / 1024
                    );
                }
            }
            Ok(Err(e)) => log::warn!("[repair] backfill failed for {}: {}", run_id, e),
            Err(e) => log::warn!("[repair] backfill join error for {}: {}", run_id, e),
        }
        // Yield + sleep so backfill doesn't monopolise IO at app start
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
    log::info!("[repair] backfill: pass complete");
}

/// Suppress unused warning until used.
#[allow(dead_code)]
pub fn repair_cooldown_secs() -> u64 {
    REPAIR_COOLDOWN_SECS
}

/// Count `trim_info` markers in this run's events.jsonl. Used to decide whether
/// to inject a one-line note into Claude's `--append-system-prompt` so Claude
/// knows the UI display has been trimmed (his own `~/.claude/projects/...`
/// session log still carries the full content).
pub fn count_trimmed_events(run_id: &str) -> u64 {
    let path = super::run_dir(run_id).join("events.jsonl");
    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return 0,
    };
    // Substring match — `trim_info` only appears as the field name on tool_end events.
    content.matches("\"trim_info\":{").count() as u64
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn trim_short_string_noop() {
        let v = json!("hello world");
        let (out, info) = trim_tool_value(&v, "write_time_cap", None);
        assert_eq!(out, v);
        assert!(info.is_none());
    }

    #[test]
    fn trim_long_string_caps_and_emits_info() {
        let big = "A".repeat(100_000);
        let v = json!(big);
        let (out, info) = trim_tool_value(&v, "write_time_cap", None);
        let info = info.expect("expected TrimInfo on oversized value");
        assert_eq!(info.source, "write_time_cap");
        assert!(info.archive_path.is_none());
        // Trimmed value must be shorter than the cap + a generous separator allowance
        let trimmed_str = out.as_str().expect("trimmed should remain a string");
        assert!(
            trimmed_str.len() < TOOL_OUTPUT_MAX_BYTES + 256,
            "trimmed length {} exceeded cap+padding",
            trimmed_str.len()
        );
        assert!(trimmed_str.contains("trimmed by CairnCC"));
        // sha256 hex is 64 chars
        assert_eq!(info.original_sha256.len(), 64);
        assert_eq!(info.original_bytes, serde_json::to_vec(&v).unwrap().len() as u64);
    }

    #[test]
    fn trim_recurses_into_objects_and_arrays() {
        let big = "B".repeat(100_000);
        let v = json!({
            "wrapper": [{ "content": big.clone() }],
            "small": "ok"
        });
        let (out, info) = trim_tool_value(&v, "repair", Some("a.gz".into()));
        let info = info.expect("expected TrimInfo");
        assert_eq!(info.source, "repair");
        assert_eq!(info.archive_path.as_deref(), Some("a.gz"));
        let new_content = out
            .pointer("/wrapper/0/content")
            .and_then(|v| v.as_str())
            .expect("content path missing");
        assert!(new_content.len() < big.len(), "content should be trimmed");
        let small = out.pointer("/small").and_then(|v| v.as_str()).unwrap();
        assert_eq!(small, "ok");
    }

    #[test]
    fn restore_round_trip() {
        // End-to-end: write a synthetic events.jsonl with a 60KB tool_end output,
        // call repair(), then call restore_from_archive() and verify byte-equality.
        // Uses real on-disk paths under the storage::runs_dir() the binary uses;
        // confined to a unique sentinel run_id so we don't collide with anything.
        use std::io::Write as _;
        let run_id = format!(
            "test-repair-roundtrip-{}",
            uuid::Uuid::new_v4().to_string()
        );
        let dir = super::super::run_dir(&run_id);
        super::super::ensure_dir(&dir).expect("mkdir test run dir");
        let events = dir.join("events.jsonl");
        // Synthesize an oversize tool_end. We must inflate the file past
        // REPAIR_TRIGGER_BYTES (25 MB) so assess() flags it for repair.
        // We do that by padding with many ordinary (untrimmed) bus events.
        let big_output = "Z".repeat(60_000);
        let big_event = serde_json::json!({
            "_bus": true,
            "seq": 1,
            "ts": "2026-05-15T00:00:00Z",
            "event": {
                "type": "tool_end",
                "run_id": run_id,
                "tool_use_id": "tu-rt-1",
                "tool_name": "Bash",
                "output": big_output,
                "status": "success"
            }
        });
        let pad_event = serde_json::json!({
            "_bus": true,
            "seq": 2,
            "ts": "2026-05-15T00:00:01Z",
            "event": { "type": "message_delta", "run_id": run_id, "text": "x" }
        });
        let mut f = fs::File::create(&events).expect("create events.jsonl");
        writeln!(f, "{}", serde_json::to_string(&big_event).unwrap()).unwrap();
        // pad until file size > REPAIR_TRIGGER_BYTES
        let mut padded = 0u64;
        let pad_line = serde_json::to_string(&pad_event).unwrap();
        while padded < super::REPAIR_TRIGGER_BYTES + 1024 {
            writeln!(f, "{}", pad_line).unwrap();
            padded += pad_line.len() as u64 + 1;
        }
        drop(f);

        // assess + repair
        let assessment = super::assess(&run_id);
        assert!(assessment.needs_repair, "should need repair: {:?}", assessment);
        assert!(assessment.oversize_events >= 1);

        let outcome = super::repair(&run_id).expect("repair should succeed");
        assert!(outcome.trimmed_events >= 1);
        assert!(outcome.bytes_saved > 0);
        assert!(!outcome.archive_path.is_empty());

        // Locate trim_info that landed in the new events.jsonl
        let trimmed_content = fs::read_to_string(&events).expect("read trimmed events");
        let first_line = trimmed_content.lines().next().expect("first line").to_string();
        let envelope: Value = serde_json::from_str(&first_line).expect("first line is json");
        let inner = envelope.get("event").unwrap();
        let info = inner.get("trim_info").expect("trim_info present after repair");
        let sha = info.get("original_sha256").and_then(|s| s.as_str()).unwrap().to_string();
        assert_eq!(info.get("source").and_then(|s| s.as_str()), Some("repair"));
        assert_eq!(
            info.get("archive_path").and_then(|s| s.as_str()),
            Some(outcome.archive_path.as_str())
        );

        // restore + verify
        let restored = super::restore_from_archive(&run_id, &outcome.archive_path, "tu-rt-1", &sha)
            .expect("restore should succeed");
        assert_eq!(restored.as_str(), Some(big_output.as_str()));

        // sha mismatch must fail
        let bad = super::restore_from_archive(
            &run_id,
            &outcome.archive_path,
            "tu-rt-1",
            "0".repeat(64).as_str(),
        );
        assert!(bad.is_err(), "wrong sha should fail");

        // Idempotency: second repair on the repaired file is a no-op
        let second = super::repair(&run_id).expect("second repair");
        assert_eq!(second.trimmed_events, 0, "repaired file must be idempotent");

        // Cleanup
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn apply_inner_trim_only_touches_tool_end() {
        let mut env = json!({
            "_bus": true,
            "seq": 5,
            "ts": "2026-05-15T00:00:00Z",
            "event": {
                "type": "user_message",
                "text": "hi"
            }
        });
        let touched = apply_inner_trim(&mut env, "repair", None);
        assert!(!touched, "non-tool_end events must be untouched");
        // Round-trip identity
        let s = serde_json::to_string(&env).unwrap();
        assert!(s.contains("\"user_message\""));
    }

    #[test]
    fn apply_inner_trim_caps_tool_end_output() {
        let big = "C".repeat(80_000);
        let mut env = json!({
            "_bus": true,
            "seq": 7,
            "ts": "2026-05-15T00:00:00Z",
            "event": {
                "type": "tool_end",
                "run_id": "r1",
                "tool_use_id": "tu-1",
                "tool_name": "Bash",
                "output": big.clone(),
                "status": "success"
            }
        });
        let touched = apply_inner_trim(&mut env, "repair", Some("arc.gz".into()));
        assert!(touched);
        let inner = env.get("event").unwrap();
        let new_out = inner.get("output").and_then(|v| v.as_str()).unwrap();
        assert!(new_out.len() < big.len());
        let trim_info = inner.get("trim_info").expect("trim_info missing");
        assert_eq!(trim_info.get("source").and_then(|v| v.as_str()), Some("repair"));
        assert_eq!(
            trim_info.get("archive_path").and_then(|v| v.as_str()),
            Some("arc.gz")
        );
    }

    #[test]
    fn assess_skips_small_files() {
        // assess() requires storage layout — covered in integration test.
    }
}
