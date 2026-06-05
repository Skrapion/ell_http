use std::os::raw::*;
use std::sync::{Mutex, OnceLock};

use base64::Engine as _;
use turso::*;
use windows::core::*;
use windows::Win32::Networking::WinHttp::*;

use crate::define_ell_http;
use crate::log;
use crate::log_value;
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

static STATUS_CALLBACK: OnceLock<Mutex<WINHTTP_STATUS_CALLBACK>> = OnceLock::new();

impl DbSetupFns {
    pub async fn ell_http_status_callback(conn: &turso::Connection) -> turso::Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS ell_http_status_callback (
                id INTEGER PRIMARY KEY,
                created_at INTEGER NOT NULL,
                hinternet INTEGER,
                dwcontext INTEGER,
                dwinternetstatus INTEGER,
                lpstatusinformation INTEGER,
                dwstatusinformationlength INTEGER,
                result INTEGER,
                consumed BOOLEAN DEFAULT FALSE NOT NULL)",
            (),
        ).await?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_http_status_callback ON ell_http_status_callback (
                hinternet, dwcontext, dwinternetstatus, lpstatusinformation, dwstatusinformationlength
            )",
            ()
        ).await?;

        Ok(())
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn ell_http_status_callback(
    hinternet: *mut c_void,
    dwcontext: usize,
    dwinternetstatus: u32,
    lpstatusinformation: *mut c_void,
    dwstatusinformationlength: u32
)
{
    log!(
        "ell_http_status_callback",
        hinternet = log_value!(hinternet: *mut c_void),
        dwcontext = log_value!(dwcontext: usize),
        dwinternetstatus = log_value!(dwinternetstatus: u32),
        lpstatusinformation = log_value!(lpstatusinformation: *mut c_void),
        dwstatusinformationlength = log_value!(dwstatusinformationlength: u32),
        result = Value::Null
    );

    let status_callback = STATUS_CALLBACK.get().unwrap().lock().unwrap();

    unsafe {
        status_callback.unwrap()(
            hinternet,
            dwcontext,
            dwinternetstatus,
            lpstatusinformation,
            dwstatusinformationlength
        );
    }
}

inventory::submit! {
    Replacement {
        rva: 0x0,
        replacement: None,
        setup: |conn| Box::pin(DbSetupFns::ell_http_status_callback(conn)),
    }
}

unsafe fn win_http_set_status_callback(
    hinternet: *mut c_void,
    lpfninternetcallback: WINHTTP_STATUS_CALLBACK,
    dwnotificationflags: u32,
    dwreserved: usize
) -> WINHTTP_STATUS_CALLBACK
{
    let mut status_callback = STATUS_CALLBACK.get_or_init(|| Mutex::new(None)).lock().unwrap();
    let last_status_callback = *status_callback;
    *status_callback = lpfninternetcallback;

    unsafe {
        let rc = WinHttpSetStatusCallback(
            hinternet,
            Some(ell_http_status_callback),
            dwnotificationflags,
            dwreserved
        );

        #[allow(unpredictable_function_pointer_comparisons)]
        if rc == Some(std::mem::transmute(!0usize)) {
            Some(std::mem::transmute(!0usize))
        } else {
            last_status_callback
        }
    }
}

