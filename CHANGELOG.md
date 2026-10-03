# Changelog

## 0.1.0

Initial preview version.

- Native Rust tray utility with an animated top-centre battery island.
- Non-activating alerts, snooze, dismiss, reminder limits and recharge hysteresis.
- Live battery GET for the validated Corsair SLIPSTREAM `1B1C:1BDC` interface.
- Standard Bluetooth LE Battery service reader and an optional iCUE SDK provider.
- Separate Bluetooth and receiver workers; cached values cannot trigger alerts.
- Portable configuration, local reminder persistence and redacted diagnostics.
- Windows CI, reproducible verification and portable ZIP packaging with licenses
  and SHA-256 checksums.

Physical low-battery crossings and broader device compatibility remain unverified.
