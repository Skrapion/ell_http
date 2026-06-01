use std::os::raw::*;

use anyhow::Result;
use turso::*;
use windows::core::*;
use windows::Win32::Networking::WinHttp::*;

use crate::log;
use log::*;

/// Replacements
pub struct Replacement {
    pub original_rva: usize,
    pub replacement: usize,
}

#[macro_export]
macro_rules! replacements {
    ($($rva:expr => $rep:expr),* $(,)?) => {
        vec![
            $(
                Replacement {
                    original_rva: $rva,
                    replacement: $rep as usize,
                }
            ),*
        ]
    };
}

pub fn replacements() -> Vec<Replacement> {
    replacements![
        0x0095CC30 => EllHttpOpen,
        0x0095CC88 => EllHttpSetStatusCallback,
        0x0095CC18 => EllHttpConnect,
        0x0095CC38 => EllHttpOpenRequest,
    ]
}

pub async fn db_setup_interfaces(conn: &Connection) -> Result<()> {
    ell_http_open_setup(conn).await?;
    ell_http_set_status_callback_setup(conn).await?;
    ell_http_connect_setup(conn).await?;
    ell_http_open_request(conn).await?;
    Ok(())
}

/// Interfaces
type Lpcwstr = *const u16;

pub async fn ell_http_open_setup(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS http_open (
            id              INTEGER PRIMARY KEY,
            created_at      INTEGER NOT NULL,
            agent           TEXT NOT NULL,
            access_type     INTEGER NOT NULL,
            proxy           TEXT NOT NULL,
            proxy_bypass    TEXT NOT NULL,
            flags           INTEGER NOT NULL,
            out_handle      INTEGER NOT NULL,
            consumed        BOOLEAN DEFAULT FALSE NOT NULL
        )",
        (),
    )
    .await?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_out_handle ON http_open(out_handle)",
        (),
    )
    .await?;

    Ok(())
}

#[unsafe(no_mangle)]
pub extern "system" fn EllHttpOpen(
    pszagentw: Lpcwstr,
    dwaccesstype: WINHTTP_ACCESS_TYPE,
    pszproxyw: Lpcwstr,
    pszproxybypassw: Lpcwstr,
    dwflags: u32,
) -> *mut c_void
{
    unsafe {
        let out_handle = WinHttpOpen(
            PCWSTR::from_raw(pszagentw), 
            dwaccesstype,
            PCWSTR::from_raw(pszproxyw),
            PCWSTR::from_raw(pszproxybypassw), 
            dwflags);

        log!(
            "http_open",
            agent = lp2str(pszagentw), 
            access_type = Value::Integer(dwaccesstype.0.into()),
            proxy = lp2str(pszproxyw),
            flags = Value::Integer(dwflags.into()),
            proxy_bypass = lp2str(pszproxybypassw),
            out_handle = Value::Integer((out_handle as u32).into())
        );

        out_handle 
    }
}

pub async fn ell_http_set_status_callback_setup(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS http_set_status_callback(
            id                  INTEGER PRIMARY KEY,
            created_at          INTEGER NOT NULL,
            hinternet           INTEGER NOT NULL,
            internet_callback   INTEGER,
            notification_flags  INTEGER NOT NULL,
            reserved            INTEGER NOT NULL,
            out_callback        INTEGER,
            consumed            BOOLEAN DEFAULT FALSE NOT NULL
        )",
        (),
    )
    .await?;

    Ok(())
} 

#[unsafe(no_mangle)]
pub extern "system" fn EllHttpSetStatusCallback (
    hinternet: *mut c_void,
    lpfninternetcallback: WINHTTP_STATUS_CALLBACK,
    dwnotificationflags: u32,
    dwreserved: usize,
) -> WINHTTP_STATUS_CALLBACK 
{
    unsafe {
        let out_callback = WinHttpSetStatusCallback(
            hinternet,
            lpfninternetcallback,
            dwnotificationflags,
            dwreserved
        );

        log!("http_set_status_callback",
            hinternet = Value::Integer((hinternet as u32).into()),
            internet_callback = match lpfninternetcallback {
                Some(callback) => Value::Integer(callback as usize as i64),
                None => Value::Null
            },
            notification_flags = Value::Integer(dwnotificationflags.into()),
            reserved = Value::Integer((dwreserved as u32).into()),
            out_callback = match out_callback {
                Some(callback) => Value::Integer(callback as usize as i64),
                None => Value::Null
            }
        );

        out_callback
    }
}

