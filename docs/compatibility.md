# Device compatibility

Support depends on the battery interface exposed by a device. Bluetooth pairing
alone does not guarantee a readable battery percentage.

| Device path | Current support | Physical evidence |
| --- | --- | --- |
| Corsair SLIPSTREAM receiver `USB 1B1C:1BDC`, mouse child 1 | Direct native HID battery GET; iCUE can be closed | Live readings of 46% and 45% on one receiver |
| Bluetooth LE standard Battery service `180F`, readable characteristic `2A19` | Windows GATT live read | Implementation present; a successful live reading on another physical device is still needed |
| Other Corsair wireless devices exposed by iCUE | Optional official SDK provider | SDK handshake succeeded on one installation; no mouse was exposed there |
| Bluetooth devices with a cached Windows battery property | Status only, labelled last known | An unavailable controller's cached 95% was excluded from alerts |
| Vendor-specific earbuds and other USB receivers | No provider yet | No claim of support |

Battery levels may be coarse or absent. Left/right/case earbud batteries are not
separately supported. The app cannot estimate charge from a device that supplies
no useful battery reading.

## Direct SLIPSTREAM path

The reader requires all of these conditions before issuing a query:

- Vendor/product/interface: `VID_1B1C&PID_1BDC&MI_01`.
- HID usage page `FF42`, usage `1`, 65-byte input/output reports.
- Mouse child 1 only, battery GET request; no control commands.
- Matching response, successful status and a battery value between 0 and 1000
  tenths of a percent.

IO is bounded and cancelled before native buffers leave scope. Other product IDs
and receiver child slots are deliberately excluded until physically validated.
The receiver product ID does not establish support for every mouse paired with a
receiver that happens to share that ID.

Direct access pauses while `iCUE.exe` is running to avoid competing for the
receiver's responses. If iCUE is closed, the direct reader needs no Corsair DLL.

## Optional iCUE SDK

For SDK-supported devices, use iCUE 4.31 or newer with SDK access enabled and the
same privilege level as Battery Alerts. iCUE must expose the device and its battery
property; an SDK connection by itself does not establish device support.

After building the app, optionally run:

```powershell
./scripts/install-corsair-sdk.ps1 -AppDirectory ./dist
```

Restart the app after installing the DLL. The script verifies the pinned official
SDK 4.0.84 and includes Corsair's terms locally. Standard CI packages omit the SDK.
For the validated direct receiver, leaving iCUE closed is sufficient.

## Troubleshooting

Open **Device battery status** from the tray. A green live value is eligible for
alerts; grey values and N/A are not. Wake or reconnect a sleeping peripheral and
allow another polling cycle. Bluetooth timeouts may take longer than a cycle,
but run independently of receiver polling and the UI.

```powershell
./dist/battery-alerts.exe --diagnose | Out-String
./dist/battery-alerts.exe --diagnose --receiver-only | Out-String
```

Diagnostics hide identifiers by default. Review device names before sharing.
If a device is unsupported, open a compatibility issue with its model, connection
type and generic USB vendor/product ID. Device serials and Bluetooth addresses
are unnecessary.
