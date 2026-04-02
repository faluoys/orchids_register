use rusqlite::types::{ToSql, Value};
use rusqlite::{Connection, OptionalExtension, Result, params, params_from_iter};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: i64,
    pub email: String,
    pub password: String,
    pub sign_up_id: Option<String>,
    pub email_code: Option<String>,
    pub register_complete: bool,
    pub created_session_id: Option<String>,
    pub created_user_id: Option<String>,
    pub client_cookie: Option<String>,
    pub desktop_jwt: Option<String>,
    pub status: String,
    pub error_message: Option<String>,
    pub batch_id: Option<String>,
    pub plan: Option<String>,
    pub credits: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
    pub group_id: i64,
    pub group_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountGroup {
    pub id: i64,
    pub name: String,
    pub pinned: bool,
    pub is_default: bool,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Domain {
    pub id: i64,
    pub domain: String,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

pub fn init_db() -> Result<Connection> {
    let app_dir = dirs_next().unwrap_or_else(|| std::path::PathBuf::from("."));
    std::fs::create_dir_all(&app_dir).ok();
    let db_path = app_dir.join("orchids_register.db");
    let conn = Connection::open(db_path)?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS accounts (
            id                      INTEGER PRIMARY KEY AUTOINCREMENT,
            email                   TEXT NOT NULL,
            password                TEXT NOT NULL,
            sign_up_id              TEXT,
            email_code              TEXT,
            register_complete       INTEGER DEFAULT 0,
            created_session_id      TEXT,
            created_user_id         TEXT,
            client_cookie           TEXT,
            desktop_jwt             TEXT,
            status                  TEXT DEFAULT 'pending' CHECK(status IN ('pending','running','complete','failed')),
            error_message           TEXT,
            batch_id                TEXT,
            group_id                INTEGER,
            plan                    TEXT,
            credits                 INTEGER,
            created_at              TEXT DEFAULT (datetime('now','localtime')),
            updated_at              TEXT DEFAULT (datetime('now','localtime'))
        );

        CREATE TABLE IF NOT EXISTS account_groups (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            name        TEXT NOT NULL UNIQUE,
            pinned      INTEGER DEFAULT 0,
            is_default  INTEGER DEFAULT 0,
            sort_order  INTEGER DEFAULT 0,
            created_at  TEXT DEFAULT (datetime('now','localtime')),
            updated_at  TEXT DEFAULT (datetime('now','localtime'))
        );

        CREATE TABLE IF NOT EXISTS config (
            key        TEXT PRIMARY KEY,
            value      TEXT NOT NULL,
            updated_at TEXT DEFAULT (datetime('now','localtime'))
        );

        CREATE TABLE IF NOT EXISTS domains (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            domain      TEXT NOT NULL UNIQUE,
            enabled     INTEGER DEFAULT 1,
            created_at  TEXT DEFAULT (datetime('now','localtime')),
            updated_at  TEXT DEFAULT (datetime('now','localtime'))
        );"
    )?;

    ensure_table_column(&conn, "accounts", "plan", "TEXT")?;
    ensure_table_column(&conn, "accounts", "credits", "INTEGER")?;
    ensure_table_column(&conn, "accounts", "group_id", "INTEGER")?;
    ensure_table_column(&conn, "account_groups", "pinned", "INTEGER DEFAULT 0")?;
    ensure_table_column(&conn, "account_groups", "is_default", "INTEGER DEFAULT 0")?;
    ensure_table_column(&conn, "account_groups", "sort_order", "INTEGER DEFAULT 0")?;
    prune_legacy_accounts_columns(&conn)?;
    prune_legacy_tables(&conn)?;
    normalize_and_prune_config(&conn)?;
    sync_register_domain_id(&conn)?;

    let default_group_id = ensure_default_group(&conn)?;
    conn.execute(
        "UPDATE accounts
         SET group_id = ?1, updated_at = datetime('now','localtime')
         WHERE group_id IS NULL",
        params![default_group_id],
    )?;
    Ok(conn)
}

fn ensure_table_column(conn: &Connection, table: &str, column: &str, definition: &str) -> Result<()> {
    let pragma_sql = format!("PRAGMA table_info({})", table);
    let mut stmt = conn.prepare(&pragma_sql)?;
    let mut rows = stmt.query([])?;
    let mut exists = false;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name == column {
            exists = true;
            break;
        }
    }

    if !exists {
        let sql = format!("ALTER TABLE {} ADD COLUMN {} {}", table, column, definition);
        conn.execute(&sql, [])?;
    }
    Ok(())
}

