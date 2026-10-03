# Security policy

Security fixes apply to `main` and the latest release.

## Reporting

Use [GitHub private vulnerability reporting](https://github.com/GlennMitchellAU/battery-alerts/security/advisories/new).
Include the app version, Windows version, reproduction steps and impact. Remove
serial numbers, device addresses and personal paths from supporting material.

## Application boundaries

The app uses native Bluetooth, HID and window APIs. It queries device battery
state; it does not alter pairing, firmware, lighting, DPI or button mappings.
Direct HID access is restricted to one documented receiver interface and is
paused while iCUE is running. No driver or administrator privileges are required.

Normal operation has no network traffic, telemetry or automatic updater. The
optional SDK setup script downloads a pinned official archive over HTTPS and
verifies SHA-256 hashes. The app loads that optional DLL by its full path beside
the executable using restricted Windows DLL search flags.

The local alert-state file contains device identifiers to retain reminder state.
Diagnostics replace identifiers by default but preserve device names. Access
violations, unexpected device-setting writes, unsafe DLL loading or disclosure of
identifiers despite default redaction are relevant security reports.
