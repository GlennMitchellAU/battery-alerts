use crate::{
    base, data_dir,
    model::{Config, Policy, Snapshot},
    now, wide,
};
use std::{
    collections::VecDeque,
    mem::{size_of, zeroed},
    ptr,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
        Arc,
    },
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::{LibraryLoader::GetModuleHandleW, Threading::CreateMutexW},
    UI::{HiDpi::*, Shell::*, WindowsAndMessaging::*},
};

const TRAY_MSG: u32 = WM_APP + 1;
const TICK: usize = 1;
const MOTION: usize = 2;
const ID_STATUS: u32 = 100;
const ID_DEMO: u32 = 101;
const ID_PAUSE: u32 = 102;
const ID_CONFIG: u32 = 103;
const ID_EXIT: u32 = 104;
struct Notice {
    title: String,
    subtitle: String,
    percent: Option<u8>,
    critical: bool,
    demo: bool,
}
struct App {
    config: Config,
    receiver: Receiver<(bool, Snapshot)>,
    wake: mpsc::Sender<()>,
    stop: Arc<AtomicBool>,
    snapshot: Snapshot,
    fast_snapshot: Snapshot,
    bluetooth_snapshot: Snapshot,
    policy: Policy,
    queue: VecDeque<Notice>,
    current: Option<Notice>,
    showing_status: bool,
    tray: NOTIFYICONDATAW,
    paused_until: u64,
    opened: Instant,
    animation_start: Instant,
    exiting: bool,
    width: i32,
    height: i32,
    dpi: f64,
    preview_only: bool,
    initialized: bool,
    taskbar_created: u32,
}
pub fn run(config: Config, demo: bool, preview_only: bool) -> Result<(), String> {
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let mutex = CreateMutexW(ptr::null(), 0, wide("Local\\BatteryAlerts-0.1").as_ptr());
        if mutex.is_null() {
            return Err("Could not create the application mutex.".into());
        }
        if GetLastError() == ERROR_ALREADY_EXISTS {
            CloseHandle(mutex);
            return Ok(());
        }
        let (sender, receiver) = mpsc::channel();
        let (wake, poll) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        if !preview_only {
            let worker_stop = stop.clone();
            let fast_sender = sender.clone();
            let seconds = config.poll_seconds;
            std::thread::spawn(move || {
                let sdk = crate::corsair::Corsair::load(&base().join("iCUESDK.x64_2019.dll"));
                // The SDK connects asynchronously. BLE discovery can run immediately.
                std::thread::sleep(Duration::from_secs(2));
                while !worker_stop.load(Ordering::Relaxed) {
                    if fast_sender.send((true, crate::fast_scan(&sdk))).is_err() {
                        break;
                    }
                    if let Err(mpsc::RecvTimeoutError::Disconnected) =
                        poll.recv_timeout(Duration::from_secs(seconds))
                    {
                        break;
                    }
                }
            });
            let bluetooth_stop = stop.clone();
            std::thread::spawn(move || {
                while !bluetooth_stop.load(Ordering::Relaxed) {
                    if sender.send((false, crate::bluetooth::scan())).is_err() {
                        break;
                    }
                    std::thread::sleep(Duration::from_secs(seconds.max(60)));
                }
            });
        }
        let policy = std::fs::read_to_string(data_dir().join("alerts.json"))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        let instance = GetModuleHandleW(ptr::null());
        let class_name = wide("BatteryAlertsIsland");
        let mut class: WNDCLASSW = zeroed();
        class.lpfnWndProc = Some(window_proc);
        class.hInstance = instance;
        class.lpszClassName = class_name.as_ptr();
        class.hCursor = LoadCursorW(ptr::null_mut(), IDC_ARROW);
        class.style = CS_DROPSHADOW;
        if RegisterClassW(&class) == 0 {
            CloseHandle(mutex);
            return Err("Could not register the island window.".into());
        }
        let mut app = Box::new(App {
            config,
            receiver,
            wake,
            stop,
            snapshot: Snapshot::default(),
            fast_snapshot: Snapshot::default(),
            bluetooth_snapshot: Snapshot::default(),
            policy,
            queue: VecDeque::new(),
            current: None,
            showing_status: false,
            tray: zeroed(),
            paused_until: 0,
            opened: Instant::now(),
            animation_start: Instant::now(),
            exiting: false,
            width: 500,
            height: 164,
            dpi: 1.0,
            preview_only,
            initialized: false,
            taskbar_created: RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()),
        });
        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            class_name.as_ptr(),
            wide("Battery Alerts").as_ptr(),
            WS_POPUP,
            0,
            0,
            500,
            164,
            ptr::null_mut(),
            ptr::null_mut(),
            instance,
            &mut *app as *mut App as *const std::ffi::c_void,
        );
        if hwnd.is_null() {
            CloseHandle(mutex);
            return Err("Could not create the island window.".into());
        }
        app.tray.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
        app.tray.hWnd = hwnd;
        app.tray.uID = 1;
        app.tray.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
        app.tray.uCallbackMessage = TRAY_MSG;
        app.tray.hIcon = create_tray_icon();
        set_text(&mut app.tray.szTip, "Battery Alerts | Checking devices...");
        if !preview_only && Shell_NotifyIconW(NIM_ADD, &app.tray) == 0 {
            DestroyWindow(hwnd);
            CloseHandle(mutex);
            return Err("Could not add the tray icon.".into());
        }
        SetTimer(hwnd, TICK, 250, None);
        if demo || preview_only {
            show_notice(hwnd, &mut app, demo_notice());
        }
        let mut msg: MSG = zeroed();
        while GetMessageW(&mut msg, ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        app.stop.store(true, Ordering::Relaxed);
        let _ = app.wake.send(());
        if !preview_only {
            Shell_NotifyIconW(NIM_DELETE, &app.tray);
        }
        DestroyIcon(app.tray.hIcon);
        CloseHandle(mutex);
        Ok(())
    }
}
fn demo_notice() -> Notice {
    Notice {
        title: "CORSAIR wireless mouse".into(),
        subtitle: "Plug in your mouse before it switches off".into(),
        percent: Some(15),
        critical: false,
        demo: true,
    }
}
fn set_text<const N: usize>(target: &mut [u16; N], text: &str) {
    target.fill(0);
    for (i, c) in text.encode_utf16().take(N - 1).enumerate() {
        target[i] = c;
    }
}
unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_NCCREATE {
        let create = &*(lparam as *const CREATESTRUCTW);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
        return DefWindowProcW(hwnd, message, wparam, lparam);
    }
    if message == WM_MOUSEACTIVATE {
        return MA_NOACTIVATE as isize;
    }
    if message == WM_ERASEBKGND {
        return 1;
    }
    let state = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut App;
    if state.is_null() {
        return DefWindowProcW(hwnd, message, wparam, lparam);
    }
    if message == (*state).taskbar_created && (*state).taskbar_created != 0 {
        Shell_NotifyIconW(NIM_ADD, &(*state).tray);
        return 0;
    }
    // Positioning and drawing send synchronous native messages. Ignore messages
    // that need no app state before borrowing it, so those callbacks do not take
    // an overlapping mutable reference while layout is in progress.
    if !matches!(
        message,
        WM_PAINT
            | WM_TIMER
            | WM_LBUTTONUP
            | WM_CONTEXTMENU
            | TRAY_MSG
            | WM_COMMAND
            | WM_DISPLAYCHANGE
            | WM_DPICHANGED
            | WM_POWERBROADCAST
            | WM_CLOSE
            | WM_DESTROY
    ) {
        return DefWindowProcW(hwnd, message, wparam, lparam);
    }
    let app = &mut *state;
    match message {
        WM_MOUSEACTIVATE => MA_NOACTIVATE as isize,
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            paint(hwnd, app);
            0
        }
        WM_TIMER if wparam == TICK => {
            tick(hwnd, app);
            0
        }
        WM_TIMER if wparam == MOTION => {
            animate(hwnd, app);
            0
        }
        WM_LBUTTONUP => {
            let x = (lparam & 0xffff) as i16 as f64 / app.dpi;
            let y = ((lparam >> 16) & 0xffff) as i16 as f64 / app.dpi;
            if y >= app.height as f64 - 46.0 && (270.0..390.0).contains(&x) && !app.showing_status {
                app.paused_until = now() + 3600;
                app.queue.clear();
                for state in app.policy.devices.values_mut() {
                    state.last_alert = 0;
                }
            }
            hide(hwnd, app);
            0
        }
        WM_CONTEXTMENU => {
            tray_menu(hwnd, app);
            0
        }
        TRAY_MSG => {
            match lparam as u32 {
                WM_LBUTTONUP => {
                    show_status(hwnd, app);
                }
                WM_RBUTTONUP | WM_CONTEXTMENU => {
                    tray_menu(hwnd, app);
                }
                _ => {}
            }
            0
        }
        WM_COMMAND => {
            command(hwnd, app, (wparam & 0xffff) as u32);
            0
        }
        WM_DISPLAYCHANGE | WM_DPICHANGED => {
            if app.current.is_some() || app.showing_status {
                layout(hwnd, app, 1.0);
            }
            0
        }
        WM_POWERBROADCAST => {
            if wparam == 7 || wparam == 18 {
                let _ = app.wake.send(());
            }
            1
        }
        WM_CLOSE => {
            DestroyWindow(hwnd);
            0
        }
        WM_DESTROY => {
            KillTimer(hwnd, TICK);
            KillTimer(hwnd, MOTION);
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}
unsafe fn tick(hwnd: HWND, app: &mut App) {
    while let Ok((fast, snapshot)) = app.receiver.try_recv() {
        if fast {
            app.fast_snapshot = snapshot;
        } else {
            app.bluetooth_snapshot = snapshot;
        }
        app.snapshot = app.fast_snapshot.clone();
        app.snapshot
            .devices
            .extend(app.bluetooth_snapshot.devices.clone());
        app.snapshot
            .status
            .extend(app.bluetooth_snapshot.status.clone());
        let live = app.snapshot.devices.iter().filter(|d| d.fresh).count();
        let tooltip = if app.paused_until > now() {
            "Battery Alerts | Alerts snoozed".into()
        } else {
            format!("Battery Alerts | {live} live battery reading(s)")
        };
        set_text(&mut app.tray.szTip, &tooltip);
        Shell_NotifyIconW(NIM_MODIFY, &app.tray);
        if app.paused_until <= now() {
            // Only the provider that just completed may change alert state.
            let completed = if fast {
                &app.fast_snapshot
            } else {
                &app.bluetooth_snapshot
            };
            for device in &completed.devices {
                if app.policy.evaluate(device, &app.config, now()) {
                    app.queue.push_back(Notice {
                        title: device.name.clone(),
                        subtitle: "Charge soon to avoid losing your connection".into(),
                        percent: device.battery,
                        critical: device
                            .battery
                            .is_some_and(|v| v <= app.config.critical_percent),
                        demo: false,
                    });
                }
            }
            save_policy(&app.policy);
        }
        if !app.initialized {
            app.initialized = true;
            if live == 0 && app.current.is_none() {
                app.queue.push_back(Notice {
                    title: "No live battery readings yet".into(),
                    subtitle: "Open tray status to check device support".into(),
                    percent: None,
                    critical: false,
                    demo: false,
                });
            }
        }
        if app.showing_status {
            InvalidateRect(hwnd, ptr::null(), 0);
        }
    }
    if app.current.is_none() && !app.showing_status {
        if let Some(notice) = app.queue.pop_front() {
            show_notice(hwnd, app, notice);
        } else if app.preview_only && app.initialized {
            DestroyWindow(hwnd);
            return;
        }
    }
    if (app.current.is_some() || app.showing_status)
        && !app.exiting
        && app.opened.elapsed().as_secs() >= app.config.display_seconds
    {
        let mut cursor: POINT = zeroed();
        GetCursorPos(&mut cursor);
        let mut rect: RECT = zeroed();
        GetWindowRect(hwnd, &mut rect);
        if PtInRect(&rect, cursor) == 0 {
            hide(hwnd, app);
        }
    }
}
fn save_policy(policy: &Policy) {
    if std::fs::create_dir_all(data_dir()).is_ok() {
        if let Ok(json) = serde_json::to_string(policy) {
            let _ = std::fs::write(data_dir().join("alerts.json"), json);
        }
    }
}
unsafe fn show_notice(hwnd: HWND, app: &mut App, notice: Notice) {
    app.current = Some(notice);
    app.showing_status = false;
    app.height = 164;
    open(hwnd, app);
}
unsafe fn show_status(hwnd: HWND, app: &mut App) {
    app.current = None;
    app.showing_status = true;
    app.height = 144 + app.snapshot.devices.len().clamp(1, 6) as i32 * 54;
    open(hwnd, app);
}
unsafe fn open(hwnd: HWND, app: &mut App) {
    app.exiting = false;
    app.opened = Instant::now();
    app.animation_start = Instant::now();
    layout(hwnd, app, 0.0);
    ShowWindow(hwnd, SW_SHOWNOACTIVATE);
    SetTimer(hwnd, MOTION, 16, None);
}
unsafe fn hide(hwnd: HWND, app: &mut App) {
    if app.exiting {
        return;
    }
    app.exiting = true;
    app.animation_start = Instant::now();
    SetTimer(hwnd, MOTION, 16, None);
}
unsafe fn animate(hwnd: HWND, app: &mut App) {
    let time = (app.animation_start.elapsed().as_secs_f64() / 0.28).min(1.0);
    let ease = 1.0 - (1.0 - time).powi(3);
    layout(hwnd, app, if app.exiting { 1.0 - ease } else { ease });
    if time >= 1.0 {
        KillTimer(hwnd, MOTION);
        if app.exiting {
            ShowWindow(hwnd, SW_HIDE);
            app.current = None;
            app.showing_status = false;
            app.exiting = false;
            app.initialized |= app.preview_only;
        }
    }
}
unsafe fn layout(hwnd: HWND, app: &mut App, progress: f64) {
    // Windows' primary monitor contains (0,0). Work-area top avoids covering a top taskbar.
    let monitor = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
    let mut info: MONITORINFO = zeroed();
    info.cbSize = size_of::<MONITORINFO>() as u32;
    GetMonitorInfoW(monitor, &mut info);
    let dpi = GetDpiForWindow(hwnd).max(96) as f64 / 96.0;
    app.dpi = dpi;
    let width = ((160.0 + (app.width as f64 - 160.0) * progress) * dpi) as i32;
    let height = ((6.0 + (app.height as f64 - 6.0) * progress) * dpi) as i32;
    let x = info.rcWork.left + (info.rcWork.right - info.rcWork.left - width) / 2;
    SetWindowPos(
        hwnd,
        HWND_TOPMOST,
        x,
        info.rcWork.top,
        width,
        height,
        SWP_NOACTIVATE | SWP_NOOWNERZORDER,
    );
    let radius = (28.0 * dpi) as i32;
    let region = CreateRoundRectRgn(0, -radius, width + 1, height + 1, radius * 2, radius * 2);
    if SetWindowRgn(hwnd, region, 0) == 0 {
        DeleteObject(region);
    }
    InvalidateRect(hwnd, ptr::null(), 0);
}
unsafe fn tray_menu(hwnd: HWND, app: &mut App) {
    let menu = CreatePopupMenu();
    for (id, label) in [
        (ID_STATUS, "Device battery status"),
        (ID_DEMO, "Preview island (demo)"),
        (
            ID_PAUSE,
            if app.paused_until > now() {
                "Resume alerts"
            } else {
                "Snooze alerts for 1 hour"
            },
        ),
        (ID_CONFIG, "Open settings file"),
        (ID_EXIT, "Exit Battery Alerts"),
    ] {
        AppendMenuW(menu, MF_STRING, id as usize, wide(label).as_ptr());
    }
    let mut point: POINT = zeroed();
    GetCursorPos(&mut point);
    SetForegroundWindow(hwnd);
    let choice = TrackPopupMenu(
        menu,
        TPM_RETURNCMD | TPM_RIGHTBUTTON,
        point.x,
        point.y,
        0,
        hwnd,
        ptr::null(),
    );
    DestroyMenu(menu);
    PostMessageW(hwnd, WM_NULL, 0, 0);
    if choice != 0 {
        command(hwnd, app, choice as u32);
    }
}
unsafe fn command(hwnd: HWND, app: &mut App, command: u32) {
    match command {
        ID_STATUS => {
            show_status(hwnd, app);
            let _ = app.wake.send(());
        }
        ID_DEMO => show_notice(hwnd, app, demo_notice()),
        ID_PAUSE => {
            app.paused_until = if app.paused_until > now() {
                0
            } else {
                now() + 3600
            };
            app.queue.clear();
            for state in app.policy.devices.values_mut() {
                state.last_alert = 0;
            }
        }
        ID_CONFIG => {
            let path = base().join("battery-alerts.json");
            if !path.exists() {
                let _ = std::fs::write(&path, serde_json::to_string_pretty(&app.config).unwrap());
            }
            ShellExecuteW(
                hwnd,
                wide("open").as_ptr(),
                wide("notepad.exe").as_ptr(),
                wide(&format!("\"{}\"", path.display())).as_ptr(),
                ptr::null(),
                SW_SHOWNORMAL,
            );
        }
        ID_EXIT => {
            DestroyWindow(hwnd);
        }
        _ => {}
    }
}
fn color(r: u8, g: u8, b: u8) -> COLORREF {
    r as u32 | (g as u32) << 8 | (b as u32) << 16
}
#[allow(clippy::too_many_arguments)] // Small drawing primitives mirror native GDI calls.
unsafe fn text(
    dc: HDC,
    dpi: f64,
    label: &str,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    points: i32,
    weight: i32,
    tint: COLORREF,
    align: u32,
) {
    let font = CreateFontW(
        -(points as f64 * dpi) as i32,
        0,
        0,
        0,
        weight,
        0,
        0,
        0,
        DEFAULT_CHARSET as u32,
        0,
        0,
        CLEARTYPE_QUALITY as u32,
        0,
        wide("Segoe UI").as_ptr(),
    );
    let old = SelectObject(dc, font);
    SetTextColor(dc, tint);
    SetBkMode(dc, TRANSPARENT as i32);
    let mut rect = RECT {
        left: (x as f64 * dpi) as i32,
        top: (y as f64 * dpi) as i32,
        right: ((x + w) as f64 * dpi) as i32,
        bottom: ((y + h) as f64 * dpi) as i32,
    };
    let label = wide(label);
    DrawTextW(
        dc,
        label.as_ptr(),
        -1,
        &mut rect,
        DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX | align,
    );
    SelectObject(dc, old);
    DeleteObject(font);
}
#[allow(clippy::too_many_arguments)]
unsafe fn rounded(dc: HDC, dpi: f64, x: i32, y: i32, w: i32, h: i32, radius: i32, tint: COLORREF) {
    let brush = CreateSolidBrush(tint);
    let old_brush = SelectObject(dc, brush);
    let old_pen = SelectObject(dc, GetStockObject(NULL_PEN));
    RoundRect(
        dc,
        (x as f64 * dpi) as i32,
        (y as f64 * dpi) as i32,
        ((x + w) as f64 * dpi) as i32,
        ((y + h) as f64 * dpi) as i32,
        (radius as f64 * dpi) as i32,
        (radius as f64 * dpi) as i32,
    );
    SelectObject(dc, old_brush);
    SelectObject(dc, old_pen);
    DeleteObject(brush);
}
unsafe fn paint(hwnd: HWND, app: &App) {
    let mut ps: PAINTSTRUCT = zeroed();
    let window_dc = BeginPaint(hwnd, &mut ps);
    let mut rect: RECT = zeroed();
    GetClientRect(hwnd, &mut rect);
    let dc = CreateCompatibleDC(window_dc);
    let bitmap = CreateCompatibleBitmap(window_dc, rect.right.max(1), rect.bottom.max(1));
    let old = SelectObject(dc, bitmap);
    let black = CreateSolidBrush(color(9, 10, 12));
    FillRect(dc, &rect, black);
    DeleteObject(black);
    let dpi = app.dpi;
    let white = color(246, 246, 249);
    let muted = color(150, 151, 158);
    if app.showing_status {
        text(
            dc,
            dpi,
            "DEVICE BATTERIES",
            28,
            17,
            350,
            22,
            11,
            600,
            muted,
            0,
        );
        text(
            dc,
            dpi,
            "Wireless, without surprises",
            28,
            42,
            420,
            28,
            21,
            600,
            white,
            0,
        );
        if app.snapshot.devices.is_empty() {
            text(
                dc,
                dpi,
                "Checking devices...",
                28,
                91,
                430,
                28,
                15,
                400,
                muted,
                0,
            );
        }
        for (i, device) in app.snapshot.devices.iter().take(6).enumerate() {
            let y = 85 + i as i32 * 54;
            text(dc, dpi, &device.name, 28, y, 343, 23, 15, 600, white, 0);
            let value = device
                .battery
                .map(|v| format!("{v}%"))
                .unwrap_or_else(|| "N/A".into());
            let tint = if !device.fresh {
                muted
            } else if device
                .battery
                .is_some_and(|v| v <= app.config.warning_percent)
            {
                color(255, 184, 77)
            } else {
                color(102, 218, 158)
            };
            text(dc, dpi, &value, 389, y, 83, 25, 19, 600, tint, DT_RIGHT);
            let detail = if device.fresh {
                device.source.clone()
            } else if device.battery.is_some() {
                "Last known; live reading unavailable".into()
            } else {
                "Battery reading unavailable".into()
            };
            text(dc, dpi, &detail, 28, y + 24, 440, 20, 11, 400, muted, 0);
        }
        let status = app
            .snapshot
            .status
            .iter()
            .find(|v| {
                v.starts_with(
                    if app
                        .snapshot
                        .devices
                        .iter()
                        .any(|d| d.id.starts_with("slipstream:") && d.fresh)
                    {
                        "SLIPSTREAM:"
                    } else {
                        "Corsair:"
                    },
                )
            })
            .cloned()
            .unwrap_or_else(|| "Monitoring supported devices".into());
        text(
            dc,
            dpi,
            &status,
            28,
            app.height - 49,
            440,
            20,
            11,
            400,
            muted,
            0,
        );
        text(
            dc,
            dpi,
            "Click to close  ·  Right-click for options",
            28,
            app.height - 28,
            440,
            18,
            10,
            400,
            muted,
            0,
        );
    } else if let Some(notice) = &app.current {
        let accent = if notice.critical {
            color(255, 103, 108)
        } else {
            color(255, 184, 77)
        };
        let heading = if notice.demo {
            "LOW BATTERY  ·  PREVIEW"
        } else if notice.percent.is_none() {
            "BATTERY MONITOR"
        } else if notice.critical {
            "BATTERY CRITICAL"
        } else {
            "LOW BATTERY"
        };
        rounded(dc, dpi, 29, 21, 6, 6, 6, accent);
        text(dc, dpi, heading, 45, 14, 400, 22, 11, 600, muted, 0);
        // A small mouse silhouette communicates the peripheral without image assets.
        rounded(dc, dpi, 29, 53, 40, 58, 30, color(45, 46, 52));
        rounded(dc, dpi, 46, 61, 6, 13, 6, accent);
        text(dc, dpi, &notice.title, 88, 49, 301, 29, 20, 600, white, 0);
        text(
            dc,
            dpi,
            &notice.subtitle,
            88,
            82,
            294,
            25,
            12,
            400,
            muted,
            0,
        );
        if let Some(percent) = notice.percent {
            text(
                dc,
                dpi,
                &format!("{percent}%"),
                391,
                47,
                82,
                43,
                30,
                600,
                accent,
                DT_RIGHT,
            );
            text(
                dc,
                dpi,
                "remaining",
                386,
                84,
                85,
                20,
                10,
                400,
                muted,
                DT_RIGHT,
            );
        }
        if notice.percent.is_some() {
            rounded(dc, dpi, 270, 123, 113, 27, 20, color(31, 32, 37));
            text(
                dc,
                dpi,
                "Snooze 1 hour",
                270,
                123,
                113,
                27,
                11,
                500,
                white,
                DT_CENTER,
            );
        }
        text(
            dc, dpi, "Dismiss", 393, 123, 79, 27, 11, 500, muted, DT_CENTER,
        );
    }
    BitBlt(window_dc, 0, 0, rect.right, rect.bottom, dc, 0, 0, SRCCOPY);
    SelectObject(dc, old);
    DeleteObject(bitmap);
    DeleteDC(dc);
    EndPaint(hwnd, &ps);
}
unsafe fn create_tray_icon() -> HICON {
    let screen = GetDC(ptr::null_mut());
    let dc = CreateCompatibleDC(screen);
    let bmp = CreateCompatibleBitmap(screen, 32, 32);
    let old = SelectObject(dc, bmp);
    let rect = RECT {
        left: 0,
        top: 0,
        right: 32,
        bottom: 32,
    };
    let bg = CreateSolidBrush(color(10, 11, 14));
    FillRect(dc, &rect, bg);
    DeleteObject(bg);
    rounded(dc, 1.0, 4, 9, 23, 14, 5, color(119, 224, 163));
    rounded(dc, 1.0, 27, 13, 3, 6, 2, color(119, 224, 163));
    let mask = CreateBitmap(32, 32, 1, 1, [0u8; 128].as_ptr() as *const _);
    let info = ICONINFO {
        fIcon: 1,
        xHotspot: 0,
        yHotspot: 0,
        hbmMask: mask,
        hbmColor: bmp,
    };
    let icon = CreateIconIndirect(&info);
    SelectObject(dc, old);
    DeleteObject(bmp);
    DeleteObject(mask);
    DeleteDC(dc);
    ReleaseDC(ptr::null_mut(), screen);
    icon
}