fn dirs_next() -> Option<std::path::PathBuf> {
    dirs::data_local_dir().map(|p| p.join("orchids-register"))
}

fn prune_legacy_accounts_columns(conn: &Connection) -> Result<()> {
    const LEGACY_COLUMNS: [&str; 14] = [
        "upstream_verify_ok",
        "upstream_verify_message",
        "login_success",
        "login_session_id",
        "login_user_id",
        "login_jwt",
        "login_credits",
        "login_plan",
        "auth_me_success",
        "auth_me_credits",
        "auth_me_plan",
        "desktop_touch_ok",
        "desktop_tokens_ok",
        "desktop_session_usable",
    ];

    let mut stmt = conn.prepare("PRAGMA table_info(accounts)")?;
    let columns: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<Vec<_>>>()?;

    let has_legacy = LEGACY_COLUMNS
        .iter()
        .any(|legacy| columns.iter().any(|col| col == legacy));
    if !has_legacy {
        return Ok(());
    }

    conn.execute_batch(
        "BEGIN IMMEDIATE;
         CREATE TABLE accounts__new (
            id                      INTEGER PRIMARY KEY AUTOINCREMENT,
            email                   TEXT NOT NULL,
            password                TEXT NOT NULL,
            sign_up_id              TEXT,
            email_code              TEXT,
            register_complete       INTEGER DEFAULT 0,
            created_session_id      TEXT,
            created_user_id         TEXT,
            client_cookie           TEXT,
            desktop_jwt             TEXT,
            status                  TEXT DEFAULT 'pending' CHECK(status IN ('pending','running','complete','failed')),
            error_message           TEXT,
            batch_id                TEXT,
            group_id                INTEGER,
            plan                    TEXT,
            credits                 INTEGER,
            created_at              TEXT DEFAULT (datetime('now','localtime')),
            updated_at              TEXT DEFAULT (datetime('now','localtime'))
         );

         INSERT INTO accounts__new (
            id, email, password, sign_up_id, email_code, register_complete,
            created_session_id, created_user_id, client_cookie, desktop_jwt,
            status, error_message, batch_id, group_id, plan, credits, created_at, updated_at
         )
         SELECT
            id, email, password, sign_up_id, email_code, register_complete,
            created_session_id, created_user_id, client_cookie, desktop_jwt,
            status, error_message, batch_id, group_id, plan, credits, created_at, updated_at
         FROM accounts;

         DROP TABLE accounts;
         ALTER TABLE accounts__new RENAME TO accounts;
         COMMIT;"
    )?;

    Ok(())
}

fn prune_legacy_tables(conn: &Connection) -> Result<()> {
    conn.execute("DROP TABLE IF EXISTS proxy_accounts", [])?;
    Ok(())
}

fn normalize_and_prune_config(conn: &Connection) -> Result<()> {
    let keep_proxy = get_config_value(conn, "proxy")?
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty());

    if keep_proxy.is_none() {
        let proxy_address = get_config_value(conn, "proxy_address")?
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty());

        let gateway_host = get_config_value(conn, "gateway_host")?
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty());
        let gateway_port = get_config_value(conn, "gateway_port")?
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty());
        let proxy_scheme = get_config_value(conn, "proxy_scheme")?
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "http".to_string());
        let proxy_source_url = get_config_value(conn, "proxy_source_url")?
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty());

        let merged_proxy = if let Some(v) = proxy_address {
            Some(v)
        } else if let (Some(host), Some(port)) = (gateway_host, gateway_port) {
            Some(format!("{}://{}:{}", proxy_scheme, host, port))
        } else {
            proxy_source_url
        };

        if let Some(proxy_value) = merged_proxy {
            save_config(conn, "proxy", &proxy_value)?;
        }
    }

    for legacy_key in [
        "proxy_address",
        "proxy_enabled",
        "proxy_source_url",
        "gateway_host",
        "gateway_port",
        "proxy_scheme",
    ] {
        delete_config(conn, legacy_key)?;
    }

    Ok(())
}

