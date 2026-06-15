use std::pin::Pin;

use turso::*;

use crate::log::error_to_file;
use crate::log::output_to_file;

pub enum Encoding {
    Base64,
    Utf8,
    Utf16,
}

/// Call this for each of the functions we're mocking.
/// See interfaces.rs
#[macro_export]
macro_rules! define_ell_http {
    (
        $rva:literal,
        $ell_fn:ident,
        $win_fn:ident,
        (
            $(
                $(< $index:tt >)? $arg:ident : $arg_ty:tt $(as ($as_ty:tt, $($as_ty_len:expr, $as_ty_encode:expr)?))?
            ),* $(,)?
        )
        -> $ret_ty:tt $(= $ret_orig_type:tt)? 
    ) => {
        impl DbSetupFns {
            pub async fn $ell_fn(conn: &turso::Connection) -> turso::Result<()> {
                conn.execute(
                    concat!(
                        "CREATE TABLE IF NOT EXISTS ", stringify!($ell_fn), " (",
                            "id INTEGER PRIMARY KEY, ",
                            "created_at INTEGER NOT NULL, ",
                            $(stringify!($arg), " ", $crate::column_type!($arg_ty $(as $as_ty)?), ", ",)*
                            "result ", $crate::column_type!($ret_ty), ", ",
                            "consumed BOOLEAN DEFAULT FALSE NOT NULL)"
                    ),
                    (),
                ).await?;

                let indices = $crate::create_index_list!($($(< $index >)? $arg),*);
                if indices.len() > 0 {
                    conn.execute(concat!("CREATE INDEX IF NOT EXISTS idx_", stringify!($ell_fn),
                        " ON ", stringify!($ell_fn), "(").to_owned()
                        + &indices[1..]
                        + ")",
                        ()
                    )
                    .await?;
                }

                Ok(())
            }
        }

        impl DbResetFns {
            pub async fn $ell_fn(conn: &turso::Connection) -> turso::Result<()> {
                let total_updated = conn.execute(
                    concat!(
                        "UPDATE ", stringify!($ell_fn),
                        " SET consumed = 0"
                    ),
                    (),
                ).await?;
        
                error_to_file(&format!("Rows changed: {}", total_updated).to_string());

                Ok(())
            }
        }

        #[unsafe(no_mangle)]
        #[allow(unused_mut)]
        pub extern "system" fn $ell_fn(
            $(mut $arg: $crate::strip_parens!($arg_ty)),*
        ) -> $crate::strip_parens!($ret_ty)
        {
            $crate::set_status_callback!($ell_fn $(, $arg)*);

            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            let handle = rt.handle();

            let replay_results = handle.block_on(async || -> turso::Result<Option<(
                $(
                    $crate::arg_value!($arg_ty),
                )*
                $crate::arg_value!($ret_ty)
            )>> 
            {
                error_to_file(&format!("CP0: {}", stringify!($ell_fn)));

                let query = concat!(
                    "UPDATE ", stringify!($ell_fn),
                    " SET consumed = true",
                    " WHERE id = (",
                        "SELECT id",
                        " FROM ", stringify!($ell_fn), 
                        " WHERE consumed IS FALSE ", 
                        $(
                            $crate::and_index!($(< $index >)? $arg)
                        ),* ,
                        " ORDER BY id ASC",
                        " LIMIT 1",
                    ")",
                    " RETURNING ",
                    $(stringify!($arg), ", ",)*
                    "result"
                );
                error_to_file(&format!("CP1: {}", query));

                if let Some((_, conn)) = db_get_replay_conn().await {
                    let mut query_params = Vec::<Value>::new();

                    #[allow(unused_unsafe)]
                    unsafe {
                        $(
                            $crate::add_index_to_vec!(
                                query_params, 
                                $($index)?, 
                                $arg,
                                ($arg_ty $(=> ($as_ty, $($as_ty_len, $as_ty_encode)?))?)
                            );
                        )*
                    }

                    for val in &query_params {
                        error_to_file(&format!("CP1.a: {:?}", val));
                    }

                    #[allow(unused_unsafe)]
                    let mut rows = unsafe {
                        conn.query(query, query_params).await?
                    };

                    if let Some(row) = rows.next().await? {
                        let mut col = 0;
                        $(
                            let $arg: Value = row.get_value(col)?;
                            error_to_file(&format!("CP2: {:?}", $arg));
                            col += 1;
                        )*

                        let result: Value = row.get_value(col)?;
                        error_to_file(&format!("CP3: {:?}", result));
                        Ok(Some((
                            $(
                                $arg,
                            )*
                            result
                        )))
                    } else {
                        error_to_file("CP4");
                        Err(turso::Error::Error(format!("Out of replay data in {}", stringify!($ell_fn)).to_string()))
                    }
                } else {
                    Ok(None)
                }
            }()).unwrap();

            error_to_file(&format!("CP4.a: {:?}", replay_results));

            unsafe {
                #[allow(unused_variables)]
                if let Some((
                    $(
                        paste::paste! { [<temp_ $arg>] },
                    )*
                    temp_result
                )) = replay_results {
                    error_to_file("CP7");
                    $(
                        error_to_file("CP7.a");
                        paste::paste! {
                            $crate::replay_value!($arg = [<temp_ $arg>] : $arg_ty $(=> ($as_ty, $($as_ty_len, $as_ty_encode)?))?);
                        }
                        error_to_file(&format!("CP5: {:?}", $arg));
                    )*

                    let ret = $crate::replay_result!(temp_result: $ret_ty);
                    error_to_file(&format!("CP6: {:?}", ret));

                    // Hacks to put a bit of extra code in a few functions.
                    // TODO: Find a better way to do this.
                    $crate::set_last_error!($ell_fn, temp_result, temp_lpbuffer);
                    $crate::set_callback_context!($ell_fn, temp_dwoption, temp_lpbuffer);
                    $crate::call_callbacks!($ell_fn, temp_hrequest);

                    ret
                } else {
                    error_to_file("CP8");
                    let result_orig = $win_fn(
                        $($arg),*
                    );
                    let result = $crate::convert_return!(result_orig: $($ret_orig_type)?);

                    log!(
                        stringify!($ell_fn),
                        $($arg = $crate::log_value!($arg: $arg_ty $(=> ($as_ty, $($as_ty_len, $as_ty_encode)?))?),)*
                        result = $crate::log_value!(result: $ret_ty)
                    );

                    result
                }
            }
        }

        inventory::submit! {
            Replacement {
                name: stringify!($ell_fn),
                rva: $rva,
                replacement: Some(|| $ell_fn as usize),
                setup: |conn| Box::pin(DbSetupFns::$ell_fn(conn)),
                reset: |conn| Box::pin(DbResetFns::$ell_fn(conn)),
            }
        }
    };
}

