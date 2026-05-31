use std::fs::OpenOptions;
use std::io::Write;
use std::time::SystemTime;

use windows::core::*;

#[allow(dead_code)]
const NO_PARAMS: &[(&str, String)] = &[];

pub fn log_function_call(func_name: &str, params: &[(&str, String)]) {
    let mut file = match OpenOptions::new()
        .create(true)
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
            log_function_call($func_name, &params);
        }
    };
}

pub fn lp2str(ptr: *const u16) -> String {
    if ptr.is_null() {
        return String::new();
    }
    
    unsafe {
        PCWSTR::from_raw(ptr).display().to_string()
    }
}