fn get_config_value(conn: &Connection, key: &str) -> Result<Option<String>> {
    conn.query_row(
        "SELECT value FROM config WHERE key = ?1",
        params![key],
        |row| row.get::<_, String>(0),
    )
    .optional()
}

fn delete_config(conn: &Connection, key: &str) -> Result<()> {
    conn.execute("DELETE FROM config WHERE key = ?1", params![key])?;
    Ok(())
}

pub fn sync_register_domain_id(conn: &Connection) -> Result<Option<i64>> {
    let current_id = get_config_value(conn, "register_domain_id")?
        .and_then(|v| v.trim().parse::<i64>().ok());

    if let Some(domain_id) = current_id {
        if let Some(domain) = get_domain_by_id(conn, domain_id)? {
            if domain.enabled && !domain.domain.trim().is_empty() {
                return Ok(Some(domain_id));
            }
        }
    }

    if let Some(domain) = list_domains(conn)?
        .into_iter()
        .find(|item| item.enabled && !item.domain.trim().is_empty())
    {
        save_config(conn, "register_domain_id", &domain.id.to_string())?;
        return Ok(Some(domain.id));
    }

    delete_config(conn, "register_domain_id")?;
    delete_config(conn, "register_domain_rr_index")?;
    Ok(None)
}

pub fn insert_account(conn: &Connection, account: &NewAccount) -> Result<i64> {
    let group_id = match account.group_id {
        Some(v) => v,
        None => ensure_default_group(conn)?,
    };

    conn.execute(
        "INSERT INTO accounts (email, password, status, batch_id, group_id)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            account.email,
            account.password,
            account.status,
            account.batch_id,
            group_id
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

#[derive(Debug, Clone)]
pub struct NewAccount {
    pub email: String,
    pub password: String,
    pub status: String,
    pub batch_id: Option<String>,
    pub group_id: Option<i64>,
}

pub fn update_account_result(
    conn: &Connection,
    id: i64,
    email: &str,
    sign_up_id: Option<&str>,
    email_code: Option<&str>,
    register_complete: bool,
    created_session_id: Option<&str>,
    created_user_id: Option<&str>,
    client_cookie: Option<&str>,
    desktop_jwt: Option<&str>,
    status: &str,
    error_message: Option<&str>,
) -> Result<()> {
    conn.execute(
        "UPDATE accounts SET
            email = ?2,
            sign_up_id = ?3,
            email_code = ?4,
            register_complete = ?5,
            created_session_id = ?6,
            created_user_id = ?7,
            client_cookie = ?8,
            desktop_jwt = ?9,
            status = ?10,
            error_message = ?11,
            updated_at = datetime('now','localtime')
         WHERE id = ?1",
        params![
            id,
            email,
            sign_up_id,
            email_code,
            register_complete as i32,
            created_session_id,
            created_user_id,
            client_cookie,
            desktop_jwt,
            status,
            error_message,
        ],
    )?;
    Ok(())
}

pub fn get_all_accounts(
    conn: &Connection,
    status_filter: Option<&str>,
    group_filter: Option<i64>,
) -> Result<Vec<Account>> {
    let mut sql = "SELECT
            a.id,
            a.email,
            a.password,
            a.sign_up_id,
            a.email_code,
            a.register_complete,
            a.created_session_id,
            a.created_user_id,
            a.client_cookie,
            a.desktop_jwt,
            a.status,
            a.error_message,
            a.batch_id,
            a.plan,
            a.credits,
            a.created_at,
            a.updated_at,
            COALESCE(a.group_id, 0) AS group_id,
            COALESCE(g.name, '默认分组') AS group_name
        FROM accounts a
        LEFT JOIN account_groups g ON g.id = a.group_id"
        .to_string();

    let mut conditions: Vec<&str> = Vec::new();
    let mut query_params: Vec<Value> = Vec::new();

    if let Some(status) = status_filter {
        conditions.push("a.status = ?");
        query_params.push(Value::from(status.to_string()));
    }
    if let Some(group_id) = group_filter {
        conditions.push("a.group_id = ?");
        query_params.push(Value::from(group_id));
    }
    if !conditions.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&conditions.join(" AND "));
    }
    sql.push_str(" ORDER BY a.id DESC");

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params_from_iter(query_params.iter()), |row| {
        Ok(Account {
            id: row.get(0)?,
            email: row.get(1)?,
            password: row.get(2)?,
            sign_up_id: row.get(3)?,
            email_code: row.get(4)?,
            register_complete: row.get::<_, i32>(5)? != 0,
            created_session_id: row.get(6)?,
            created_user_id: row.get(7)?,
            client_cookie: row.get(8)?,
            desktop_jwt: row.get(9)?,
            status: row.get(10)?,
            error_message: row.get(11)?,
            batch_id: row.get(12)?,
            plan: row.get(13)?,
            credits: row.get(14)?,
            created_at: row.get(15)?,
            updated_at: row.get(16)?,
            group_id: row.get(17)?,
            group_name: row.get(18)?,
        })
    })?;

    rows.collect()
}

