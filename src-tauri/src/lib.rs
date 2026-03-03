use std::process::Command;
use std::sync::Mutex;
use tauri::State;
use rusqlite::{params, Connection};
use serde::{Serialize, Deserialize};
use chrono::Local;
use regex::Regex;
use std::env;
use std::path::PathBuf;
use std::fs;

#[derive(Serialize, Deserialize, Debug)]
struct PingResult {
    success: bool,
    rtt: Option<f64>,
    error: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Session {
    id: i64,
    target: String,
    start_time: String,
    end_time: Option<String>,
    sent: i32,
    received: i32,
    lost: i32,
    min: Option<f64>,
    avg: Option<f64>,
    max: Option<f64>,
}

#[derive(Serialize, Deserialize, Debug)]
struct PingRecord {
    id: i64,
    timestamp: String,
    rtt: Option<f64>,
    success: bool,
}

struct AppState {
    db: Mutex<Connection>,
}

fn get_db_path() -> PathBuf {
    #[cfg(debug_assertions)]
    {
        if let Ok(cwd) = env::current_dir() {
            return cwd.join("ping_history.db");
        }
        return PathBuf::from("ping_history.db");
    }

    #[cfg(not(debug_assertions))]
    {
        if let Ok(mut exe_path) = env::current_exe() {
            exe_path.pop();
            exe_path.push("ping_history.db");
            return exe_path;
        }
        PathBuf::from("ping_history.db")
    }
}

#[tauri::command]
fn start_session(target: String, state: State<AppState>) -> Result<i64, String> {
    let conn = state.db.lock().map_err(|_| "Database lock failed".to_string())?;
    let start_time = Local::now().to_rfc3339();

    conn.execute(
        "INSERT INTO sessions (target, start_time, sent, received, lost) VALUES (?1, ?2, 0, 0, 0)",
        params![target, start_time],
    ).map_err(|e| e.to_string())?;

    let id = conn.last_insert_rowid();
    Ok(id)
}

#[tauri::command]
fn stop_session(id: i64, sent: i32, received: i32, lost: i32, min: Option<f64>, avg: Option<f64>, max: Option<f64>, state: State<AppState>) -> Result<(), String> {
    let conn = state.db.lock().map_err(|_| "Database lock failed".to_string())?;
    let end_time = Local::now().to_rfc3339();

    conn.execute(
        "UPDATE sessions SET end_time = ?1, sent = ?2, received = ?3, lost = ?4, min = ?5, avg = ?6, max = ?7 WHERE id = ?8",
        params![end_time, sent, received, lost, min, avg, max, id],
    ).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
fn get_sessions(state: State<AppState>) -> Result<Vec<Session>, String> {
    let conn = state.db.lock().map_err(|_| "Database lock failed".to_string())?;
    let mut stmt = conn.prepare("SELECT id, target, start_time, end_time, sent, received, lost, min, avg, max FROM sessions ORDER BY id DESC").map_err(|e| e.to_string())?;

    let session_iter = stmt.query_map([], |row| {
        Ok(Session {
            id: row.get(0)?,
            target: row.get(1)?,
            start_time: row.get(2)?,
            end_time: row.get(3)?,
            sent: row.get(4)?,
            received: row.get(5)?,
            lost: row.get(6)?,
            min: row.get(7)?,
            avg: row.get(8)?,
            max: row.get(9)?,
        })
    }).map_err(|e| e.to_string())?;

    let mut sessions = Vec::new();
    for session in session_iter {
        sessions.push(session.map_err(|e| e.to_string())?);
    }

    Ok(sessions)
}

#[tauri::command]
fn delete_session(id: i64, state: State<AppState>) -> Result<(), String> {
    let conn = state.db.lock().map_err(|_| "Database lock failed".to_string())?;

    // Delete the session
    conn.execute("DELETE FROM sessions WHERE id = ?1", params![id]).map_err(|e| e.to_string())?;

    // Delete associated pings
    conn.execute("DELETE FROM pings WHERE session_id = ?1", params![id]).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
fn get_session_pings(id: i64, state: State<AppState>) -> Result<Vec<PingRecord>, String> {
    let conn = state.db.lock().map_err(|_| "Database lock failed".to_string())?;
    let mut stmt = conn.prepare("SELECT id, timestamp, rtt, success FROM pings WHERE session_id = ?1 ORDER BY id ASC").map_err(|e| e.to_string())?;

    let ping_iter = stmt.query_map(params![id], |row| {
        Ok(PingRecord {
            id: row.get(0)?,
            timestamp: row.get(1)?,
            rtt: row.get(2)?,
            success: row.get(3)?,
        })
    }).map_err(|e| e.to_string())?;

    let mut pings = Vec::new();
    for ping in ping_iter {
        pings.push(ping.map_err(|e| e.to_string())?);
    }

    Ok(pings)
}

#[tauri::command]
fn export_logs_to_path(path: String, logs: String) -> Result<(), String> {
    fs::write(path, logs).map_err(|e| e.to_string())
}

#[tauri::command]
fn save_binary_file(path: String, data: Vec<u8>) -> Result<(), String> {
    fs::write(path, data).map_err(|e| e.to_string())
}

#[tauri::command]
fn ping_host(ip: String, session_id: Option<i64>, state: State<AppState>) -> PingResult {
    let cmd_name;
    let args: Vec<String>;

    #[cfg(target_os = "windows")]
    {
        cmd_name = "ping";
        args = vec!["-n".to_string(), "1".to_string(), "-w".to_string(), "1000".to_string(), ip.clone()];
    }

    #[cfg(target_os = "macos")]
    {
        cmd_name = "/sbin/ping";
        args = vec!["-c".to_string(), "1".to_string(), "-W".to_string(), "1000".to_string(), ip.clone()];
    }

    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        cmd_name = "ping";
        args = vec!["-c".to_string(), "1".to_string(), "-W".to_string(), "1".to_string(), ip.clone()];
    }

    let mut cmd = Command::new(cmd_name);
    cmd.args(&args);

    let output = match cmd.output() {
        Ok(o) => o,
        Err(e) => return PingResult { success: false, rtt: None, error: Some(format!("Failed to execute ping: {}", e)) },
    };

    let stdout = String::from_utf8_lossy(&output.stdout);

    let rtt = if output.status.success() {
        parse_ping_output(&stdout)
    } else {
        None
    };

    let success = rtt.is_some();

    if let Ok(conn) = state.db.lock() {
        let timestamp = Local::now().to_rfc3339();
        let _ = conn.execute(
            "INSERT INTO pings (target, timestamp, success, rtt, session_id) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![ip, timestamp, success, rtt, session_id],
        );
    }

    PingResult {
        success,
        rtt,
        error: if success { None } else { Some("Request timed out".to_string()) },
    }
}

fn parse_ping_output(output: &str) -> Option<f64> {
    let re = Regex::new(r"time[=<]([\d\.]+)").ok()?;
    if let Some(caps) = re.captures(output) {
        if let Some(m) = caps.get(1) {
            return m.as_str().parse::<f64>().ok();
        }
    }
    None
}

#[tauri::command]
fn get_stats(state: State<AppState>) -> Result<serde_json::Value, String> {
    let conn = state.db.lock().map_err(|_| "Database lock failed".to_string())?;

    let mut stmt = conn.prepare("SELECT count(*), avg(rtt), min(rtt), max(rtt) FROM pings WHERE success = 1").map_err(|e| e.to_string())?;

    let mut rows = stmt.query([]).map_err(|e| e.to_string())?;

    if let Some(row) = rows.next().map_err(|e| e.to_string())? {
        let count: i64 = row.get(0).unwrap_or(0);
        let avg: Option<f64> = row.get(1).ok();
        let min: Option<f64> = row.get(2).ok();
        let max: Option<f64> = row.get(3).ok();

        return Ok(serde_json::json!({
            "count": count,
            "avg": avg,
            "min": min,
            "max": max
        }));
    }

    Ok(serde_json::json!({}))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let db_path = get_db_path();
    println!("--------------------------------------------------");
    println!("DATABASE PATH: {:?}", db_path);
    println!("--------------------------------------------------");

    let conn = Connection::open(&db_path).unwrap_or_else(|e| {
        eprintln!("Failed to open database file: {}. Falling back to in-memory.", e);
        Connection::open_in_memory().expect("Failed to create in-memory database")
    });

    // Update pings table to include session_id
    // We use CREATE TABLE IF NOT EXISTS, but if the table already exists without session_id, we need to handle migration.
    // For simplicity in this dev phase, we'll just try to add the column and ignore error if it exists.
    let _ = conn.execute("ALTER TABLE pings ADD COLUMN session_id INTEGER", []);

    let _ = conn.execute(
        "CREATE TABLE IF NOT EXISTS pings (
            id INTEGER PRIMARY KEY,
            target TEXT NOT NULL,
            timestamp TEXT NOT NULL,
            success BOOLEAN NOT NULL,
            rtt REAL,
            session_id INTEGER
        )",
        [],
    );

    let _ = conn.execute(
        "CREATE TABLE IF NOT EXISTS sessions (
            id INTEGER PRIMARY KEY,
            target TEXT NOT NULL,
            start_time TEXT NOT NULL,
            end_time TEXT,
            sent INTEGER,
            received INTEGER,
            lost INTEGER,
            min REAL,
            avg REAL,
            max REAL
        )",
        [],
    );

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(AppState { db: Mutex::new(conn) })
        .invoke_handler(tauri::generate_handler![ping_host, get_stats, start_session, stop_session, get_sessions, delete_session, get_session_pings, export_logs_to_path, save_binary_file])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