pub async fn ell_http_connect_setup(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS http_connect(
            id                  INTEGER PRIMARY KEY,
            created_at          INTEGER NOT NULL,
            hsession            INTEGER NOT NULL,
            server_name         TEXT NOT NULL,
            server_port         INTEGER NOT NULL,
            reserved            INTEGER NOT NULL,
            out_connection      INTEGER NOT NULL,
            consumed            BOOLEAN DEFAULT FALSE NOT NULL
        )",
        (),
    )
    .await?;

    Ok(())
}

#[unsafe(no_mangle)]
pub extern "system" fn EllHttpConnect(
    hsession: *mut c_void,
    pswzservername: Lpcwstr,
    nserverport: u16,
    dwreserved: u32,
) -> *mut c_void
{
    unsafe {
        let out_connection = WinHttpConnect(
            hsession,
            PCWSTR::from_raw(pswzservername),
            nserverport,
            dwreserved
        );

        log!("http_connect",
            hsession = Value::Integer((hsession as u32).into()),
            server_name = lp2str(pswzservername),
            server_port = Value::Integer(nserverport.into()),
            reserved = Value::Integer(dwreserved.into()),
            out_connection = Value::Integer((out_connection as u32).into())
        );

        out_connection
    }
}

unsafe fn accept_types_to_value(
    accept_types: *const windows_strings::PCWSTR,
) -> Value {
    if accept_types.is_null() {
        return Value::Null;
    }

    let mut values = Vec::new();
    let mut i = 0;

    unsafe {
        while !(*accept_types.add(i)).is_null() {
            values.push((*accept_types.add(i)).to_string().unwrap_or_default());
            i += 1;
        }
    }

    Value::Text(values.join(", "))
}

pub async fn ell_http_open_request(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS http_open_request(
            id                  INTEGER PRIMARY KEY,
            created_at          INTEGER NOT NULL,
            hconnect            INTEGER NOT NULL,
            verb                TEXT NOT NULL,
            object_name         TEXT NOT NULL,
            version             TEXT,
            referrer            TEXT NOT NULL,
            accept_types        TEXT,
            flags               INTEGER NOT NULL,
            out_request         INTEGER NOT NULL,
            consumed            BOOLEAN DEFAULT FALSE NOT NULL
        )",
        (),
    )
    .await?;

    Ok(())
}

#[unsafe(no_mangle)]
pub extern "system" fn EllHttpOpenRequest(
    hconnect: *mut c_void,
    lpszverb: Lpcwstr,
    lpszobjectname: Lpcwstr,
    lpszversion: Lpcwstr,
    lpszreferrer: Lpcwstr,
    lplpszaccepttypes: *const windows_strings::PCWSTR,
    dwflags: WINHTTP_OPEN_REQUEST_FLAGS,
) -> *mut c_void
{
    unsafe {
        let out_request = WinHttpOpenRequest(
            hconnect,
            PCWSTR::from_raw(lpszverb),
            PCWSTR::from_raw(lpszobjectname),
            PCWSTR::from_raw(lpszversion),
            PCWSTR::from_raw(lpszreferrer),
            lplpszaccepttypes,
            dwflags
        );

        log!("http_open_request",
            hconnect = Value::Integer((hconnect as u32).into()),
            verb = lp2str(lpszverb),
            object_name = lp2str(lpszobjectname),
            version = if lpszversion.is_null() {
                Value::Null
            } else {
                lp2str(lpszversion)
            },
            referrer = lp2str(lpszreferrer),
            accept_types = accept_types_to_value(lplpszaccepttypes),
            flags = Value::Integer(dwflags.0.into()),
            out_request = Value::Integer((out_request as u32).into())
        );

        out_request
    }
}
