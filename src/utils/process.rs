use std::env;
use std::fs::{self, File};
use std::path::PathBuf;
use std::process::Command;

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

pub fn start_all_services() {
    eprintln!("start_all_services() without settings is deprecated; use start_all_services_with_settings().");
}

pub fn start_all_services_with_settings(settings: &Settings) {
    println!("Starting Raven services...");

    let services = service_registry::all_services(settings);
    for svc in services {
        let args: Vec<&str> = svc.args.iter().map(|s| s.as_str()).collect();
        start_service_proc(svc.bin_name, svc.log_name, &args);
    }

    let log_dir = get_log_dir();
    println!("All services started. PIDs stored in {log_dir:?}/");
    println!("Check logs in {log_dir:?}/ for output.");
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
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(pid) = content.trim().parse::<i32>() {
                    let name = path.file_stem().unwrap().to_string_lossy();
                    
                    // Use standard kill command since we are on unix-like systems
                    if Command::new("kill").arg("-0").arg(pid.to_string()).status().map(|s| s.success()).unwrap_or(false) {
                         println!("Killing {name} (PID: {pid})...");
                         let _ = Command::new("kill").arg(pid.to_string()).status();
                    } else {
                        println!("{name} (PID: {pid}) is not running. Cleaning up.");
                    }
                    
                    // Remove pid file
                    let _ = fs::remove_file(&path);
                }
            }
        }
    }

    if !found_pid {
        println!("No running services found (no .pid files).");
    } else {
        println!("All services stopped.");
    }
}

