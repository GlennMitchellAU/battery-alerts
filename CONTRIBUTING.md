# Contributing

Focused bug reports, device compatibility reports and pull requests are welcome.

## Reports

Read [device compatibility](docs/compatibility.md) first. Include the device model,
connection type, Windows version, app version and whether iCUE is running. A USB
receiver's vendor/product ID is useful; its serial number is unnecessary.

`--diagnose` hides identifiers by default. Review device names before posting its
output: they can be personal. Do not attach unredacted output from
`--include-identifiers`, hardware inventory dumps, serial numbers, Bluetooth
addresses, local user-profile paths, or the persisted `alerts.json` file.

Report security problems privately using [SECURITY.md](SECURITY.md).

## Development

1. Install Rust through rustup and the x64 MSVC C++ Build Tools with a Windows SDK.
2. Create a focused branch and keep provider changes scoped to known devices.
3. Run `./scripts/verify.ps1`, then `./scripts/package.ps1 -SkipVerification`.
4. Explain the behaviour, validation and remaining hardware checks in the PR.

Keep the app small and offline. New providers must distinguish live, cached and
unknown readings. Unknown must never become 0%. Vendor HID support needs verified
interface descriptors, bounded IO, response validation and a documented read-only
query. Do not broaden a receiver allowlist without physical evidence.

For UI changes, verify the foreground window before and after an automatic alert,
plus placement at multiple display scales. Builds and simulated previews do not
prove a physical battery threshold crossing. See [validation](docs/validation.md).

Contributions are licensed under this repository's MIT license.