#[macro_export]
macro_rules! arg_value {
    ( $arg:tt ) => { Value }
}

#[macro_export]
macro_rules! filter_index_names {
    (<index> $arg:ident) => {$arg};
    ($arg:ident) => {};
}

#[macro_export]
macro_rules! and_index {
    (<index> $arg:ident) => {
        concat!(" AND ", stringify!($arg), " IS ?")
    };
    ($arg:ident) => {""}
}

#[macro_export]
macro_rules! add_index_to_vec {
    (
        $vec:ident, 
        index, 
        $arg:ident, 
        ($($arg_tys:tt)*)
    ) => 
    {
        $vec.push($crate::log_value!($arg: $($arg_tys)*));
    };
    ($vec:ident, , $arg:ident, $arg_ty:tt) => {}
}

#[macro_export]
macro_rules! create_index_list_comma {
    (<index> $arg:ident) => { concat!(", ", stringify!($arg)) };
    ($arg:ident) => {""};
}

#[macro_export]
macro_rules! create_index_list {
    () => {""};
    (
        $($(< $index:tt >)? $arg:ident),*
    ) => {
        concat!(
            $(
                create_index_list_comma!($(< $index >)? $arg)
            ),*
        )
    }
}

#[macro_export]
macro_rules! set_last_error {
    (
        ell_http_query_headers,
        $temp_result:ident,
        $temp_lpbuffer:ident
    ) => 
    {
        // HACK! We should be storing this at record time and handle
        // it across all the calls that use GetLastError.
        if let Some(header_err) = $temp_result.as_integer() {
            if *header_err == 0 {
                if $temp_lpbuffer.is_null() {
                    windows::Win32::Foundation::SetLastError(
                        windows::Win32::Foundation::WIN32_ERROR(
                            windows_sys::Win32::Foundation::ERROR_INSUFFICIENT_BUFFER
                        )
                    );
                } else {
                    windows::Win32::Foundation::SetLastError(
                        windows::Win32::Foundation::WIN32_ERROR(
                            windows_sys::Win32::Networking::WinHttp::ERROR_WINHTTP_HEADER_NOT_FOUND
                        )
                    );
                }
            }
        }
    };
    ($ell_fn:ident, $temp_result:ident, $temp_lpbuffer:ident) => {};
}

