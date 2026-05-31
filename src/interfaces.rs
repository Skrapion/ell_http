use std::os::raw::*;

use windows::core::*;
use windows::Win32::Networking::WinHttp::*;

use crate::log;
use log::*;

/// Replacements
pub struct Replacement {
    pub original_rva: usize,
    pub replacement: usize,
}

pub fn replacements() -> Vec<Replacement> {
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

/// Interfaces
type LPCWSTR = *const u16;

#[unsafe(no_mangle)]
pub extern "system" fn EllHttpOpen(
    pszagentw: LPCWSTR,
    dwaccesstype: WINHTTP_ACCESS_TYPE,
    pszproxyw: LPCWSTR,
    pszproxybypassw: LPCWSTR,
    dwflags: u32,
) -> *mut c_void
{
    unsafe {
        let result = WinHttpOpen(
            PCWSTR::from_raw(pszagentw), 
            dwaccesstype,
            PCWSTR::from_raw(pszproxyw),
            PCWSTR::from_raw(pszproxybypassw), 
            dwflags);

        log!(
            "WinHttpOpen",
            pszagentw = lp2str(pszagentw), 
            dwaccesstype = dwaccesstype,
            pszproxyw = lp2str(pszproxyw),
            pszproxybypassw = lp2str(pszproxybypassw),
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