define_ell_http! {
    0x0095CC88,
    ell_http_set_status_callback,
    win_http_set_status_callback,
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

unsafe fn win_http_query_headers(
    hrequest: *mut c_void,
    dwinfolevel: u32,
    pwszname: PCWSTR,
    lpbuffer: *mut c_void,
    lpdwbufferlength: *mut u32,
    lpdwindex: *mut u32,
) -> windows::core::Result<()>
{
    let lpbuffer_opt = if lpbuffer.is_null() {
        None
    } else {
        Some(lpbuffer)
    };

    unsafe {
        WinHttpQueryHeaders(
            hrequest,
            dwinfolevel,
            pwszname,
            lpbuffer_opt,
            lpdwbufferlength,
            lpdwindex
        )
    }
}

define_ell_http! {
    0x0095CC50,
    ell_http_query_headers,
    win_http_query_headers,
    (
        hrequest: (*mut c_void),
        dwinfolevel: u32,
        pwszname: PCWSTR,
        lpbuffer: (*mut c_void) as (
            TEXT, 
            *lpdwbufferlength, 
            (dwinfolevel & (WINHTTP_QUERY_FLAG_NUMBER | WINHTTP_QUERY_FLAG_SYSTEMTIME) != 0)
        ),
        lpdwbufferlength: (*mut u32),
        lpdwindex: (*mut u32),
    ) -> BOOL = (Result<()>),
    index on(
        hrequest,
        dwinfolevel,
        pwszname,
        lpbuffer,
        lpdwbufferlength,
        lpdwindex
    )
}

unsafe fn win_http_set_option(
    hinternet: *const c_void,
    dwoption: u32,
    lpbuffer: *mut c_void,
    dwbufferlength: u32
) -> windows::core::Result<()>
{
    let hinternet_opt = if hinternet.is_null() {
        None
    } else {
        Some(hinternet)
    };

    unsafe {
        let buffer_opt = if dwbufferlength == 0 {
            None
        } else {
            Some(
                std::slice::from_raw_parts(lpbuffer as *mut u8, dwbufferlength as usize).to_vec()
            )
        };
        WinHttpSetOption(hinternet_opt, dwoption, buffer_opt.as_deref())
    }
}

define_ell_http! {
    0x0095CC80,
    ell_http_set_option,
    win_http_set_option,
    (
        hinternet: (*const c_void),
        dwoption: u32,
        lpbuffer: (*mut c_void) as (TEXT, dwbufferlength, true),
        dwbufferlength: u32, 
    ) -> BOOL = (Result<()>),
    index on(
        hinternet,
        dwoption,
        lpbuffer,
        dwbufferlength
    )
}

unsafe fn win_http_query_option(
    hinternet: *mut c_void,
    dwoption: u32,
    lpbuffer: *mut c_void,
    dwbufferlength: *mut u32
) -> windows::core::Result<()>
{
    unsafe {
        let buffer_opt = if dwbufferlength.is_null() {
            None
        } else {
            Some(lpbuffer)
        };
        WinHttpQueryOption(hinternet, dwoption, buffer_opt, dwbufferlength)
    }
}

define_ell_http! {
    0x0095CC58,
    ell_http_query_option,
    win_http_query_option,
    (
        hinternet: (*mut c_void),
        dwoption: u32,
        lpbuffer: (*mut c_void) as (TEXT, *lpdwbufferlength, true),
        lpdwbufferlength: (*mut u32)
    ) -> BOOL = (Result<()>),
    index on(
        hinternet,
        dwoption,
        lpbuffer,
        lpdwbufferlength
    )
}

pub unsafe fn win_http_send_request (
    hrequest: *mut c_void,
    lpszheaders: PCWSTR,
    dwheaderslength: u32,
    lpoptional: *const c_void,
    dwoptionallength: u32,
    dwtotallength: u32,
    dwcontext: usize,
) -> windows::core::Result<()>
{
    let lpoptional_opt = if lpoptional.is_null() {
        None
    } else {
        Some(lpoptional)
    };

    unsafe {
        let lpszheaders_opt = if dwheaderslength == 0 {
            None
        } else {
            Some(
                std::slice::from_raw_parts(lpszheaders.0, dwheaderslength.try_into().unwrap()).to_vec()
            )
        };

        WinHttpSendRequest(
            hrequest,
            lpszheaders_opt.as_deref(),
            lpoptional_opt,
            dwoptionallength,
            dwtotallength,
            dwcontext
        )
    }
}

define_ell_http! {
    0x0095CC70,
    ell_http_send_request,
    win_http_send_request,
    (
        hrequest: (*mut c_void),
        lpszheaders: PCWSTR,
        dwheaderslength: u32,
        lpoptional: (*const c_void),
        dwoptionallength: u32,
        dwtotallength: u32,
        dwcontext: usize,
    ) -> BOOL = (Result<()>),
    index on(
        hrequest,
        lpszheaders,
        dwheaderslength,
        lpoptional,
        dwoptionallength,
        dwtotallength,
        dwcontext
    )
}

define_ell_http! {
    0x0095CC10,
    ell_http_close_handle,
    WinHttpCloseHandle,
    (
        hinternet: (*mut c_void)
    ) -> BOOL = (Result<()>)
}

unsafe fn win_http_write_data(
    hrequest: *mut c_void,
    lpbuffer: *const c_void,
    dwnumberofbytestowrite: u32,
    lpdwnumberofbyteswritten: *mut u32,
) -> windows::core::Result<()>
{
    let lpbuffer_opt = {
        if lpbuffer.is_null() {
            None
        } else {
            Some(lpbuffer)
        }
    };

    unsafe {
        WinHttpWriteData(
            hrequest,
            lpbuffer_opt,
            dwnumberofbytestowrite,
            lpdwnumberofbyteswritten,
        )
    }
}

define_ell_http! {
    0x0095CC98,
    ell_http_write_data,
    win_http_write_data,
    (
        hrequest: (*mut c_void),
        // NOTE: setting this to false outputs plaintext, which we may not actually want.
        lpbuffer: (*mut c_void) as (TEXT, dwnumberofbytestowrite, false),
        dwnumberofbytestowrite: u32,
        lpdwnumberofbyteswritten: (*mut u32),
    ) -> BOOL = (Result<()>),
    index on(
        hrequest,
        lpbuffer,
        dwnumberofbytestowrite,
        lpdwnumberofbyteswritten
    )
}
