# Validation

## Development observations (3 October 2026)

On one Windows 11 x64 desktop:

- The native reader obtained live 46% and 45% readings from a Corsair SLIPSTREAM
  `1B1C:1BDC` receiver while iCUE was closed.
- The optional SDK connected to an existing iCUE installation but exposed no
  mouse. SDK-supported mouse readings remain unverified.
- An unavailable controller's cached 95% value was excluded from alerts. Earbuds
  without a standard reading were shown as unavailable.
- A native foreground-window check remained unchanged when the island appeared.
  The window's topmost and no-activate styles were verified.
- Placement was centred at the primary work-area top. At 125% display scaling,
  the expanded panel measured 625 by 205 physical pixels.
- A synthetic Dismiss click closed the island. The simulated warning and real
  device-status panel were visually inspected.

The initial release executable measured approximately 268 KiB, and the running
monitor used approximately 15 MiB working-set memory during local checks. These
are observations of one development build, not fixed package sizes or performance
guarantees. License documentation adds to the portable ZIP size.

Local screenshots and machine-specific verification output are excluded from the
public repository. The README uses an illustration with a simulated percentage.

## Automated gate

Run `./scripts/verify.ps1`. It checks formatting, lint, alert policy, receiver
response validation, diagnostic identifier redaction, release compilation and
the version command. Policy tests exercise threshold entry, immediate critical
escalation, reminder limits, cached/unknown/disconnected readings, recharge
hysteresis and restart persistence.

The GitHub Build workflow runs that gate from a fresh checkout and packages only
the native executable, configuration, usage notes and license texts. It produces
a ZIP and SHA-256 checksum. Physical device and interactive display behaviour
cannot be established by hosted CI.

## Physical acceptance checklist

Before promoting the preview to a wider release, record the device model,
connection type, Windows version and result for these checks:

1. Use pointer movement, buttons and device sleep/wake normally while monitoring.
2. Observe a real battery crossing 20%, then 10%; confirm both alerts appear.
3. Charge above 25%, then confirm a later low battery rearms the warning.
4. Type in another application during an automatic alert and while clicking
   Snooze/Dismiss; confirm keyboard focus remains there.
5. Test 100%, 125% and 150% scaling, multiple monitors, a primary-monitor change
   and a taskbar at the top of the screen.
6. Test a Bluetooth LE device that supplies a live standard battery reading.
7. If adding a startup shortcut, confirm the tray icon returns after sign-in.

Real threshold crossings and the broader display/device matrix have not yet been
observed. Simulated UI and parser/policy tests do not substitute for these checks.

## Release preparation

Run `./scripts/package.ps1` from a clean checkout and confirm the exact commit's
Build workflow is green. A tag matching `Cargo.toml` (for example `v0.1.0`) runs
verification again and creates a **draft** release with a portable ZIP and
checksum. Review that draft and record physical validation before publishing it.
The workflow does not publish a draft automatically.