#[macro_export]
macro_rules! set_callback_context {
    (
        ell_http_set_option,
        $temp_dwoption:ident, 
        $temp_lpbuffer:ident
    ) => {
        if $temp_dwoption == windows_sys::Win32::Networking::WinHttp::WINHTTP_OPTION_CONTEXT_VALUE.into() {
            let mut status_callback_context = 
                STATUS_CALLBACK_CONTEXT.get_or_init(|| Mutex::new(0)).lock().unwrap();
            if let Some(val) = $temp_lpbuffer.as_integer() {
                *status_callback_context = *val as usize;
            } else if $temp_lpbuffer.is_null() {
                *status_callback_context = 0;
            }
        }
    };
    ($ell_fn:ident, $temp_dwoption:ident, $temp_lpbuffer:ident) => {};
}

#[macro_export]
macro_rules! set_status_callback {
    (
        ell_http_set_status_callback,
        $hinternet:ident,
        $callback:ident
        $(, $other:ident )*
    ) => {
        error_to_file("Setting Status Callback");
        let mut status_callback = STATUS_CALLBACK.get_or_init(|| Mutex::new(None)).lock().unwrap();
        *status_callback = $callback;
    };
    ($ell_fn:ident $(, $other:ident)*) => {};
}

#[macro_export]
macro_rules! call_callbacks {
    (
        ell_http_send_request,
        $hrequest:ident
    ) => {
        ell_http_status_callback(
            *$hrequest.as_integer().unwrap()
                as *const i64 as *mut i64 as *mut c_void,
            0,
            windows_sys::Win32::Networking::WinHttp::WINHTTP_CALLBACK_STATUS_SENDING_REQUEST.try_into().unwrap(),
            std::ptr::null_mut(),
            0
        );

        ell_http_status_callback(
            *$hrequest.as_integer().unwrap()
                as *const i64 as *mut i64 as *mut c_void,
            0,
            windows_sys::Win32::Networking::WinHttp::WINHTTP_CALLBACK_STATUS_REQUEST_SENT.try_into().unwrap(),
            std::ptr::null_mut(),
            0
        );
    };
    (
        ell_http_write_data,
        $hrequest:ident
    ) => {
        ell_http_status_callback(
            *$hrequest.as_integer().unwrap()
                as *const i64 as *mut i64 as *mut c_void,
            0,
            windows_sys::Win32::Networking::WinHttp::WINHTTP_CALLBACK_STATUS_SENDING_REQUEST.try_into().unwrap(),
            std::ptr::null_mut(),
            0
        );

        ell_http_status_callback(
            *$hrequest.as_integer().unwrap()
                as *const i64 as *mut i64 as *mut c_void,
            0,
            windows_sys::Win32::Networking::WinHttp::WINHTTP_CALLBACK_STATUS_REQUEST_SENT.try_into().unwrap(),
            std::ptr::null_mut(),
            0
        );
    };
    ($ell_fn:ident, $hrequest:ident) => {};
}

