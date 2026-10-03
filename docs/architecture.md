# Architecture

Battery Alerts uses Rust with the `windows-sys` Win32 bindings, native GDI drawing
and a Windows message loop. Serde handles portable configuration and local state.
There is no service, browser engine or .NET runtime.

## Readings and policy

Each provider returns a snapshot with device name, identifier, optional percentage,
source, detail and a `fresh` flag. A percentage is fresh only after a successful
live device/SDK read. Windows cached values are visible but never alert or rearm.

The fast worker queries the receiver and optional SDK. A separate worker handles
Bluetooth discovery and live GATT reads. Provider completion messages pass to the
UI thread. Only the provider that just completed updates alert policy, preventing
a retained snapshot from repeatedly evaluating an old reading as newly acquired.

Default policy warns at 20% and escalates at 10%. The first entry into a more
urgent tier alerts immediately; reminders follow the configured interval. A live
reading above warning + 5 percentage points rearms the device. Disconnection does
not reset reminder state. Reminders persist locally; the one-hour snooze is held
in memory and does not survive exit.

## Island and focus

The primary display's work area determines position; a top taskbar is respected.
The window is topmost, excluded from the taskbar and created with
`WS_EX_NOACTIVATE`. Showing uses `SW_SHOWNOACTIVATE`, positioning uses
`SWP_NOACTIVATE`, and pointer activation returns `MA_NOACTIVATE`. Alert display and
the snooze/dismiss interaction do not require changing keyboard focus.

The explicit tray context menu uses the normal Windows foreground-menu behaviour.
Opening settings intentionally starts the user's editor. These user-initiated
actions are distinct from an automatic battery notification.

Expansion and collapse take 280 ms, drawn through a double-buffered GDI surface.
Alerts queue; the active panel closes after the configured duration unless hovered.

## Files

- `src/model.rs`: configuration validation and alert policy.
- `src/slipstream.rs`: narrowly scoped receiver discovery, GET query and parsing.
- `src/bluetooth.rs`: PnP status and standard Battery service reads.
- `src/corsair.rs`: optional runtime-loaded, read-only SDK bindings.
- `src/ui.rs`: tray, workers, persistence, native window and drawing.
- `src/main.rs`: entry point and diagnostics.

`battery-alerts.json` is read beside the executable. Reminder state lives in
`%LOCALAPPDATA%/BatteryAlerts/alerts.json` and includes identifiers for device
continuity. It is not a telemetry log and should not be posted publicly.