pub fn update_account_plan_credits(
    conn: &Connection,
    id: i64,
    plan: Option<&str>,
    credits: Option<i64>,
) -> Result<()> {
    conn.execute(
        "UPDATE accounts SET
            plan = ?2,
            credits = ?3,
            updated_at = datetime('now','localtime')
         WHERE id = ?1",
        params![id, plan, credits],
    )?;
    Ok(())
}

pub fn delete_account_by_id(conn: &Connection, id: i64) -> Result<usize> {
    conn.execute("DELETE FROM accounts WHERE id = ?1", params![id])
}

pub fn delete_accounts_by_ids(conn: &Connection, ids: &[i64]) -> Result<usize> {
    if ids.is_empty() {
        return Ok(0);
    }
    let placeholders: Vec<String> = ids.iter().map(|_| "?".to_string()).collect();
    let sql = format!("DELETE FROM accounts WHERE id IN ({})", placeholders.join(","));
    let params: Vec<Box<dyn ToSql>> = ids
        .iter()
        .map(|id| Box::new(*id) as Box<dyn ToSql>)
        .collect();
    let params_refs: Vec<&dyn ToSql> = params.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, params_refs.as_slice())
}

pub fn ensure_default_group(conn: &Connection) -> Result<i64> {
    let default_id = conn
        .query_row(
            "SELECT id FROM account_groups WHERE is_default = 1 ORDER BY id LIMIT 1",
            [],
            |row| row.get::<_, i64>(0),
        )
        .optional()?;

    let id = if let Some(id) = default_id {
        id
    } else {
        let by_name = conn
            .query_row(
                "SELECT id FROM account_groups WHERE name = '默认分组' ORDER BY id LIMIT 1",
                [],
                |row| row.get::<_, i64>(0),
            )
            .optional()?;

        if let Some(id) = by_name {
            id
        } else {
            conn.execute(
                "INSERT INTO account_groups (name, pinned, is_default, sort_order, created_at, updated_at)
                 VALUES ('默认分组', 1, 1, 0, datetime('now','localtime'), datetime('now','localtime'))",
                [],
            )?;
            conn.last_insert_rowid()
        }
    };

    conn.execute(
        "UPDATE account_groups
         SET is_default = CASE WHEN id = ?1 THEN 1 ELSE 0 END",
        params![id],
    )?;
    conn.execute(
        "UPDATE account_groups
         SET name = '默认分组',
             pinned = 1,
             sort_order = 0,
             updated_at = datetime('now','localtime')
         WHERE id = ?1",
        params![id],
    )?;

    normalize_group_sort(conn)?;
    Ok(id)
}

pub fn list_account_groups(conn: &Connection) -> Result<Vec<AccountGroup>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, pinned, is_default, sort_order, created_at, updated_at
         FROM account_groups
         ORDER BY pinned DESC, is_default DESC, sort_order ASC, id ASC",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(AccountGroup {
            id: row.get(0)?,
            name: row.get(1)?,
            pinned: row.get::<_, i32>(2)? != 0,
            is_default: row.get::<_, i32>(3)? != 0,
            sort_order: row.get(4)?,
            created_at: row.get(5)?,
            updated_at: row.get(6)?,
        })
    })?;

    rows.collect()
}