/*
#[macro_export]
macro_rules! create_index {
    (
        $conn:ident, 
        $ell_fn:ident, 
    ) => {};
    (
        $idx_first:ident
        $(, $($idx_rest:ident),* $(,)?)?
    ) => {
        $conn.execute(concat!("CREATE INDEX IF NOT EXISTS idx_", stringify!($ell_fn),
            " ON ", stringify!($ell_fn), "(",
            stringify!($idx_first),
            $($(", ", stringify!($idx_rest)),*,)?
            ")"),
            ()
        )
        .await?;
    }
}
*/


/// Framework for storing replacement details in an inventory to set them
/// up at runtime startup.

/// Returns the collected Replacement info
pub fn replacements() -> Vec<Replacement<'static>> {
    inventory::iter::<Replacement>
        .into_iter()
        .map(|entry| Replacement {
            name: entry.name,
            rva: entry.rva,
            replacement: entry.replacement,
            setup: entry.setup,
            reset: entry.reset,
        })
        .collect()
}

/// Calls all the db setup functions that were collected.
pub async fn db_setup_interfaces(conn: &Connection) -> Result<()> {
    for entry in inventory::iter::<Replacement> {
        (entry.setup)(conn).await?;
    }
    Ok(())
}

pub type AsyncSetupFn = for<'a> fn(
    &'a turso::Connection,
) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>;

pub struct Replacement<'a> {
    pub name: &'a str,
    pub rva: usize,                 // The original function address
    pub replacement: Option<fn() -> usize>, // The address of the injected function
    pub setup: AsyncSetupFn,        // A setup function to create the db table
    pub reset: AsyncSetupFn,        // A function to reset the consumed columns
}

inventory::collect!(Replacement<'static>);


/// Functions for the replay feature
pub async fn db_get_replay_conn() -> Option<(turso::Database, turso::Connection)>
{
    let replay_file = "replay.db";
    if !std::path::Path::new(replay_file).exists() {
        None
    } else {
        output_to_file("Opening replay.db");
        let db = Builder::new_local(replay_file)
            .build()
            .await
            .unwrap_or_else(|err| {
                error_to_file(&err.to_string());
                panic!();
            });

        let conn = db.connect().ok()?;

        Some((db, conn))
    }
}

/// Calls all the db reset functions that were collected.
pub fn reset_replay() -> Result<()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let handle = rt.handle();

    handle.block_on(async || -> Result<()> {
        if let Some((_, conn)) = db_get_replay_conn().await {
            for entry in inventory::iter::<Replacement> {
                error_to_file(&format!("Resetting {}", entry.name).to_string());

                (entry.reset)(&conn).await?;
            }
        }

        Ok(())
    }())
}


/// Conversion functions and macros
pub unsafe fn to_csv(vals: *const windows_strings::PCWSTR) -> Value {
    if vals.is_null() {
        return Value::Null;
    }

    let mut vals_vec = Vec::new();
    let mut i = 0;

    unsafe {
        while !(*vals.add(i)).is_null() {
            vals_vec.push((*vals.add(i)).to_string().unwrap_or_default());
            i += 1;
        }
    }

    Value::Text(vals_vec.join(", "))
}

#[macro_export]
macro_rules! strip_parens {
    // If it's wrapped in parens, unwrap it and emit the inner tokens
    (($($inner:tt)*)) => { $($inner)* };
    // If it's not wrapped, just pass it through as-is
    ($otherwise:tt) => { $otherwise };
}

#[macro_export]
macro_rules! convert_return {
    ($result_orig:ident : (Result<()>)) => 
    { 
        Into::<BOOL>::into($result_orig.is_ok()) 
    };
    ($result_orig:ident :) => { $result_orig };
}

