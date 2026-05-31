use std::os::raw::*;
use std::fs::OpenOptions;
use std::io::Write;
use std::time::SystemTime;
use windows::core::*;
use windows::Win32::System::LibraryLoader::*;
use windows::Win32::System::Memory::*;
use windows::Win32::Networking::WinHttp::*;

type HINTERNET = *mut std::ffi::c_void; 
type DWORD = u32;
type LPCWSTR = *const u16;

const NO_PARAMS: &[(&str, String)] = &[];
pub fn log_function_call(func_name: &str, params: &[(&str, String)]) {
    let mut file = match OpenOptions::new()
        .create(true)
        .write(true)
        .append(true)
        .open("log.txt")
    {
        Ok(f) => f,
        Err(_) => return, // Fail silently to prevent crashing your DLL
    };

    // Get a simple timestamp
    let timestamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    // Format the parameters into a readable string: [param1: val1, param2: val2]
    let param_string: Vec<String> = params
        .iter()
        .map(|(name, val)| format!("{}: {}", name, val))
        .collect();
    
    let joined_params = param_string.join(", ");

    // Write to file: [1716943521] Called: MyFunction | Params: [a: 10, b: "test"]
    let _ = writeln!(
        file,
        "[{}] Called: {} | Params: [{}]",
        timestamp, func_name, joined_params
    );
}

#[macro_export]
macro_rules! log {
    ($func_name:expr, $($param_name:ident = $param_val:expr),* $(,)?) => {
        {
            let params = vec![
                $(
                    (stringify!($param_name), format!("{:?}", $param_val)),
                )*
            ];
            $crate::log_function_call($func_name, &params);
        }
    };
}

unsafe fn lp2str(ptr: *const u16) -> String {
    if ptr.is_null() {
        return String::new();
    }
    
    unsafe {
        PCWSTR::from_raw(ptr).display().to_string()
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn EllHttpOpen(
    psz_agent_w: LPCWSTR, 
    dw_access_type: WINHTTP_ACCESS_TYPE,
    psz_proxy_w: LPCWSTR, 
    psz_proxy_bypass_w: LPCWSTR,
    dw_flags: DWORD) -> HINTERNET
{
    unsafe {
        let result = WinHttpOpen(
            PCWSTR::from_raw(psz_agent_w), 
            dw_access_type, 
            PCWSTR::from_raw(psz_proxy_w),
            PCWSTR::from_raw(psz_proxy_bypass_w), 
            dw_flags);

        log!(
            "WinHttpOpen",
            psz_agent_w = lp2str(psz_agent_w), 
            dw_access_type = dw_access_type,
            psz_proxy_w = lp2str(psz_proxy_w),
            psz_proxy_bypass_w = lp2str(psz_proxy_bypass_w),
            result = result
        );

        result
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn EllHttpSetStatusCallback(
    hinternet: *mut c_void,
    lpfninternetcallback: WINHTTP_STATUS_CALLBACK,
    dwnotificationflags: u32,
    dwreserved: usize,
) -> WINHTTP_STATUS_CALLBACK 
{
    unsafe {
        let result = WinHttpSetStatusCallback(
            hinternet,
            lpfninternetcallback,
            dwnotificationflags,
            dwreserved
        );

        log!("WinHttpSetStatusCallback",
            hinternet = hinternet,
            lpfninternetcallback = lpfninternetcallback,
            dwnotificationflags = dwnotificationflags,
            dwreserved = dwreserved,
            result = result
        );

        result
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn EllHttpConnect(
    hsession: *mut c_void,
    pswzservername: LPCWSTR,
    nserverport: u16,
    dwreserved: u32,
) -> *mut c_void
{
    unsafe {
        let result = WinHttpConnect(
            hsession,
            PCWSTR::from_raw(pswzservername),
            nserverport,
            dwreserved
        );

        log!("WinHttpConnect",
            hsession = hsession,
            pswzservername = lp2str(pswzservername),
            nserverport = nserverport,
            dwreserved = dwreserved,
            result = result
        );

        result
    }
}

//////////////// Replacements
struct Replacement {
    original_rva: usize,
    replacement: usize,
}

fn replacements() -> Vec<Replacement> {
    vec![
        Replacement {
            original_rva: 0x0095CC30,
            replacement: EllHttpOpen as usize,
        },
        Replacement {
            original_rva: 0x0095CC88,
            replacement: EllHttpSetStatusCallback as usize,
        },
        Replacement {
            original_rva: 0x0095CC18,
            replacement: EllHttpConnect as usize,
        },
    ]
}

///////////////// Patching
unsafe fn patch_call(
    call_addr: *mut u8,
    target_addr: usize,
)
{
    unsafe {
        let next_instr = call_addr.add(5) as usize;

        let rel =
            (target_addr as isize - next_instr as isize) as i32;

        let mut old = PAGE_PROTECTION_FLAGS(0);

        let _ = VirtualProtect(
            call_addr as _,
            5,
            PAGE_EXECUTE_READWRITE,
            &mut old,
        );

        *call_addr = 0xE8;

        std::ptr::write_unaligned(
            call_addr.add(1) as *mut i32,
            rel,
        );

        let _ = VirtualProtect(
            call_addr as _,
            5,
            old,
            &mut old,
        );
    }
}

fn run_patch()
{
    unsafe {
        let exe = GetModuleHandleA(None).unwrap();
        let start = exe.0 as *const u8;
        let end = 0x1000000 as *const u8;//0x1173FFF as *const u8;

        let mut p = start;

        while p < end {
            if *p == 0xE8 {
                let disp =
                    std::ptr::read_unaligned(
                        p.add(1) as *const i32
                    );

                let address = p.add(5) as isize;

                let destination = address + disp as isize;

                for repl in replacements() {
                    if destination as usize == repl.original_rva {
                        patch_call(
                            p as *mut u8,
                            repl.replacement as usize,
                        );
                    }
                }
            }

            p = p.add(1);
        }
    }
}

use windows::Win32::Foundation::*;
use windows::Win32::System::SystemServices::*;

#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllMain(
    _hinst: HINSTANCE,
    reason: u32,
    _: *mut core::ffi::c_void,
) -> BOOL {
    if reason == DLL_PROCESS_ATTACH {
        run_patch();
    }

    TRUE
}
