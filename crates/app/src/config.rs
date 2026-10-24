//! 应用配置：数据目录下的 `config.toml`，当前只有主题一项。
//! 数据目录由 `storage::data_dir` 决定：安装版在 `%APPDATA%\NiuMa-Workstation`，绿色版在 exe 旁的 `data`。

use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::{storage, theme::ThemeMode};

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub theme: ThemeMode,
}

pub fn path() -> PathBuf {
    storage::data_dir().join("config.toml")
}

impl Config {
    /// 读取配置；文件缺失或损坏时返回默认值并补写一份。
    pub fn load() -> Self {
        let p = path();
        match std::fs::read_to_string(&p) {
            Ok(text) => toml::from_str(&text).unwrap_or_default(),
            Err(_) => {
                let config = Self::default();
                let _ = config.save();
                config
            }
        }
    }

    pub fn save(&self) -> Result<()> {
        let p = path();
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir)
                .with_context(|| format!("创建配置目录失败: {}", dir.display()))?;
        }
        let text = toml::to_string(self)?;
        std::fs::write(&p, text).with_context(|| format!("写入配置失败: {}", p.display()))
    }
}