pub fn read_utf16_le(bytes: &[u8]) -> String {
    // Group bytes into pairs of 2 and convert them into u16 elements
    let u16_buffer: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|chunk| {
            // Converts [u8; 2] to a single u16 value
            u16::from_le_bytes([chunk[0], chunk[1]])
        })
        .collect();

    // Decode the u16 vector into a standard UTF-8 Rust String
    String::from_utf16(&u16_buffer).unwrap()
}

#[macro_export]
macro_rules! column_type {
    ($t:tt as $explicit:tt) => { stringify!($explicit) };
    (PCWSTR) => { "TEXT" };
    (*const windows_strings::PCWSTR) => { "TEXT" };
    (*mut c_void) => { "INTEGER" };
    (*const c_void) => { "INTEGER" };
    (*mut u32) => { "INTEGER" };
    (WINHTTP_STATUS_CALLBACK) => { "INTEGER" };
    (WINHTTP_ACCESS_TYPE) => { "INTEGER" };
    (WINHTTP_OPEN_REQUEST_FLAGS) => { "INTEGER" };
    (*mut WINHTTP_CURRENT_USER_IE_PROXY_CONFIG) => { "TEXT" };
    (*mut WINHTTP_AUTOPROXY_OPTIONS) => { "TEXT" };
    (*mut WINHTTP_PROXY_INFO) => { "TEXT" };
    (BOOL) => { "BOOLEAN" };
    ($t:tt) => { "INTEGER" };
}

/*
#[macro_export]
macro_rules! turso_type {
    ($t:tt as BOOL) => { Value::Integer };
    ($t:tt as TEXT) => { Value::Text };
    ($t:tt as INTEGER) => { Value::Integer };
    (PCWSTR) => { Value::Text };
    (*const windows_strings::PCWSTR) => { Value::Text };
    (*mut c_void) => { Value::Integer };
    (*const c_void) => { Value::Integer };
    (*mut u32) => { Value::Integer };
    (WINHTTP_STATUS_CALLBACK) => { Value::Integer };
    (WINHTTP_ACCESS_TYPE) => { Value::Integer };
    (WINHTTP_OPEN_REQUEST_FLAGS) => { Value::Integer };
    (*mut WINHTTP_CURRENT_USER_IE_PROXY_CONFIG) => { Value::Text };
    (*mut WINHTTP_AUTOPROXY_OPTIONS) => { Value::Text };
    (*mut WINHTTP_PROXY_INFO) => { Value::Text };
    (BOOL) => { Value::Integer };
    ($t:tt) => { Value::Integer };
}
*/

