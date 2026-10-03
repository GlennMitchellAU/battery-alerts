#![cfg_attr(not(test), windows_subsystem = "windows")]
mod bluetooth;
mod corsair;
mod model;
mod slipstream;
mod ui;
use model::{Config, Snapshot};
use std::{
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn base() -> PathBuf {
    std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}
fn data_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(base)
        .join("BatteryAlerts")
}
fn scan(sdk: &Result<corsair::Corsair, String>) -> Snapshot {
    let mut snapshot = fast_scan(sdk);
    let bluetooth = bluetooth::scan();
    snapshot.devices.extend(bluetooth.devices);
    snapshot.status.extend(bluetooth.status);
    snapshot
}
fn fast_scan(sdk: &Result<corsair::Corsair, String>) -> Snapshot {
    let mut snapshot = slipstream::scan();
    match sdk {
        Ok(sdk) => {
            let (devices, status) = sdk.scan();
            snapshot.devices.extend(devices);
            snapshot.status.push(status);
        }
        Err(error) => snapshot.status.push(error.clone()),
    }
    snapshot
}
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let console = args
        .iter()
        .any(|v| matches!(v.as_str(), "--diagnose" | "--help" | "--version"));
    if console {
        unsafe {
            windows_sys::Win32::System::Console::AttachConsole(u32::MAX);
        }
    }
    if let Err(error) = run(&args) {
        if console {
            eprintln!("Battery Alerts: {error}");
        } else {
            unsafe {
                windows_sys::Win32::UI::WindowsAndMessaging::MessageBoxW(
                    std::ptr::null_mut(),
                    wide(&error).as_ptr(),
                    wide("Battery Alerts").as_ptr(),
                    0x10,
                );
            }
        }
        std::process::exit(1);
    }
}
fn run(args: &[String]) -> Result<(), String> {
    if args.iter().any(|v| v == "--help") {
        println!("Battery Alerts\n\nUsage: battery-alerts.exe [OPTIONS]\n\n  --demo                 Monitor devices and show a simulated warning\n  --preview-only         Preview without polling devices; exit when closed\n  --diagnose             Print JSON readings and exit\n  --receiver-only        With --diagnose, skip Bluetooth scanning\n  --include-identifiers  With --diagnose, include device identifiers\n  --version              Print the application version\n  --help                 Show this help\n\nSettings: battery-alerts.json beside the executable.\nDiagnostics hide identifiers by default; device names may still be personal.");
        return Ok(());
    }
    if args.iter().any(|v| v == "--version") {
        println!("Battery Alerts {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let diagnose = args.iter().any(|v| v == "--diagnose");
    for arg in args {
        if !matches!(
            arg.as_str(),
            "--demo"
                | "--preview-only"
                | "--diagnose"
                | "--receiver-only"
                | "--include-identifiers"
        ) {
            return Err(format!(
                "Unknown option: {arg}. Use --help to see available options."
            ));
        }
        if matches!(arg.as_str(), "--receiver-only" | "--include-identifiers") && !diagnose {
            return Err(format!("{arg} requires --diagnose."));
        }
    }
    let config_path = base().join("battery-alerts.json");
    let config = if config_path.exists() {
        serde_json::from_str::<Config>(
            &std::fs::read_to_string(&config_path).map_err(|e| e.to_string())?,
        )
        .map_err(|e| format!("Invalid battery-alerts.json: {e}"))?
    } else {
        Config::default()
    };
    config.validate()?;
    if diagnose {
        let sdk = corsair::Corsair::load(&base().join("iCUESDK.x64_2019.dll"));
        std::thread::sleep(Duration::from_secs(3));
        let snapshot = if args.iter().any(|v| v == "--receiver-only") {
            fast_scan(&sdk)
        } else {
            scan(&sdk)
        };
        let snapshot =
            diagnostic_snapshot(snapshot, args.iter().any(|v| v == "--include-identifiers"));
        println!("{}", serde_json::to_string_pretty(&snapshot).unwrap());
        return Ok(());
    }
    ui::run(
        config,
        args.iter().any(|v| v == "--demo"),
        args.iter().any(|v| v == "--preview-only"),
    )
}

fn diagnostic_snapshot(mut snapshot: Snapshot, include_identifiers: bool) -> Snapshot {
    if !include_identifiers {
        for (i, device) in snapshot.devices.iter_mut().enumerate() {
            device.id = format!("device-{}", i + 1);
        }
    }
    snapshot
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diagnostic_identifiers_require_explicit_opt_in() {
        let snapshot = Snapshot {
            devices: vec![model::Device {
                id: "slipstream:private-example-serial".into(),
                name: "Example mouse".into(),
                battery: Some(45),
                fresh: true,
                source: "SLIPSTREAM receiver".into(),
                detail: String::new(),
            }],
            status: vec![],
        };
        let public = diagnostic_snapshot(snapshot.clone(), false);
        assert_eq!(public.devices[0].id, "device-1");
        assert!(!serde_json::to_string(&public)
            .unwrap()
            .contains("private-example-serial"));
        assert_eq!(
            diagnostic_snapshot(snapshot, true).devices[0].id,
            "slipstream:private-example-serial"
        );
    }
}
