use std::pin::Pin;

use turso::*;

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
        $(, index on ($($idx_col:ident),*))?
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
                rva: $rva,
                replacement: Some(|| $ell_fn as usize),
                setup: |conn| Box::pin(DbSetupFns::$ell_fn(conn)),
            }
        }
    };
}

#[macro_export]
macro_rules! create_index {
    (
        $conn:ident, 
        $ell_fn:ident, 
        $idx_first:ident,
        $($idx_rest:ident),* $(,)?
    ) => {
        $conn.execute(concat!("CREATE INDEX IF NOT EXISTS idx_", stringify!($ell_fn),
            " ON ", stringify!($ell_fn), "(",
            stringify!($idx_first),
            $(", ", stringify!($idx_rest)),*,
            ")"),
            ()
        )
        .await?;
    }
}


/// Framework for storing replacement details in an inventory to set them
/// up at runtime startup.

/// Returns the collected Replacement info
pub fn replacements() -> Vec<Replacement> {
    inventory::iter::<Replacement>
        .into_iter()
        .map(|entry| Replacement {
            rva: entry.rva,
            replacement: entry.replacement,
            setup: entry.setup
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

pub struct Replacement {
    pub rva: usize,                 // The original function address
    pub replacement: Option<fn() -> usize>, // The address of the injected function
    pub setup: AsyncSetupFn,        // A setup function to create the db table
}

inventory::collect!(Replacement);

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

            if $as_ty_encode == true {
                Value::Text(
                    base64::engine::general_purpose::STANDARD.encode(byte_slice)
                )
            } else {
                Value::Text(str::from_utf8(byte_slice).unwrap().to_string())
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

                if $as_ty_encode == true {
                    Value::Text(
                        base64::engine::general_purpose::STANDARD.encode(byte_slice)
                    )
                } else {
                    Value::Text(str::from_utf8(byte_slice).unwrap().to_string())
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
            Value::Integer($name as usize as i64)
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
    ($name:ident : BOOL) => { Value::Integer($name.0.into()) };
    ($name:ident : $t:tt) => { Value::Integer(($name as i64).into()) };
}

pub struct DbSetupFns;

