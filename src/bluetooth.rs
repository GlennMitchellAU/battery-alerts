//! Windows PnP discovery plus live reads of the standard BLE Battery service.
//! Cached PnP percentages are labelled and never used to trigger alerts.
use crate::model::{Device, Snapshot};
use std::{
    mem::{size_of, zeroed},
    ptr,
};
use windows_sys::{
    core::GUID,
    Win32::{
        Devices::{Bluetooth::*, DeviceAndDriverInstallation::*, Properties::*},
        Foundation::{CloseHandle, DEVPROPKEY, GENERIC_READ, GENERIC_WRITE, INVALID_HANDLE_VALUE},
        Storage::FileSystem::{CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING},
    },
};

const BATTERY: DEVPROPKEY = DEVPROPKEY {
    fmtid: GUID::from_u128(0x104ea319_6ee2_4701_bd47_8ddbf425bbe5),
    pid: 2,
};
struct DeviceSet(HDEVINFO);
impl Drop for DeviceSet {
    fn drop(&mut self) {
        unsafe {
            SetupDiDestroyDeviceInfoList(self.0);
        }
    }
}
unsafe fn property(
    set: HDEVINFO,
    dev: &SP_DEVINFO_DATA,
    key: &DEVPROPKEY,
) -> Option<(u32, Vec<u8>)> {
    let mut kind = 0;
    let mut needed = 0;
    SetupDiGetDevicePropertyW(set, dev, key, &mut kind, ptr::null_mut(), 0, &mut needed, 0);
    if needed == 0 || needed > 65536 {
        return None;
    }
    let mut buffer = vec![0u8; needed as usize];
    if SetupDiGetDevicePropertyW(
        set,
        dev,
        key,
        &mut kind,
        buffer.as_mut_ptr(),
        needed,
        &mut needed,
        0,
    ) == 0
    {
        return None;
    }
    buffer.truncate(needed as usize);
    Some((kind, buffer))
}
fn decode(bytes: &[u8]) -> String {
    let data: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|v| u16::from_le_bytes([v[0], v[1]]))
        .take_while(|v| *v != 0)
        .collect();
    String::from_utf16_lossy(&data)
}
unsafe fn name(set: HDEVINFO, dev: &SP_DEVINFO_DATA) -> String {
    property(set, dev, &DEVPKEY_Device_FriendlyName)
        .or_else(|| property(set, dev, &DEVPKEY_Device_DeviceDesc))
        .map(|(_, b)| decode(&b))
        .unwrap_or_else(|| "Bluetooth device".into())
}
unsafe fn instance(set: HDEVINFO, dev: &SP_DEVINFO_DATA) -> String {
    let mut buffer = [0u16; 1024];
    if SetupDiGetDeviceInstanceIdW(set, dev, buffer.as_mut_ptr(), 1024, ptr::null_mut()) == 0 {
        return String::new();
    }
    String::from_utf16_lossy(&buffer[..buffer.iter().position(|v| *v == 0).unwrap_or(buffer.len())])
}
pub fn scan() -> Snapshot {
    unsafe { scan_inner() }
}
unsafe fn scan_inner() -> Snapshot {
    let mut snapshot = Snapshot::default();
    let set = DeviceSet(SetupDiGetClassDevsW(
        ptr::null(),
        ptr::null(),
        ptr::null_mut(),
        DIGCF_ALLCLASSES | DIGCF_PRESENT,
    ));
    if set.0 == INVALID_HANDLE_VALUE as isize {
        snapshot
            .status
            .push("Windows device discovery failed.".into());
        return snapshot;
    }
    let mut index = 0;
    loop {
        let mut dev: SP_DEVINFO_DATA = zeroed();
        dev.cbSize = size_of::<SP_DEVINFO_DATA>() as u32;
        if SetupDiEnumDeviceInfo(set.0, index, &mut dev) == 0 {
            break;
        }
        index += 1;
        let id = instance(set.0, &dev);
        if !(id.starts_with("BTHLE\\DEV_") || id.starts_with("BTHENUM\\DEV_")) {
            continue;
        }
        let battery = property(set.0, &dev, &BATTERY).and_then(|(kind, data)| {
            if kind == DEVPROP_TYPE_BYTE && data.len() == 1 && data[0] <= 100 {
                Some(data[0])
            } else {
                None
            }
        });
        snapshot.devices.push(Device {
            id: format!("windows:{id}"),
            name: name(set.0, &dev),
            battery,
            fresh: false,
            source: "Windows Bluetooth".into(),
            detail: if battery.is_some() {
                "Last Windows reading; awaiting a live battery read".into()
            } else {
                "No standard battery reading exposed by Windows".into()
            },
        });
    }
    // Enumerate Battery-service interfaces only. Each read stays off the UI thread.
    let battery_uuid = GUID::from_u128(0x0000180f_0000_1000_8000_00805f9b34fb);
    let interfaces = DeviceSet(SetupDiGetClassDevsW(
        &battery_uuid,
        ptr::null(),
        ptr::null_mut(),
        DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
    ));
    if interfaces.0 != INVALID_HANDLE_VALUE as isize {
        let mut index = 0;
        loop {
            let mut interface: SP_DEVICE_INTERFACE_DATA = zeroed();
            interface.cbSize = size_of::<SP_DEVICE_INTERFACE_DATA>() as u32;
            if SetupDiEnumDeviceInterfaces(
                interfaces.0,
                ptr::null(),
                &battery_uuid,
                index,
                &mut interface,
            ) == 0
            {
                break;
            }
            index += 1;
            let mut needed = 0;
            SetupDiGetDeviceInterfaceDetailW(
                interfaces.0,
                &interface,
                ptr::null_mut(),
                0,
                &mut needed,
                ptr::null_mut(),
            );
            if !(8..=65536).contains(&needed) {
                continue;
            }
            // u64 storage provides the alignment required by the native structure.
            let mut storage = vec![0u64; (needed as usize).div_ceil(8)];
            let detail = storage.as_mut_ptr() as *mut SP_DEVICE_INTERFACE_DETAIL_DATA_W;
            (*detail).cbSize = if cfg!(target_pointer_width = "64") {
                8
            } else {
                6
            };
            let mut dev: SP_DEVINFO_DATA = zeroed();
            dev.cbSize = size_of::<SP_DEVINFO_DATA>() as u32;
            if SetupDiGetDeviceInterfaceDetailW(
                interfaces.0,
                &interface,
                detail,
                needed,
                ptr::null_mut(),
                &mut dev,
            ) == 0
            {
                continue;
            }
            let path = (*detail).DevicePath.as_ptr();
            let service_id = instance(interfaces.0, &dev);
            let parent = property(interfaces.0, &dev, &DEVPKEY_Device_Parent)
                .map(|(_, bytes)| decode(&bytes))
                .unwrap_or_default();
            let device_index = snapshot.devices.iter().position(|d| {
                !parent.is_empty() && d.id.eq_ignore_ascii_case(&format!("windows:{parent}"))
            });
            let result = read_battery(path);
            if let Some(i) = device_index {
                let d = &mut snapshot.devices[i];
                match result {
                    Ok(value) => {
                        d.battery = Some(value);
                        d.fresh = true;
                        d.detail = "Live standard Bluetooth battery reading".into();
                    }
                    Err(reason) => {
                        d.detail = format!("Device unavailable for a live read ({reason}); last value is not used for alerts");
                    }
                }
            } else if let Ok(value) = result {
                snapshot.devices.push(Device {
                    id: format!("windows:{service_id}"),
                    name: name(interfaces.0, &dev),
                    battery: Some(value),
                    fresh: true,
                    source: "Bluetooth Battery service".into(),
                    detail: "Live standard Bluetooth battery reading".into(),
                });
            }
        }
    }
    let live = snapshot.devices.iter().filter(|d| d.fresh).count();
    snapshot.status.push(format!("Bluetooth: {live} device(s) with a live standard battery reading. Unsupported or cached readings cannot trigger alerts."));
    snapshot
}
unsafe fn read_battery(path: *const u16) -> Result<u8, String> {
    let handle = CreateFileW(
        path,
        GENERIC_READ | GENERIC_WRITE,
        FILE_SHARE_READ | FILE_SHARE_WRITE,
        ptr::null(),
        OPEN_EXISTING,
        0,
        ptr::null_mut(),
    );
    if handle == INVALID_HANDLE_VALUE {
        return Err("service access unavailable".into());
    }
    let result = (|| {
        let mut count = 0;
        BluetoothGATTGetCharacteristics(handle, ptr::null(), 0, ptr::null_mut(), &mut count, 0);
        if count == 0 || count > 128 {
            return Err("battery characteristic missing".into());
        }
        let mut chars: Vec<BTH_LE_GATT_CHARACTERISTIC> = (0..count).map(|_| zeroed()).collect();
        let hr = BluetoothGATTGetCharacteristics(
            handle,
            ptr::null(),
            count,
            chars.as_mut_ptr(),
            &mut count,
            0,
        );
        if hr < 0 {
            return Err(format!("characteristic read 0x{:08X}", hr as u32));
        }
        for c in chars.iter().take(count as usize) {
            let uuid = &c.CharacteristicUuid;
            let is_battery = if uuid.IsShortUuid {
                uuid.Value.ShortUuid == 0x2a19
            } else {
                let value = uuid.Value.LongUuid;
                value.data1 == 0x2a19
                    && value.data2 == 0
                    && value.data3 == 0x1000
                    && value.data4 == [0x80, 0, 0, 0x80, 0x5f, 0x9b, 0x34, 0xfb]
            };
            if !is_battery || !c.IsReadable {
                continue;
            }
            let mut buffer = [0u64; 8];
            let mut needed = 0;
            let value = buffer.as_mut_ptr() as *mut BTH_LE_GATT_CHARACTERISTIC_VALUE;
            let hr = BluetoothGATTGetCharacteristicValue(
                handle,
                c,
                size_of_val(&buffer) as u32,
                value,
                &mut needed,
                BLUETOOTH_GATT_FLAG_FORCE_READ_FROM_DEVICE,
            );
            if hr < 0 {
                return Err(format!("live read 0x{:08X}", hr as u32));
            }
            if (*value).DataSize >= 1 && (*value).Data[0] <= 100 {
                return Ok((*value).Data[0]);
            }
            return Err("invalid battery value".into());
        }
        Err("readable battery characteristic missing".into())
    })();
    CloseHandle(handle);
    result
}
