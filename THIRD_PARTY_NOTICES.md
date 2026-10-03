# Third-party notices

Battery Alerts source is MIT licensed. Third-party software retains its own terms.

## Rust dependencies and runtime

`Cargo.lock` records dependency versions and checksums. The portable build includes
the original dependency license texts in `licenses/`, including Unicode data
notices, and the Rust standard library copyright manifest and accompanying texts.
`scripts/licenses.ps1` collects these from the locked Cargo packages and installed
Rust toolchain. It fails if a package has no license file.

Most dependencies offer MIT or Apache-2.0 terms. `unicode-ident` also includes the
Unicode-3.0 license. See the packaged `licenses/INDEX.txt` and original texts for
the complete attribution of the particular build.

## Optional Corsair SDK

Corsair's iCUE SDK is proprietary software, distributed by Corsair under its SDK
EULA. The SDK DLL and documentation are **not included** in this repository or
standard packages. The optional setup script downloads Corsair's official SDK
4.0.84, verifies pinned archive and DLL hashes, and copies it locally beside the
app, together with Corsair's reference PDF containing its terms.

- [Official SDK documentation](https://corsairofficial.github.io/cue-sdk/)
- [Official SDK release](https://github.com/CorsairOfficial/cue-sdk/releases/tag/v4.0.84)

The independently written ABI bindings call only enumeration and battery-reading
functions. This project's MIT license does not grant rights to the Corsair DLL.

## Protocol references

These projects were consulted to understand receiver protocol values and Windows
HID report framing. Their source code is not bundled with Battery Alerts:

- [OpenLinkHub M75 AIR protocol reference](https://github.com/jurkovic-nikola/OpenLinkHub/blob/main/src/devices/m75AirW/m75AirW.go)
- [m75tool HID framing reference](https://github.com/XPookachu/m75tool/blob/main/bragi.py)

The receiver reader is an independent Rust implementation of the narrow battery
GET exchange. It does not include the reference projects' device-control features.

CORSAIR, SLIPSTREAM and iCUE are trademarks of their respective owner. Battery
Alerts is an independent project and is not affiliated with or endorsed by Corsair.