pub fn get_account_group_by_id(conn: &Connection, id: i64) -> Result<Option<AccountGroup>> {
    conn.query_row(
        "SELECT id, name, pinned, is_default, sort_order, created_at, updated_at
         FROM account_groups
         WHERE id = ?1",
        params![id],
        |row| {
            Ok(AccountGroup {
                id: row.get(0)?,
                name: row.get(1)?,
                pinned: row.get::<_, i32>(2)? != 0,
                is_default: row.get::<_, i32>(3)? != 0,
                sort_order: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        },
    )
    .optional()
}

pub fn create_account_group(conn: &Connection, name: &str) -> Result<i64> {
    let next_sort = conn.query_row(
        "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM account_groups WHERE pinned = 0",
        [],
        |row| row.get::<_, i64>(0),
    )?;

    conn.execute(
        "INSERT INTO account_groups (name, pinned, is_default, sort_order, created_at, updated_at)
         VALUES (?1, 0, 0, ?2, datetime('now','localtime'), datetime('now','localtime'))",
        params![name, next_sort],
    )?;
    normalize_group_sort(conn)?;
    Ok(conn.last_insert_rowid())
}

pub fn rename_account_group(conn: &Connection, id: i64, name: &str) -> Result<()> {
    conn.execute(
        "UPDATE account_groups
         SET name = ?2, updated_at = datetime('now','localtime')
         WHERE id = ?1",
        params![id, name],
    )?;
    Ok(())
}

pub fn delete_account_group(conn: &Connection, id: i64) -> Result<()> {
    let default_group_id = ensure_default_group(conn)?;
    conn.execute(
        "UPDATE accounts
         SET group_id = ?1, updated_at = datetime('now','localtime')
         WHERE group_id = ?2",
        params![default_group_id, id],
    )?;
    conn.execute("DELETE FROM account_groups WHERE id = ?1", params![id])?;
    normalize_group_sort(conn)?;
    Ok(())
}

pub fn set_account_group_pinned(conn: &Connection, id: i64, pinned: bool) -> Result<()> {
    let pin_int = if pinned { 1 } else { 0 };
    let next_sort = conn.query_row(
        "SELECT COALESCE(MAX(sort_order), -1) + 1
         FROM account_groups
         WHERE pinned = ?1 AND id <> ?2",
        params![pin_int, id],
        |row| row.get::<_, i64>(0),
    )?;

    conn.execute(
        "UPDATE account_groups
         SET pinned = ?2, sort_order = ?3, updated_at = datetime('now','localtime')
         WHERE id = ?1",
        params![id, pin_int, next_sort],
    )?;
    normalize_group_sort(conn)?;
    Ok(())
}

pub fn move_account_group(conn: &Connection, id: i64, direction: &str) -> Result<()> {
    let group = match get_account_group_by_id(conn, id)? {
        Some(g) => g,
        None => return Ok(()),
    };

    let sql = if group.pinned {
        "SELECT id
         FROM account_groups
         WHERE pinned = 1 AND is_default = 0
         ORDER BY sort_order ASC, id ASC"
    } else {
        "SELECT id
         FROM account_groups
         WHERE pinned = 0
         ORDER BY sort_order ASC, id ASC"
    };
    let mut stmt = conn.prepare(sql)?;
    let ids: Vec<i64> = stmt
        .query_map([], |row| row.get::<_, i64>(0))?
        .collect::<Result<Vec<_>>>()?;

    let pos = match ids.iter().position(|gid| *gid == id) {
        Some(p) => p,
        None => return Ok(()),
    };

    let mut reordered = ids.clone();
    match direction {
        "up" if pos > 0 => reordered.swap(pos, pos - 1),
        "down" if pos + 1 < reordered.len() => reordered.swap(pos, pos + 1),
        _ => {}
    }

    for (idx, gid) in reordered.iter().enumerate() {
        conn.execute(
            "UPDATE account_groups
             SET sort_order = ?2, updated_at = datetime('now','localtime')
             WHERE id = ?1",
            params![gid, idx as i64],
        )?;
    }
    Ok(())
}

pub fn move_accounts_to_group(conn: &Connection, ids: &[i64], target_group_id: i64) -> Result<usize> {
    if ids.is_empty() {
        return Ok(0);
    }

    let placeholders: Vec<String> = (0..ids.len()).map(|idx| format!("?{}", idx + 2)).collect();
    let sql = format!(
        "UPDATE accounts
         SET group_id = ?1, updated_at = datetime('now','localtime')
         WHERE id IN ({})",
        placeholders.join(",")
    );

    let mut params: Vec<Value> = Vec::with_capacity(ids.len() + 1);
    params.push(Value::from(target_group_id));
    for id in ids {
        params.push(Value::from(*id));
    }

    conn.execute(&sql, params_from_iter(params.iter()))
}

fn normalize_group_sort(conn: &Connection) -> Result<()> {
    for pinned in [1, 0] {
        let sql = if pinned == 1 {
            "SELECT id
             FROM account_groups
             WHERE pinned = 1
             ORDER BY is_default DESC, sort_order ASC, id ASC"
        } else {
            "SELECT id
             FROM account_groups
             WHERE pinned = 0
             ORDER BY sort_order ASC, id ASC"
        };
        let mut stmt = conn.prepare(sql)?;
        let ids: Vec<i64> = stmt
            .query_map([], |row| row.get::<_, i64>(0))?
            .collect::<Result<Vec<_>>>()?;

        for (idx, id) in ids.iter().enumerate() {
            conn.execute(
                "UPDATE account_groups
                 SET sort_order = ?2
                 WHERE id = ?1",
                params![id, idx as i64],
            )?;
        }
    }
    Ok(())
}

pub fn list_domains(conn: &Connection) -> Result<Vec<Domain>> {
    let mut stmt = conn.prepare(
        "SELECT id, domain, enabled, created_at, updated_at
         FROM domains
         ORDER BY id ASC",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(Domain {
            id: row.get(0)?,
            domain: row.get(1)?,
            enabled: row.get::<_, i32>(2)? != 0,
            created_at: row.get(3)?,
            updated_at: row.get(4)?,
        })
    })?;

    rows.collect()
}

