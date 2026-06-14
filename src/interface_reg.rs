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
        ($($arg:ident : $arg_ty:tt $(as ($as_ty:tt, $($as_ty_len:expr, $as_ty_encode:expr)?))?),* $(,)?)
        -> $ret_ty:tt $(= $ret_orig_type:tt)? 
        $(, index on ($($idx_col:ident),* $(,)?))?
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

                $( $crate::create_index!(conn, $ell_fn, $($idx_col),*); )?

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
        pub extern "system" fn $ell_fn(
            $($arg: $crate::strip_parens!($arg_ty)),*
        ) -> $crate::strip_parens!($ret_ty)
        {
            unsafe {
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
macro_rules! create_index {
    (
        $conn:ident, 
        $ell_fn:ident, 
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
    (*mut u32) => { "INTEGET" };
    (WINHTTP_STATUS_CALLBACK) => { "INTEGER" };
    (WINHTTP_ACCESS_TYPE) => { "INTEGER" };
    (WINHTTP_OPEN_REQUEST_FLAGS) => { "INTEGER" };
    (*mut WINHTTP_CURRENT_USER_IE_PROXY_CONFIG) => { "TEXT" };
    (*mut WINHTTP_AUTOPROXY_OPTIONS) => { "TEXT" };
    (*mut WINHTTP_PROXY_INFO) => { "TEXT" };
    (BOOL) => { "BOOLEAN" };
    ($t:tt) => { "INTEGER" };
}

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
    ($name:ident : *const PCWSTR) => { to_csv($name) };
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

pub struct DbSetupFns;
pub struct DbResetFns;

