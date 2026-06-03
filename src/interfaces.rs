use std::os::raw::*;

use turso::*;
use windows::core::*;
use windows::Win32::Networking::WinHttp::*;

use crate::define_ell_http;
use crate::log;
use crate::log::*;
use crate::interface_reg::*;

define_ell_http! {
    0x0095CC30,
    ell_http_open,
    WinHttpOpen,
    (
        pszagentw: PCWSTR,
        dwaccesstype: WINHTTP_ACCESS_TYPE,
        pszproxyw: PCWSTR,
        pszproxybypassw: PCWSTR,
        dwflags: u32
    ) -> (*mut c_void)
}

define_ell_http! {
    0x0095CC88,
    ell_http_set_status_callback,
    WinHttpSetStatusCallback,
    (
        hinternet: (*mut c_void),
        lpfninternetcallback: WINHTTP_STATUS_CALLBACK,
        dwnotificationflags: u32,
        dwreserved: usize
    ) -> WINHTTP_STATUS_CALLBACK
}

define_ell_http! {
    0x0095CC18,
    ell_http_connect,
    WinHttpConnect,
    (
        hsession: (*mut c_void),
        pswzservername: PCWSTR,
        nserverport: u16,
        dwreserved: u32,
    ) -> (*mut c_void),
    index on(hsession, pswzservername, nserverport)
}

define_ell_http! {
    0x0095CC38,
    ell_http_open_request,
    WinHttpOpenRequest,
    (
        hconnect: (*mut c_void),
        lpszverb: PCWSTR,
        lpszobjectname: PCWSTR,
        lpszversion: PCWSTR,
        lpszreferrer: PCWSTR,
        lplpszaccepttypes: (*const PCWSTR),
        dwflags: WINHTTP_OPEN_REQUEST_FLAGS,
    ) -> (*mut c_void),
    index on(
        hconnect, 
        lpszverb, 
        lpszobjectname, 
        lpszversion, 
        lpszreferrer, 
        lplpszaccepttypes, 
        dwflags
    )
}

define_ell_http! {
    0x0095CC90,
    ell_http_set_timeouts,
    WinHttpSetTimeouts,
    (
        hinternet: (*mut c_void),
        nresolvetimeout: i32,
        nconnecttimeout: i32,
        nsendtimeout: i32,
        nreceivetimeout: i32,
    ) -> BOOL = (Result<()>),
    index on(
        hinternet,
        nresolvetimeout,
        nconnecttimeout,
        nsendtimeout,
        nreceivetimeout
    )
}

fn win_http_add_request_headers(
    hrequest: *mut c_void,
    lpszheaders: PCWSTR,
    dwheaderslength: u32,
    dwmodifiers: u32
) -> windows::core::Result<()> {

    unsafe {
        let headers = if dwheaderslength == -1i32 as u32 {
            let mut len = 0;

            while *lpszheaders.0.add(len) != 0 {
                len += 1;
            }

            std::slice::from_raw_parts(lpszheaders.0, len).to_vec()
        } else {
            std::slice::from_raw_parts(lpszheaders.0, dwheaderslength.try_into().unwrap()).to_vec()
        };

        WinHttpAddRequestHeaders(
            hrequest,
            &headers,
            dwmodifiers
        )
    }
}

define_ell_http! {
    0x0095CC08,
    ell_http_add_request_headers,
    win_http_add_request_headers,
    (
        hrequest: (*mut c_void),
        lpszheaders: PCWSTR,
        dwheaderslength: u32, 
        dwmodifiers: u32
    ) -> BOOL = (Result<()>),
    index on(
        hrequest,
        lpszheaders,
        dwheaderslength,
        dwmodifiers
    )
}
