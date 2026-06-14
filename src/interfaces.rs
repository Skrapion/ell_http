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
    ) -> (*mut c_void),
    index on(
        pszagentw,
        dwaccesstype,
        pszproxyw,
        pszproxybypassw,
        dwflags
    )
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
    ) -> WINHTTP_STATUS_CALLBACK,
    index on(
        hinternet,
        dwnotificationflags
    )
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

use popout::*;

fn copy_to_mut_c_void_16(source_str: &str, dest_ptr: *mut c_void, max_len: usize) -> u32 {
    // 1. Encode string to UTF-16 and add a null terminator
    let mut utf16_vec: Vec<u16> = source_str.encode_utf16().collect();
    utf16_vec.push(0);

    // 2. Ensure we do not write past the allocated destination size
    let bytes_to_copy = std::cmp::min(utf16_vec.len(), max_len) * std::mem::size_of::<u16>();

    unsafe {
        // 3. Copy the raw memory to the *mut c_void
        std::ptr::copy_nonoverlapping(
            utf16_vec.as_ptr() as *const c_void,
            dest_ptr,
            bytes_to_copy,
        );
    }

    bytes_to_copy.try_into().unwrap()
}

fn copy_to_mut_c_void_8(source_str: &str, dest_ptr: *mut c_void, max_len: usize) -> u32 {
    // 1. Convert to CString to ensure a null-terminated byte sequence
    // This will fail if the internal string contains an unexpected null byte
    if let Ok(c_str) = std::ffi::CString::new(source_str) {
        let bytes = c_str.as_bytes_with_nul();
        
        // 2. Bound check the transfer size against your allocated memory
        let bytes_to_copy = std::cmp::min(bytes.len(), max_len);

        unsafe {
            // 3. Copy the raw memory to the *mut c_void
            std::ptr::copy_nonoverlapping(
                bytes.as_ptr() as *const c_void,
                dest_ptr,
                bytes_to_copy,
            );
        }

        bytes_to_copy.try_into().unwrap()
    } else {
        0
    }
}

fn should_inject() -> bool {
    std::path::Path::new("inject.txt").exists()
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

    // If I call this after WinHttpQueryHeaders it crashes. I'm not sure why.
    let inject = should_inject();

    unsafe {
        let rc = WinHttpQueryHeaders(
            hrequest,
            dwinfolevel,
            pwszname,
            lpbuffer_opt,
            lpdwbufferlength,
            lpdwindex
        );

        if inject {
            if lpbuffer.is_null() {
                //*lpdwbufferlength = *lpdwbufferlength + 1000;
            } else if rc.is_ok() {
                let byte_slice = std::slice::from_raw_parts(
                    lpbuffer as *const u16, 
                    (*lpdwbufferlength / 2).try_into().unwrap()
                );

                let mut data = String::from_utf16(byte_slice).unwrap().to_string();

                popout::create_window(
                    |ui| {
                        ui.label("Header:");
                        ui.separator();
                        ui.text_edit_multiline(&mut data);
                        if ui.button("Done").clicked() {
                            return Some(());
                        }
                        None
                    },
                    WindowAttributes::default()
                        .with_title("Header")
                        .with_inner_size(LogicalSize::new(400, 400)),
                )
                .unwrap();

                let byte_len = copy_to_mut_c_void_16(&data, lpbuffer, (*lpdwbufferlength / 2).try_into().unwrap());
                *lpdwbufferlength = byte_len;
            }
        }

        rc
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
            if dwinfolevel & (WINHTTP_QUERY_FLAG_NUMBER | WINHTTP_QUERY_FLAG_SYSTEMTIME) != 0 {
                Encoding::Base64
            } else {
                Encoding::Utf16
            }
        ),
        lpdwbufferlength: (*mut u32),
        lpdwindex: (*mut u32),
    ) -> BOOL = (Result<()>),
    index on(
        hrequest,
        dwinfolevel,
        pwszname,
        lpbuffer,
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
        lpbuffer: (*mut c_void) as (TEXT, dwbufferlength, Encoding::Base64),
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
        lpbuffer: (*mut c_void) as (TEXT, *lpdwbufferlength, Encoding::Base64),
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
    ) -> BOOL = (Result<()>),
    index on(
        hinternet
    )
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
        // NOTE: There may be cases where we don't want this encoding
        lpbuffer: (*mut c_void) as (TEXT, dwnumberofbytestowrite, Encoding::Utf8),
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

define_ell_http! {
    0x0095CC68,
    ell_http_receive_response,
    WinHttpReceiveResponse,
    (
        hrequest: (*mut c_void),
        lpreserved: (*mut c_void),
    ) -> BOOL = (Result<()>),
    index on(
        hrequest
    )
}

