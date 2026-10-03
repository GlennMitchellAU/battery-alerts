//! Read-only bindings for Corsair's official iCUE SDK 4.0.84.
//! ABI source: CorsairOfficial/cue-sdk, src/include/iCUESDK.h.
use crate::model::Device;
use std::{
    ffi::{c_char, c_void},
    path::Path,
    ptr,
    sync::atomic::{AtomicI32, Ordering},
};
use windows_sys::Win32::{
    Foundation::{FreeLibrary, HMODULE},
    System::LibraryLoader::{
        GetProcAddress, LoadLibraryExW, LOAD_LIBRARY_SEARCH_DEFAULT_DIRS,
        LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR,
    },
};

#[repr(C)]
#[derive(Clone, Copy)]
struct DeviceInfo {
    kind: i32,
    id: [c_char; 128],
    serial: [c_char; 128],
    model: [c_char; 128],
    led_count: i32,
    channel_count: i32,
}
#[repr(C)]
struct Filter {
    mask: i32,
}
#[repr(C)]
struct StateChanged {
    state: i32,
    versions: [i32; 9],
}
#[repr(C)]
union Value {
    int32: i32,
    _alignment_and_size: [u64; 2],
}
#[repr(C)]
struct Property {
    kind: i32,
    value: Value,
}
type Connect = unsafe extern "C" fn(
    unsafe extern "C" fn(*mut c_void, *const StateChanged),
    *mut c_void,
) -> i32;
type Disconnect = unsafe extern "C" fn() -> i32;
type GetDevices = unsafe extern "C" fn(*const Filter, i32, *mut DeviceInfo, *mut i32) -> i32;
type Read = unsafe extern "C" fn(*const c_char, i32, u32, *mut Property) -> i32;
type Free = unsafe extern "C" fn(*mut Property) -> i32;
static SESSION: AtomicI32 = AtomicI32::new(1);
unsafe extern "C" fn on_state(_: *mut c_void, event: *const StateChanged) {
    if !event.is_null() {
        SESSION.store((*event).state, Ordering::Relaxed);
    }
}
pub struct Corsair {
    module: HMODULE,
    disconnect: Disconnect,
    devices: GetDevices,
    read: Read,
    free: Free,
}
impl Corsair {
    pub fn load(path: &Path) -> Result<Self, String> {
        unsafe {
            let path: Vec<u16> = path
                .as_os_str()
                .to_string_lossy()
                .encode_utf16()
                .chain(Some(0))
                .collect();
            let module = LoadLibraryExW(
                path.as_ptr(),
                ptr::null_mut(),
                LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS,
            );
            if module.is_null() {
                return Err(
                    "Corsair: optional SDK not installed. See docs/compatibility.md for setup."
                        .into(),
                );
            }
            macro_rules! symbol {
                ($name:literal, $ty:ty) => {{
                    let Some(p) = GetProcAddress(module, concat!($name, "\0").as_ptr()) else {
                        FreeLibrary(module);
                        return Err(concat!("Missing Corsair SDK function: ", $name).into());
                    };
                    std::mem::transmute::<unsafe extern "system" fn() -> isize, $ty>(p)
                }};
            }
            let connect = symbol!("CorsairConnect", Connect);
            let sdk = Self {
                module,
                disconnect: symbol!("CorsairDisconnect", Disconnect),
                devices: symbol!("CorsairGetDevices", GetDevices),
                read: symbol!("CorsairReadDeviceProperty", Read),
                free: symbol!("CorsairFreeProperty", Free),
            };
            SESSION.store(2, Ordering::Relaxed);
            let error = connect(on_state, ptr::null_mut());
            if error != 0 {
                return Err(format!("Corsair SDK connection error {error}."));
            }
            Ok(sdk)
        }
    }
    pub fn scan(&self) -> (Vec<Device>, String) {
        let state = SESSION.load(Ordering::Relaxed);
        if state != 6 {
            let reason = match state {
                2 => "Connecting to iCUE",
                3 => "iCUE is not running",
                4 => "iCUE SDK access is disabled",
                5 => "iCUE connection lost",
                _ => "iCUE is unavailable",
            };
            return (
                Vec::new(),
                format!("Corsair: {reason}. iCUE 4.31+ and SDK access are required."),
            );
        }
        unsafe {
            let mut info: [DeviceInfo; 64] = std::mem::zeroed();
            let mut count = 0;
            let error = (self.devices)(
                &Filter {
                    mask: 1 | 2 | 8 | 0x1000,
                },
                64,
                info.as_mut_ptr(),
                &mut count,
            );
            if error != 0 {
                return (
                    Vec::new(),
                    format!("Corsair: device enumeration error {error}."),
                );
            }
            let mut devices = Vec::new();
            for d in info.iter().take(count.clamp(0, 64) as usize) {
                let mut prop: Property = std::mem::zeroed();
                let error = (self.read)(d.id.as_ptr(), 9, 0, &mut prop);
                let battery = if error == 0 && prop.kind == 1 {
                    u8::try_from(prop.value.int32).ok().filter(|v| *v <= 100)
                } else {
                    None
                };
                if error == 0 {
                    (self.free)(&mut prop);
                }
                devices.push(Device {
                    id: format!("corsair:{}", string(&d.id)),
                    name: string(&d.model),
                    battery,
                    fresh: battery.is_some(),
                    source: "Corsair iCUE".into(),
                    detail: if battery.is_some() {
                        "Battery reported by iCUE".into()
                    } else {
                        format!("Battery unavailable from iCUE (error {error})")
                    },
                });
            }
            let reported = devices.iter().filter(|d| d.battery.is_some()).count();
            (
                devices,
                format!("Corsair: connected to iCUE; {reported} device(s) report battery."),
            )
        }
    }
}
fn string(value: &[c_char]) -> String {
    String::from_utf8_lossy(
        &value
            .iter()
            .take_while(|v| **v != 0)
            .map(|v| *v as u8)
            .collect::<Vec<_>>(),
    )
    .into_owned()
}
impl Drop for Corsair {
    fn drop(&mut self) {
        unsafe {
            (self.disconnect)();
            FreeLibrary(self.module);
        }
    }
}
