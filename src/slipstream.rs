//! Narrow native reader for the user's 1B1C:1BDC SLIPSTREAM receiver.
//! Protocol facts: Bragi GET=02, BatteryLevel=000F, child 1 route=09,
//! uint16 little-endian level in tenths of a percent. Only GET is sent.
//! These were researched in OpenLinkHub's m75AirW driver and confirmed on
//! this receiver with a successful 460/10 battery response.
use crate::{
    model::{Device, Snapshot},
    wide,
};
use std::{
    mem::{size_of, zeroed},
    ptr,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Devices::{DeviceAndDriverInstallation::*, HumanInterfaceDevice::*},
    Foundation::*,
    Storage::FileSystem::*,
    System::{Diagnostics::ToolHelp::*, Threading::*, IO::*},
};
struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
struct Set(HDEVINFO);
impl Drop for Set {
    fn drop(&mut self) {
        unsafe {
            SetupDiDestroyDeviceInfoList(self.0);
        }
    }
}
pub fn icue_running() -> bool {
    unsafe {
        let handle = Handle(CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0));
        if handle.0 == INVALID_HANDLE_VALUE {
            return true;
        }
        let mut entry: PROCESSENTRY32W = zeroed();
        entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
        if Process32FirstW(handle.0, &mut entry) == 0 {
            return false;
        }
        loop {
            let name = String::from_utf16_lossy(
                &entry.szExeFile[..entry.szExeFile.iter().position(|v| *v == 0).unwrap_or(260)],
            );
            if name.eq_ignore_ascii_case("icue.exe") {
                return true;
            }
            if Process32NextW(handle.0, &mut entry) == 0 {
                break;
            }
        }
        false
    }
}
pub fn scan() -> Snapshot {
    let mut result = Snapshot::default();
    if icue_running() {
        result.status.push("SLIPSTREAM: direct access paused while iCUE is running; using the official SDK instead.".into());
        return result;
    }
    match unsafe { read_receivers() } {
        Ok(devices) => {
            result.status.push(format!(
                "SLIPSTREAM: {} receiver(s) queried directly without iCUE.",
                devices.len()
            ));
            result.devices = devices;
        }
        Err(error) => result.status.push(format!("SLIPSTREAM: {error}")),
    }
    result
}
unsafe fn read_receivers() -> Result<Vec<Device>, String> {
    let mut guid = zeroed();
    HidD_GetHidGuid(&mut guid);
    let set = Set(SetupDiGetClassDevsW(
        &guid,
        ptr::null(),
        ptr::null_mut(),
        DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
    ));
    if set.0 == INVALID_HANDLE_VALUE as isize {
        return Err("HID discovery unavailable".into());
    }
    let mut devices = Vec::new();
    let mut index = 0;
    loop {
        let mut interface: SP_DEVICE_INTERFACE_DATA = zeroed();
        interface.cbSize = size_of::<SP_DEVICE_INTERFACE_DATA>() as u32;
        if SetupDiEnumDeviceInterfaces(set.0, ptr::null(), &guid, index, &mut interface) == 0 {
            break;
        }
        index += 1;
        let mut needed = 0;
        SetupDiGetDeviceInterfaceDetailW(
            set.0,
            &interface,
            ptr::null_mut(),
            0,
            &mut needed,
            ptr::null_mut(),
        );
        if !(8..=65536).contains(&needed) {
            continue;
        }
        let mut storage = vec![0u64; (needed as usize).div_ceil(8)];
        let detail = storage.as_mut_ptr() as *mut SP_DEVICE_INTERFACE_DETAIL_DATA_W;
        (*detail).cbSize = if cfg!(target_pointer_width = "64") {
            8
        } else {
            6
        };
        if SetupDiGetDeviceInterfaceDetailW(
            set.0,
            &interface,
            detail,
            needed,
            ptr::null_mut(),
            ptr::null_mut(),
        ) == 0
        {
            continue;
        }
        let path_units =
            std::slice::from_raw_parts((*detail).DevicePath.as_ptr(), (needed as usize - 4) / 2);
        let path = String::from_utf16_lossy(
            &path_units[..path_units
                .iter()
                .position(|v| *v == 0)
                .unwrap_or(path_units.len())],
        );
        if !path
            .to_ascii_lowercase()
            .contains("vid_1b1c&pid_1bdc&mi_01")
        {
            continue;
        }
        let path_w = wide(&path);
        let handle = Handle(CreateFileW(
            path_w.as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_OVERLAPPED,
            ptr::null_mut(),
        ));
        if handle.0 == INVALID_HANDLE_VALUE {
            return Err("receiver access unavailable".into());
        }
        let mut preparsed = 0isize;
        if !HidD_GetPreparsedData(handle.0, &mut preparsed) {
            return Err("receiver descriptor unavailable".into());
        }
        let mut caps: HIDP_CAPS = zeroed();
        let status = HidP_GetCaps(preparsed, &mut caps);
        HidD_FreePreparsedData(preparsed);
        if status < 0
            || caps.UsagePage != 0xff42
            || caps.Usage != 1
            || caps.InputReportByteLength != 65
            || caps.OutputReportByteLength != 65
        {
            continue;
        }
        let mut serial = [0u16; 128];
        HidD_GetSerialNumberString(
            handle.0,
            serial.as_mut_ptr() as *mut _,
            size_of_val(&serial) as u32,
        );
        let serial = String::from_utf16_lossy(
            &serial[..serial.iter().position(|v| *v == 0).unwrap_or(serial.len())],
        );
        let mut request = [0u8; 65];
        request[1..5].copy_from_slice(&[0x09, 0x02, 0x0f, 0x00]);
        let read = (|| {
            io(handle.0, &mut request, true, 500)?;
            let deadline = Instant::now() + Duration::from_millis(700);
            loop {
                let remaining = deadline
                    .saturating_duration_since(Instant::now())
                    .as_millis() as u32;
                if remaining == 0 {
                    return Err(
                        "battery request timed out; mouse may be asleep or disconnected".into(),
                    );
                }
                let mut report = [0u8; 65];
                let count = io(handle.0, &mut report, false, remaining)?;
                if count < 6 || report[0] != 0 || report[1] != 1 || report[2] != 2 {
                    continue;
                }
                return decode(&report[..count]);
            }
        })();
        let (battery, detail) = match read {
            Ok(level) => (
                Some(level),
                "Live battery reading through the SLIPSTREAM receiver".into(),
            ),
            Err(reason) => (None, reason),
        };
        devices.push(Device {
            id: format!(
                "slipstream:{}",
                if serial.is_empty() { &path } else { &serial }
            ),
            name: "CORSAIR wireless mouse".into(),
            battery,
            fresh: battery.is_some(),
            source: "SLIPSTREAM receiver".into(),
            detail,
        });
    }
    Ok(devices)
}
unsafe fn io(
    handle: HANDLE,
    buffer: &mut [u8],
    write: bool,
    timeout: u32,
) -> Result<usize, String> {
    let event = Handle(CreateEventW(ptr::null(), 1, 0, ptr::null()));
    if event.0.is_null() {
        return Err("could not create receiver IO event".into());
    }
    let mut overlapped: OVERLAPPED = zeroed();
    overlapped.hEvent = event.0;
    let mut transferred = 0;
    let ok = if write {
        WriteFile(
            handle,
            buffer.as_ptr(),
            buffer.len() as u32,
            &mut transferred,
            &mut overlapped,
        )
    } else {
        ReadFile(
            handle,
            buffer.as_mut_ptr(),
            buffer.len() as u32,
            &mut transferred,
            &mut overlapped,
        )
    };
    if ok == 0 && GetLastError() != ERROR_IO_PENDING {
        return Err(format!("receiver IO error {}", GetLastError()));
    }
    if ok == 0 && WaitForSingleObject(event.0, timeout) != WAIT_OBJECT_0 {
        // Drain cancellation before OVERLAPPED and its backing buffer leave scope.
        CancelIoEx(handle, &overlapped);
        GetOverlappedResult(handle, &overlapped, &mut transferred, 1);
        return Err("battery request timed out; mouse may be asleep or disconnected".into());
    }
    if GetOverlappedResult(handle, &overlapped, &mut transferred, 0) == 0 {
        return Err(format!("receiver completion error {}", GetLastError()));
    }
    Ok(transferred as usize)
}
fn decode(report: &[u8]) -> Result<u8, String> {
    if report.len() < 6 || report[0..3] != [0, 1, 2] {
        return Err("unmatched battery response".into());
    }
    if report[3] != 0 {
        return Err(format!(
            "receiver could not read battery (status {})",
            report[3]
        ));
    }
    let tenths = u16::from_le_bytes([report[4], report[5]]);
    if tenths > 1000 {
        return Err("receiver returned an invalid battery level".into());
    }
    Ok((tenths / 10) as u8)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepts_verified_report_and_valid_extremes() {
        assert_eq!(decode(&[0, 1, 2, 0, 0xcc, 1]).unwrap(), 46);
        assert_eq!(decode(&[0, 1, 2, 0, 0, 0]).unwrap(), 0);
        assert_eq!(decode(&[0, 1, 2, 0, 0xe8, 3]).unwrap(), 100);
    }
    #[test]
    fn rejects_short_failed_other_channel_and_out_of_range_reports() {
        for report in [
            &[0, 1, 2][..],
            &[0, 1, 2, 5, 0, 0],
            &[0, 2, 2, 0, 0, 0],
            &[0, 1, 2, 0, 0xe9, 3],
        ] {
            assert!(decode(report).is_err());
        }
    }
}