#[macro_export]
macro_rules! log_value {
    // If it's wrapped in parenths, unwrap and recurse.
    ($name:ident : ($($inner:tt)*)) => {
        $crate::log_value!($name : $($inner)*)
    };
    ($name:ident : ($($inner:tt)*) => ($as_ty:tt, $as_ty_len:expr, $as_ty_encode:expr)) => {
        $crate::log_value!($name : $($inner)* => ($as_ty, $as_ty_len, $as_ty_encode))
    };
    ($name:ident : *mut c_void) => {
        if $name.is_null() {
            Value::Null
        } else {
            Value::Integer($name as usize as i64)
        }
    };
    ($name:ident : *const c_void => (TEXT, $as_ty_len:expr, $as_ty_encode:expr)) => {
        if $name.is_null() {
            Value::Null
        } else {
            let byte_slice = std::slice::from_raw_parts(
                $name as *const u8, 
                $as_ty_len.try_into().unwrap()
            );

            match $as_ty_encode {
                Encoding::Base64 =>
                    Value::Text(
                        base64::engine::general_purpose::STANDARD.encode(byte_slice)
                    ),
                Encoding::Utf8 => 
                    Value::Text(str::from_utf8(byte_slice).unwrap().to_string()),
                Encoding::Utf16 => 
                    Value::Text(read_utf16_le(byte_slice)),
            }
        }
    };
    ($name:ident : *mut c_void => (TEXT, $as_ty_len:expr, $as_ty_encode:expr)) => {
        if $name.is_null() {
            Value::Null
        } else {
            let byte_slice = std::slice::from_raw_parts(
                $name as *const u8, 
                $as_ty_len.try_into().unwrap()
            );

            match $as_ty_encode {
                Encoding::Base64 =>
                    Value::Text(
                        base64::engine::general_purpose::STANDARD.encode(byte_slice)
                    ),
                Encoding::Utf8 => 
                    Value::Text(str::from_utf8(byte_slice).unwrap().to_string()),
                Encoding::Utf16 => 
                    Value::Text(read_utf16_le(byte_slice)),
            }
        }
    };
    ($name:ident : Option<*mut c_void> => (TEXT, $as_ty_len:expr, $as_ty_encode:expr)) => {
        match $name {
            None => Value::Null,
            Some(opt) => {
                let byte_slice = std::slice::from_raw_parts(
                    opt as *const u8, 
                    $as_ty_len.try_into().unwrap()
                );

                match $as_ty_encode {
                    Encoding::Base64 =>
                        Value::Text(
                            base64::engine::general_purpose::STANDARD.encode(byte_slice)
                        ),
                    Encoding::Utf8 => 
                        Value::Text(str::from_utf8(byte_slice).unwrap().to_string()),
                    Encoding::Utf16 => 
                        Value::Text(read_utf16_le(byte_slice)),
                }
            }

        }
    };
    ($name:ident : *const c_void) => {
        if $name.is_null() {
            Value::Null
        } else {
            Value::Integer($name as usize as i64)
        }
    };
    ($name:ident : *mut u32) => {
        if $name.is_null() {
            Value::Null
        } else {
            Value::Integer(*$name as usize as i64)
        }
    };
    ($name:ident : PCWSTR) => {
        if $name.is_null() {
            Value::Null
        } else {
            Value::Text($name.to_string().unwrap())
        }
    };
    ($name:ident : *const PCWSTR) => { 
        to_csv($name)
    };
    ($name:ident : WINHTTP_STATUS_CALLBACK) => {
        match $name {
            Some(cb) => Value::Integer(cb as usize as i64),
            None => Value::Null,
        }
    };
    ($name:ident : WINHTTP_ACCESS_TYPE) => { Value::Integer($name.0.into()) };
    ($name:ident : WINHTTP_OPEN_REQUEST_FLAGS) => { Value::Integer($name.0.into()) };
    ($name:ident : *mut WINHTTP_CURRENT_USER_IE_PROXY_CONFIG) => {
        if $name.is_null() {
            Value::Null
        } else {
            let byte_slice = std::slice::from_raw_parts(
                $name as *const _ as *const u8, 
                std::mem::size_of::<WINHTTP_CURRENT_USER_IE_PROXY_CONFIG>(),
            );

            Value::Text(
                base64::engine::general_purpose::STANDARD.encode(byte_slice)
            )
        }
    };
    ($name:ident : *mut WINHTTP_AUTOPROXY_OPTIONS) => {
        if $name.is_null() {
            Value::Null
        } else {
            let byte_slice = std::slice::from_raw_parts(
                $name as *const _ as *const u8, 
                std::mem::size_of::<WINHTTP_AUTOPROXY_OPTIONS>(),
            );

            Value::Text(
                base64::engine::general_purpose::STANDARD.encode(byte_slice)
            )
        }
    };
    ($name:ident : *mut WINHTTP_PROXY_INFO) => {
        if $name.is_null() {
            Value::Null
        } else {
            let byte_slice = std::slice::from_raw_parts(
                $name as *const _ as *const u8, 
                std::mem::size_of::<WINHTTP_PROXY_INFO>(),
            );

            Value::Text(
                base64::engine::general_purpose::STANDARD.encode(byte_slice)
            )
        }
    };
    ($name:ident : BOOL) => { Value::Integer($name.0.into()) };
    ($name:ident : $t:tt) => { Value::Integer(($name as i64).into()) };
}

