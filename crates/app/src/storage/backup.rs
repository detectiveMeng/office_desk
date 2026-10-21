//! 备份与完整性检查。

use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::Connection;

/// 在线一致备份：`VACUUM INTO` 生成一个压缩后的完整副本。
pub fn backup_to(conn: &Connection, target: impl AsRef<Path>) -> Result<()> {
    let target = target.as_ref();
    if target.exists() {
        std::fs::remove_file(target)
            .with_context(|| format!("覆盖旧备份失败: {}", target.display()))?;
    }
    if let Some(dir) = target.parent() {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir)?;
        }
    }
    conn.execute("VACUUM INTO ?1", [target.to_string_lossy().as_ref()])
        .with_context(|| format!("备份失败: {}", target.display()))?;
    Ok(())
}

/// `PRAGMA integrity_check` 是否返回 ok。
pub fn integrity_ok(conn: &Connection) -> bool {
    conn.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))
        .map(|v| v == "ok")
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integrity_ok_on_fresh_db() {
        let conn = Connection::open_in_memory().unwrap();
        assert!(integrity_ok(&conn));
    }
}
