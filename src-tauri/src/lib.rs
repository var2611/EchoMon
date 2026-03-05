use std::sync::Mutex;
use tauri::{State, Manager};
use rusqlite::{params, Connection};
use serde::{Serialize, Deserialize};
use chrono::Utc;
use std::env;
use std::path::PathBuf;
use std::fs;
use std::net::{IpAddr, Ipv4Addr, ToSocketAddrs};
use std::str::FromStr;
use std::time::Duration;
use surge_ping::{Client, Config, PingIdentifier, PingSequence, IcmpPacket};
use rand::random;
use tauri_plugin_dialog::DialogExt;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct PingResult {
    success: bool,
    rtt: Option<f64>,
    error: Option<String>,
    resolved_ip: Option<String>,
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
            if cwd.ends_with("src-tauri") {
                return cwd.parent().unwrap().join("ping_history.db");
            }
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
    let start_time = Utc::now().to_rfc3339();

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
    let end_time = Utc::now().to_rfc3339();

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

    conn.execute("DELETE FROM sessions WHERE id = ?1", params![id]).map_err(|e| e.to_string())?;
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
async fn ping_host(ip: String, session_id: Option<i64>, state: State<'_, AppState>) -> Result<PingResult, String> {
    // Try to parse as IP first, if fails, try to resolve hostname
    let (addr, resolved_ip) = if let Ok(addr) = IpAddr::from_str(&ip) {
        (addr, None)
    } else {
        // Resolve hostname
        match (ip.as_str(), 0).to_socket_addrs() {
            Ok(mut addrs) => {
                // Prefer IPv4 for now as surge-ping configuration might default to V4 socket
                // or we can try to find the first one.
                let mut chosen_addr = None;

                // Try to find an IPv4 address first
                for a in addrs {
                    if a.ip().is_ipv4() {
                        chosen_addr = Some(a.ip());
                        break;
                    }
                }

                // If no IPv4, take whatever (IPv6) - but surge-ping client needs to be configured for it?
                // The current Client::new(&Config::default()) creates a V4 socket by default on some platforms/versions?
                // Actually Config::default() usually supports both if the OS does.
                // But let's see.

                if let Some(socket_addr) = chosen_addr {
                    (socket_addr, Some(socket_addr.to_string()))
                } else {
                    // Re-resolve to get the iterator again or just error if we consumed it and found nothing?
                    // Let's just re-resolve to get the first one if no IPv4 found.
                    if let Ok(mut addrs_retry) = (ip.as_str(), 0).to_socket_addrs() {
                         if let Some(socket_addr) = addrs_retry.next() {
                             (socket_addr.ip(), Some(socket_addr.ip().to_string()))
                         } else {
                             return Ok(PingResult { success: false, rtt: None, error: Some("Could not resolve hostname".to_string()), resolved_ip: None });
                         }
                    } else {
                        return Ok(PingResult { success: false, rtt: None, error: Some("Could not resolve hostname".to_string()), resolved_ip: None });
                    }
                }
            },
            Err(e) => return Ok(PingResult { success: false, rtt: None, error: Some(format!("DNS resolution failed: {}", e)), resolved_ip: None }),
        }
    };

    let client = Client::new(&Config::default()).map_err(|e| e.to_string())?;
    let mut pinger = client.pinger(addr, PingIdentifier(random())).await;
    pinger.timeout(Duration::from_secs(1));

    let result: Result<(IcmpPacket, Duration), surge_ping::SurgeError> = pinger.ping(PingSequence(0), &[]).await;

    let (success, rtt, error) = match result {
        Ok((_packet, rtt)) => (true, Some(rtt.as_micros() as f64 / 1000.0), None),
        Err(e) => (false, None, Some(e.to_string())),
    };

    if let Ok(conn) = state.db.lock() {
        let timestamp = Utc::now().to_rfc3339();
        let _ = conn.execute(
            "INSERT INTO pings (target, timestamp, success, rtt, session_id) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![ip, timestamp, success, rtt, session_id],
        );
    }

    Ok(PingResult { success, rtt, error, resolved_ip })
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

        // Calculate Jitter
        let mut jitter: Option<f64> = None;
        if count > 1 {
             let mut stmt_jitter = conn.prepare("SELECT rtt FROM pings WHERE success = 1 ORDER BY id ASC").map_err(|e| e.to_string())?;
             let rtt_iter = stmt_jitter.query_map([], |row| row.get::<_, f64>(0)).map_err(|e| e.to_string())?;

             let mut rtts = Vec::new();
             for rtt in rtt_iter {
                 if let Ok(val) = rtt {
                     rtts.push(val);
                 }
             }

             if rtts.len() > 1 {
                 let sum_diff: f64 = rtts.windows(2).map(|w| (w[1] - w[0]).abs()).sum();
                 jitter = Some(sum_diff / (rtts.len() - 1) as f64);
             }
        }

        return Ok(serde_json::json!({
            "count": count,
            "avg": avg,
            "min": min,
            "max": max,
            "jitter": jitter
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
        .setup(|app| {
            // On Windows, check for admin rights.
            #[cfg(windows)]
            {
                if !is_admin() {
                    let handle = app.handle().clone();

                    if let Some(window) = handle.get_webview_window("main") {
                         app.dialog()
                            .message("EchoMon needs to be run as an administrator to send ICMP packets. Please restart the application with administrator rights.")
                            .title("Administrator Privileges Required")
                            .parent(&window)
                            .show(|_| {});
                    } else {
                        eprintln!("Administrator Privileges Required: EchoMon needs to be run as an administrator.");
                    }
                }
            }
            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(AppState { db: Mutex::new(conn) })
        .invoke_handler(tauri::generate_handler![ping_host, get_stats, start_session, stop_session, get_sessions, delete_session, get_session_pings, export_logs_to_path, save_binary_file])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(windows)]
fn is_admin() -> bool {
    use windows::Win32::System::Threading::{OpenProcessToken, GetCurrentProcess};
    use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_QUERY, TOKEN_ELEVATION};
    use std::mem;

    let mut token = Default::default();
    unsafe {
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }
    }

    let mut elevation: TOKEN_ELEVATION = unsafe { mem::zeroed() };
    let mut size = mem::size_of::<TOKEN_ELEVATION>() as u32;
    unsafe {
        if GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut _),
            size,
            &mut size,
        ).is_err() {
            return false;
        }
    }

    elevation.TokenIsElevated != 0
}
