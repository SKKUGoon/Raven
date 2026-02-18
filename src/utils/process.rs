use std::env;
use std::fs::{self, File};
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use crate::config::Settings;
use crate::utils::service_registry;

pub fn get_log_dir() -> PathBuf {
    let home = env::var("HOME").expect("HOME not set");
    PathBuf::from(home).join(".raven/log")
}

pub fn find_binary(name: &str) -> Option<PathBuf> {
    // 1. Same directory as current executable
    if let Ok(current_exe) = env::current_exe() {
        if let Some(dir) = current_exe.parent() {
            let candidate = dir.join(name);
            if candidate.exists() {
                return Some(candidate);
            }
            // 2. Development path: target/release (if we are in target/debug or similar)
            let candidate_rel = PathBuf::from("target/release").join(name);
            if candidate_rel.exists() {
                return Some(candidate_rel);
            }
            // 3. Just checking relative to CWD
            let candidate_cwd = PathBuf::from(name);
            if candidate_cwd.exists() {
                return Some(candidate_cwd);
            }
        }
    }
    None
}

pub fn start_service_proc(bin_name: &str, service_name: &str, args: &[&str]) {
    let log_dir = get_log_dir();
    if let Err(e) = fs::create_dir_all(&log_dir) {
        eprintln!("Failed to create log dir {log_dir:?}: {e}");
        return;
    }

    let bin_path = match find_binary(bin_name) {
        Some(p) => p,
        None => {
            eprintln!("Binary {bin_name} not found.");
            return;
        }
    };

    let log_file_path = log_dir.join(format!("{service_name}.log"));
    let log_file = match File::create(&log_file_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Failed to create log file {log_file_path:?}: {e}");
            return;
        }
    };

    println!("Starting {service_name}...");

    // We clone log_file for stderr
    let stderr_file = match log_file.try_clone() {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Failed to clone log file handle: {e}");
            return;
        }
    };

    let child_res = Command::new(bin_path)
        .args(args)
        .stdout(log_file)
        .stderr(stderr_file)
        .spawn();

    match child_res {
        Ok(child) => {
            let pid = child.id();
            let pid_file = log_dir.join(format!("{service_name}.pid"));
            if let Err(e) = fs::write(&pid_file, pid.to_string()) {
                eprintln!("Failed to write pid file: {e}");
            }
        }
        Err(e) => eprintln!("Failed to start {service_name}: {e}"),
    }
}

pub fn start_all_services_with_settings(settings: &Settings) {
    println!("Starting Raven services...");

    let services = service_registry::all_services(settings);
    for svc in services {
        let eff = svc.effective_args();
        let args: Vec<&str> = eff.iter().map(|s| s.as_str()).collect();
        start_service_proc(svc.bin_name, svc.log_name, &args);
    }

    let log_dir = get_log_dir();
    println!("All services started. PIDs stored in {log_dir:?}/");
    println!("Check logs in {log_dir:?}/ for output.");
}

pub fn running_services(settings: &Settings) -> Vec<String> {
    let log_dir = get_log_dir();
    let services = service_registry::all_services(settings);
    let mut out = Vec::new();

    for svc in services {
        let pid_path = log_dir.join(format!("{}.pid", svc.log_name));
        if !pid_path.exists() {
            continue;
        }
        let Ok(content) = fs::read_to_string(&pid_path) else {
            continue;
        };
        let Ok(pid) = content.trim().parse::<i32>() else {
            continue;
        };
        if is_pid_running(pid) {
            out.push(format!("{} ({})", svc.display_name, svc.log_name));
        }
    }

    out
}

fn is_pid_running(pid: i32) -> bool {
    Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn pid_command_name(pid: i32) -> Option<String> {
    let out = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "comm="])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

fn pids_listening_on_port(port: u16) -> Option<Vec<i32>> {
    let out = Command::new("lsof")
        .args([
            "-nP",
            &format!("-iTCP:{port}"),
            "-sTCP:LISTEN",
            "-t",
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return Some(Vec::new());
    }

    let mut pids = Vec::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        if let Ok(pid) = line.trim().parse::<i32>() {
            if !pids.contains(&pid) {
                pids.push(pid);
            }
        }
    }
    Some(pids)
}

fn kill_pid_gracefully(pid: i32, name: &str) {
    if !is_pid_running(pid) {
        println!("{name} (PID: {pid}) is not running. Cleaning up.");
        return;
    }

    println!("Killing {name} (PID: {pid}) with SIGTERM...");
    let _ = Command::new("kill").arg(pid.to_string()).status();

    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        if !is_pid_running(pid) {
            println!("{name} stopped.");
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    println!("{name} still running; sending SIGKILL...");
    let _ = Command::new("kill").arg("-9").arg(pid.to_string()).status();
}

/// Stop a single service using its PID file name (e.g. `binance_spot`, `timebar_1m`).
/// Returns true if a pid file was found (regardless of whether the process was running).
pub fn stop_service(log_name: &str) -> bool {
    let log_dir = get_log_dir();
    let pid_path = log_dir.join(format!("{log_name}.pid"));
    if !pid_path.exists() {
        return false;
    }

    if let Ok(content) = fs::read_to_string(&pid_path) {
        if let Ok(pid) = content.trim().parse::<i32>() {
            // Best-effort safety check: warn if pid doesn't look like the expected binary name.
            if let Some(comm) = pid_command_name(pid) {
                if !comm.contains(log_name)
                    && !comm.contains("raven_")
                    && !comm.contains("binance_")
                {
                    eprintln!(
                        "Warning: PID {pid} command looks like `{comm}`, expected something like `{log_name}`. Proceeding to kill anyway."
                    );
                }
            }
            kill_pid_gracefully(pid, log_name);
        }
    }

    let _ = fs::remove_file(&pid_path);
    true
}

/// Stop a single service by resolving PID from control port first; falls back to pid file.
pub fn stop_service_by_port_or_pid(log_name: &str, port: u16) -> bool {
    match pids_listening_on_port(port) {
        Some(mut pids) => {
            if let Some(pid) = pids.pop() {
                if !pids.is_empty() {
                    eprintln!(
                        "Multiple pids found listening on port {port}; using pid {pid} for {log_name}."
                    );
                }
                kill_pid_gracefully(pid, log_name);
                let pid_path = get_log_dir().join(format!("{log_name}.pid"));
                let _ = fs::remove_file(&pid_path);
                return true;
            }
        }
        None => {
            // lsof is unavailable or failed to execute; fallback to pid file behavior below.
        }
    }

    stop_service(log_name)
}

pub fn stop_all_services() {
    let log_dir = get_log_dir();
    if !log_dir.exists() {
        println!("No logs directory found.");
        return;
    }

    println!("Stopping Raven services...");

    let entries = match fs::read_dir(&log_dir) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Failed to read log dir: {e}");
            return;
        }
    };

    let mut found_pid = false;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("pid") {
            found_pid = true;
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                let _ = stop_service(stem);
            }
        }
    }

    if !found_pid {
        println!("No running services found (no .pid files).");
    } else {
        println!("All services stopped.");
    }
}

pub fn stop_all_services_with_settings(settings: &Settings) {
    println!("Stopping Raven services...");
    let mut found_any = false;
    for svc in service_registry::all_services(settings) {
        if stop_service_by_port_or_pid(svc.log_name, svc.port) {
            found_any = true;
        }
    }

    if !found_any {
        println!("No running services found.");
    } else {
        println!("All services stopped.");
    }
}