#[macro_export]
macro_rules! replay_value {
    // If it's wrapped in parenths, unwrap and recurse.
    ($result:ident = $name:ident : ($($inner:tt)*)) => {
        $crate::replay_value!($result = $name : $($inner)*)
    };
    ($result:ident = $name:ident : ($($inner:tt)*) => ($as_ty:tt, $as_ty_len:expr, $as_ty_encode:expr)) => {
        $crate::replay_value!($result = $name : $($inner)* => ($as_ty, $as_ty_len, $as_ty_encode))
    };
    ($result:ident = $name:ident : *mut c_void) => {
        // We don't know what underlying type this is. Likely input anyway.
    };
    ($result:ident = $name:ident : *const c_void) => {
        // Input value; do not change
    };
    ($result:ident = $name:ident : *const c_void => (TEXT, $as_ty_len:expr, $as_ty_encode:expr)) => {
        // Input value; do not change
    };
    ($result:ident = $name:ident : *mut c_void => (TEXT, $as_ty_len:expr, $as_ty_encode:expr)) => {
        if !$result.is_null() {
            if let Value::Text(ref val) = $name {
                match $as_ty_encode {
                    Encoding::Base64 => {
                        let mut decoded_bytes: Vec<u8> = base64::engine::general_purpose::STANDARD
                            .decode(std::string::String::from(val))
                            .expect("Failed to decode base64");
                        decoded_bytes.shrink_to_fit();

                        std::ptr::copy_nonoverlapping(
                            decoded_bytes.as_ptr(),
                            $result as *mut u8,
                            ($as_ty_len).try_into().unwrap()
                        );
                    },
                    Encoding::Utf8 => {
                        let str = std::string::String::from(val);
                        let c_str = std::ffi::CString::new(str).expect("Failed to create CString");
                        std::ptr::copy_nonoverlapping(
                            c_str.as_ptr(),
                            $result as *mut i8,
                            ($as_ty_len).try_into().unwrap()
                        );
                    },
                    Encoding::Utf16 => {
                        let string_bytes: Vec<u16> = 
                            (std::string::String::from(val))
                            .encode_utf16()
                            .chain(std::iter::once(0))
                            .collect();

                        let len = std::cmp::min(
                            ($as_ty_len/2).try_into().unwrap(),
                            (string_bytes.len()).try_into().unwrap()
                        );

                        error_to_file(&format!("Converting to UTF-16 {}: {:?}", len, string_bytes));

                        std::ptr::copy_nonoverlapping(
                            string_bytes.as_ptr(),
                            $result as *mut u16,
                            len
                        );
                    }
                }
            } else if $as_ty_len > 1 {
                let typed_ptr = $result as *mut u16;
                *typed_ptr = 0;
            }
        }
    };
    /*
    ($result:ident = $name:ident : Option<*mut c_void> => (TEXT, $as_ty_len:expr, $as_ty_encode:expr)) => {
        if let Some(val) = $name {
            match $as_ty_encode {
                Encoding::Base64 => {
                    let mut decoded_bytes: Vec<u8> = base64::engine::general_purpose::STANDARD
                        .decode(val as String)
                        .expect("Failed to decode base64");
                    decoded_bytes.shrink_to_fit();

                    std::ptr::copy_nonoverlapping(
                        decoded_bytes,
                        $result as *mut u8,
                        $as_ty_len
                    );
                },
                Encoding::Utf8 => {
                    let string_bytes = (val as String).as_bytes();

                    std::ptr::copy_nonoverlapping(
                        string_bytes.as_ptr(),
                        $result as *mut u8,
                        $as_ty_len
                    );
                },
                Encoding::Utf16 => {
                    let string_bytes: Vec<u16> = (val as String).encode_utf16().collect();

                    std::ptr::copy_nonoverlapping(
                        string_bytes.as_ptr(),
                        $result as *nut u16,
                        $as_ty_len / 2
                    );
                }
            }
        } else {
            $result = std::ptr::null();
        }
    };
    */
    ($result:ident = $name:ident : *const c_void) => {
        // This is an in value that we can't change.
    };
    ($result:ident = $name:ident : *mut u32) => {
        if !$result.is_null() {
            if let Value::Integer(val) = $name {
                *$result = val as u32;
            } else {
                *$result = 0;
            }
        }
    };
    ($result:ident = $name:ident : PCWSTR) => {
        // This is an in value that we can't change.
    };
    ($result:ident = $name:ident : *const PCWSTR) => { 
        // This is an in value that we can't change.
    };
    ($result:ident = $name:ident : WINHTTP_STATUS_CALLBACK) => {
        // This is an in value that we can't change.
    };
    ($result:ident = $name:ident : WINHTTP_ACCESS_TYPE) => { 
        // This is an in value that we can't change.
    };
    ($result:ident = $name:ident : WINHTTP_OPEN_REQUEST_FLAGS) => { 
        // This is an in value
    };
    ($result:ident = $name:ident : *mut WINHTTP_CURRENT_USER_IE_PROXY_CONFIG) => {
        if !$result.is_null() {
            if let Value::Text(val) = $name {
                let mut decoded_bytes: Vec<u8> = base64::engine::general_purpose::STANDARD
                    .decode(val as String)
                    .expect("Failed to decode base64");
                decoded_bytes.shrink_to_fit();

                std::ptr::copy_nonoverlapping(
                    decoded_bytes.as_ptr(),
                    $result as *mut u8,
                    std::mem::size_of::<WINHTTP_CURRENT_USER_IE_PROXY_CONFIG>()
                );
            } else {
                *$result = WINHTTP_CURRENT_USER_IE_PROXY_CONFIG::default();
            }
        }
    };
    ($result:ident = $name:ident : *mut WINHTTP_AUTOPROXY_OPTIONS) => {
        if !$result.is_null() {
            if let Value::Text(val) = $name {
                let mut decoded_bytes: Vec<u8> = base64::engine::general_purpose::STANDARD
                    .decode(val as String)
                    .expect("Failed to decode base64");
                decoded_bytes.shrink_to_fit();

                std::ptr::copy_nonoverlapping(
                    decoded_bytes.as_ptr(),
                    $result as *mut u8,
                    std::mem::size_of::<WINHTTP_AUTOPROXY_OPTIONS>()
                );
            } else {
                *$result = WINHTTP_AUTOPROXY_OPTIONS::default();
            }
        }
    };
    ($result:ident = $name:ident : *mut WINHTTP_PROXY_INFO) => {
        if !$result.is_null() {
            if let Value::Text(val) = $name {
                let mut decoded_bytes: Vec<u8> = base64::engine::general_purpose::STANDARD
                    .decode(val as String)
                    .expect("Failed to decode base64");
                decoded_bytes.shrink_to_fit();

                std::ptr::copy_nonoverlapping(
                    decoded_bytes.as_ptr(),
                    $result as *mut u8,
                    std::mem::size_of::<WINHTTP_PROXY_INFO>()
                );
            } else {
                *$result = WINHTTP_PROXY_INFO::default();
            }
        }
    };
    ($result:ident = $name:ident : BOOL) => {
        if let Value::Integer(val) = $name {
            $result = val as u32;
        } else {
            $result = 0;
        }
    };
    ($result:ident = $name:ident : $t:tt) => {
        // This is an input type that we can't change
    };
}

#[macro_export]
macro_rules! replay_result {
    ($name:ident : (*mut c_void)) => {
        if let Value::Integer(val) = $name {
            val as i64 as *mut c_void
        } else {
            std::ptr::null_mut()
        }
    };
    ($name:ident : WINHTTP_STATUS_CALLBACK) => {
        if let Value::Integer(val) = $name {
            WINHTTP_STATUS_CALLBACK::Some(std::mem::transmute(val as *const ()))
        } else {
            WINHTTP_STATUS_CALLBACK::None
        }
    };
    ($name:ident : BOOL) => {
        if let Value::Integer(val) = $name {
            (val != 0).into()
        } else {
            false.into()
        }
    };
}

pub struct DbSetupFns;
pub struct DbResetFns;

