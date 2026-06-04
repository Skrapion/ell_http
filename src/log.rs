use std::fs::OpenOptions;
use std::io::Write;
use std::time::SystemTime;

use anyhow::Result;
use chrono::Local;
use turso::*;

use crate::interface_reg::*;

pub struct LogItem {
    pub table: String,
    pub args: Vec<(String, turso::Value)>,
}

pub static LOG_QUEUE: once_cell::sync::Lazy<crossbeam::queue::SegQueue<LogItem>> =
    once_cell::sync::Lazy::new(crossbeam::queue::SegQueue::new);

pub type SqlArgs = Vec<turso::Value>;

#[macro_export]
macro_rules! log {
    (
        $table:expr, 
        $($col:ident = $val:expr $(=> $as_ty:tt)?),* $(,)?
    ) => {{
        let params = vec![
            $(
                (stringify!($col).to_string(), $val),
            )*
        ];

        LOG_QUEUE.push(LogItem{table: $table.to_string(), args: params});
    }};
}

async fn create_db_conn() -> Result<(turso::Database, turso::Connection)> {
    let timestamp = Local::now().format("%Y-%m-%dT%H-%M-%S");

    let db = Builder::new_local(&format!("capture_{}.db", timestamp))
        .build()
        .await
        .unwrap_or_else(|err| {
            error_to_file(&err.to_string());
            panic!();
        });

    output_to_file(&format!("INFO: Created capture_{}.db", timestamp));

    let conn = db.connect()?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS master_log(
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            created_at  INTEGER NOT NULL,
            table_name  TEXT NOT NULL,
            params      TEXT NOT NULL
        )",
        ()
    )
    .await
    .unwrap_or_else(|err| {
        error_to_file(&err.to_string());
        0
    });

    db_setup_interfaces(&conn).await?;

    Ok((db, conn))
}

pub fn spawn_logger() {
    std::thread::spawn(|| {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        rt.block_on(async {
            let (_db, conn) = create_db_conn()
                .await.unwrap_or_else(|err| {
                    panic!("Failed to connect to database: {}", err);
                });

            loop {
                while let Some(item) = LOG_QUEUE.pop() {
                    log_item(&conn, item).await;
                }

                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        });
    });
}

async fn log_item(conn: &Connection, item: LogItem) {
    let timestamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let mut file_line = String::new();
    let mut cols = String::from("id, created_at");
    let mut placeholders = String::from("?, ?");
    let mut args: SqlArgs = Vec::new();
    args.push(Value::Integer(timestamp.try_into().unwrap()));

    for arg in item.args {
        file_line.push_str(&format!(", {}={:?}", arg.0, arg.1).to_string());
        cols.push_str(&format!(", {}", arg.0).to_string());
        placeholders.push_str(", ?");
        args.push(arg.1);
    }

    let _ = conn.execute(
        "INSERT INTO master_log (created_at, table_name, params) VALUES (?, ?, ?)",
        (
            Value::Integer(timestamp.try_into().unwrap()), 
            Value::Text(item.table.clone()),
            Value::Text(file_line[2..].to_string()) // Remove from ", " from the front
        )
    )
    .await
    .unwrap_or_else(|err| {
        error_to_file(&err.to_string());
        0
    });
    let id = conn.last_insert_rowid();

    args.insert(0, Value::Integer(id));
    file_line.insert_str(
        0, 
        &format!("{}[{}]: created_at: {}, ", item.table, id, timestamp).to_string()
    );

    output_to_file(&file_line);

    let sql = format!("INSERT INTO {} ({}) VALUES ({})",
        item.table,
        cols,
        placeholders).to_string();

    let _ = conn.execute(
        &sql,
        turso::params_from_iter(args),
    )
    .await
    .unwrap_or_else(|err| {
        error_to_file(&err.to_string());
        0
    });
}

pub fn output_to_file(s: &str) {
    let mut file = match OpenOptions::new()
        .create(true)
        .append(true)
        .open("log.txt")
    {
        Ok(f) => f,
        Err(_) => return, // Fail silently to prevent crashing your DLL
    };

    let _ = writeln!(file, "{}", s);
}

pub fn error_to_file(err: &str) {
    output_to_file(&format!("ERROR: {}", err).to_string());
}