unsafe fn win_http_query_data_available(
    hrequest: *mut c_void,
    lpdwnumberofbytesavailable: *mut u32,
) -> windows::core::Result<()>
{
    unsafe {
        let rc = WinHttpQueryDataAvailable(hrequest, lpdwnumberofbytesavailable);

        /*
        if should_inject() {
            if rc.is_ok() {
                *lpdwnumberofbytesavailable = *lpdwnumberofbytesavailable + 1000;
            }
        }
        */

        rc
    }
}

define_ell_http! {
    0x0095CC48,
    ell_http_query_data_available,
    win_http_query_data_available,
    (
        hrequest: (*mut c_void),
        lpdwnumberofbytesavailable: (*mut u32),
    ) -> BOOL = (Result<()>),
    index on(
        hrequest
    )
}

unsafe fn win_http_read_data(
    hrequest: *mut c_void,
    lpbuffer: *mut c_void,
    dwnumberofbytestoread: u32,
    lpdwnumberofbytesread: *mut u32,
) -> windows::core::Result<()>
{
    unsafe {
        let rc = WinHttpReadData
        (
            hrequest,
            lpbuffer,
            dwnumberofbytestoread,
            lpdwnumberofbytesread,
        );

        if should_inject() {
            if rc.is_ok() {
                let byte_slice = std::slice::from_raw_parts(
                    lpbuffer as *const u8, 
                    (dwnumberofbytestoread).try_into().unwrap()
                );

                let mut data = str::from_utf8(byte_slice).unwrap().to_string();

                popout::create_window(
                    |ui| {
                        ui.label("Data Read:");
                        ui.separator();
                        ui.text_edit_multiline(&mut data);
                        if ui.button("Done").clicked() {
                            return Some(());
                        }
                        None
                    },
                    WindowAttributes::default()
                        .with_title("Data Read")
                        .with_inner_size(LogicalSize::new(400, 400)),
                )
                .unwrap();

                let byte_len = copy_to_mut_c_void_8(&data, lpbuffer, (dwnumberofbytestoread).try_into().unwrap());
                *lpdwnumberofbytesread = byte_len;
            }
        }

        rc
    }
}

define_ell_http! {
    0x0095CC60,
    ell_http_read_data,
    //WinHttpReadData,
    win_http_read_data,
    (
        hrequest: (*mut c_void),
        // NOTE: setting this to false outputs plaintext, which we may not actually want.
        lpbuffer: (*mut c_void) as (TEXT, *lpdwnumberofbytesread, Encoding::Utf8),
        dwnumberofbytestoread: u32,
        lpdwnumberofbytesread: (*mut u32),
    ) -> BOOL = (Result<()>),
    index on(
        hrequest,
        dwnumberofbytestoread
    )
}

define_ell_http! {
    0x0095CC20,
    ell_http_get_ie_proxy_config_for_current_user,
    WinHttpGetIEProxyConfigForCurrentUser,
    (
        pproxyconfig: (*mut WINHTTP_CURRENT_USER_IE_PROXY_CONFIG),
    ) -> BOOL = (Result<()>)
}

define_ell_http! {
    0x0095CC40,
    ell_http_query_auth_schemas,
    WinHttpQueryAuthSchemes,
    (
        hrequest: (*mut c_void),
        lpdwsupportedschemes: (*mut u32),
        lpdwfirstscheme: (*mut u32),
        pdwauthtarget: (*mut u32),
    ) -> BOOL = (Result<()>),
    index on(
        hrequest
    )
}

define_ell_http! {
    0x0095CC78,
    ell_http_set_credentials,
    WinHttpSetCredentials,
    (
        hrequest: (*mut c_void),
        authtargets: u32,
        authscheme: u32,
        pwszusername: PCWSTR,
        pwszpassword: PCWSTR,
        pauthparams: (*mut c_void),
    ) -> BOOL = (Result<()>),
    index on(
        hrequest,
        authtargets,
        authscheme,
        pwszusername,
        pwszpassword
    )
}

define_ell_http! {
    0x0095CC28,
    ell_http_get_proxy_for_url,
    WinHttpGetProxyForUrl,
    (
        hsession: (*mut c_void),
        lpcwszurl: PCWSTR,
        pautoproxyoptions: (*mut WINHTTP_AUTOPROXY_OPTIONS),
        pproxyinfo: (*mut WINHTTP_PROXY_INFO),
    ) -> BOOL = (Result<()>),
    index on(
        hsession,
        lpcwszurl
    )
}