pub fn get_domain_by_id(conn: &Connection, id: i64) -> Result<Option<Domain>> {
    conn.query_row(
        "SELECT id, domain, enabled, created_at, updated_at
         FROM domains
         WHERE id = ?1",
        params![id],
        |row| {
            Ok(Domain {
                id: row.get(0)?,
                domain: row.get(1)?,
                enabled: row.get::<_, i32>(2)? != 0,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        },
    )
    .optional()
}

pub fn create_domain(conn: &Connection, domain: &str, enabled: bool) -> Result<i64> {
    conn.execute(
        "INSERT INTO domains (domain, enabled, created_at, updated_at)
         VALUES (?1, ?2, datetime('now','localtime'), datetime('now','localtime'))",
        params![domain, if enabled { 1 } else { 0 }],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn update_domain(conn: &Connection, id: i64, domain: &str, enabled: bool) -> Result<()> {
    conn.execute(
        "UPDATE domains
         SET domain = ?2,
             enabled = ?3,
             updated_at = datetime('now','localtime')
         WHERE id = ?1",
        params![id, domain, if enabled { 1 } else { 0 }],
    )?;
    Ok(())
}

pub fn delete_domain(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM domains WHERE id = ?1", params![id])?;
    Ok(())
}

// Config CRUD
pub fn get_all_config(conn: &Connection) -> Result<HashMap<String, String>> {
    let mut stmt = conn.prepare("SELECT key, value FROM config")?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;

    let mut map = HashMap::new();
    for row in rows {
        let (k, v) = row?;
        map.insert(k, v);
    }
    Ok(map)
}

pub fn save_config(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO config (key, value, updated_at) VALUES (?1, ?2, datetime('now','localtime'))
         ON CONFLICT(key) DO UPDATE SET value = ?2, updated_at = datetime('now','localtime')",
        params![key, value],
    )?;
    Ok(())
}

pub fn reset_config(conn: &Connection) -> Result<()> {
    conn.execute("DELETE FROM config", [])?;
    Ok(())
}
