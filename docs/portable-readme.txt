Battery Alerts 0.1.0 (preview)
https://github.com/GlennMitchellAU/battery-alerts

Windows 10/11 x64. No installer, administrator access or .NET runtime required.
Extract the entire ZIP to a writable folder, then run battery-alerts.exe.

The tray icon opens device status. Right-click for preview, snooze, settings or
exit. Low-battery islands appear at 20% and 10% without taking keyboard focus.
Settings are in battery-alerts.json; restart after editing.

Device support is limited to a verified Corsair SLIPSTREAM 1B1C:1BDC mouse
interface, devices exposing a readable standard Bluetooth LE Battery service,
and an optional iCUE SDK provider. Support is not universal. Cached or unavailable
values do not trigger alerts. See docs/compatibility.md in the source repository.

The standard package contains no proprietary Corsair DLL. The verified receiver
works directly while iCUE is closed. Optional SDK setup is documented online.

Preview:     battery-alerts.exe --preview-only
Diagnostics: battery-alerts.exe --diagnose
Help:        battery-alerts.exe --help

Diagnostics hide identifiers by default, but device names may be personal.
The app runs offline. Reminder state is under %LOCALAPPDATA%/BatteryAlerts.
To start at sign-in, add a shortcut to shell:startup. To remove, exit the tray
app, remove any startup shortcut and delete the extracted folder. You can also
delete the BatteryAlerts local-state folder if you want to reset reminders.

The executable is unsigned. Verify the ZIP against its .sha256 file. Do not
disable Windows security settings to run it.

MIT licensed. See LICENSE, THIRD_PARTY_NOTICES.md and licenses/ for terms.
