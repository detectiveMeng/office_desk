//! 双主题色板：浅色与深色一起交付，切换后立即生效并落盘到 config.toml。

use serde::{Deserialize, Serialize};

/// 主题模式
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    #[default]
    Light,
    Dark,
}

impl ThemeMode {
    pub fn label(self) -> &'static str {
        match self {
            ThemeMode::Light => "浅色",
            ThemeMode::Dark => "深色",
        }
    }

    pub fn toggled(self) -> Self {
        match self {
            ThemeMode::Light => ThemeMode::Dark,
            ThemeMode::Dark => ThemeMode::Light,
        }
    }

    /// 文本输入框的闪烁光标色。
    ///
    /// 组件库默认光标偏黑，深色主题下几乎看不见，因此深色主题固定用纯白，
    /// 浅色主题沿用正文墨色。
    pub fn caret_color(self) -> u32 {
        match self {
            ThemeMode::Light => 0x152420,
            ThemeMode::Dark => 0xffffff,
        }
    }
}

/// 一套完整色板。所有颜色均为 `0xRRGGBB`，渲染时用 `rgb()`。
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub bg: u32,
    pub paper: u32,
    pub line: u32,
    pub line2: u32,
    pub ink: u32,
    pub ink2: u32,
    pub ink3: u32,
    pub teal: u32,
    pub teal_soft: u32,
    pub teal_ink: u32,
    pub red: u32,
    pub red_soft: u32,
    pub amber: u32,
    pub amber_soft: u32,
    pub blue: u32,
    pub blue_soft: u32,
    pub hover: u32,
}

impl Palette {
    pub fn of(mode: ThemeMode) -> Self {
        match mode {
            ThemeMode::Light => Self {
                bg: 0xf6f8f9,
                paper: 0xffffff,
                line: 0xe2e9e7,
                line2: 0xeef2f1,
                ink: 0x152420,
                ink2: 0x4a5c58,
                ink3: 0x7c8b87,
                teal: 0x0f766e,
                teal_soft: 0xe8f4f2,
                teal_ink: 0x0b4f4a,
                red: 0xc0392b,
                red_soft: 0xfdf0ee,
                amber: 0xa86413,
                amber_soft: 0xfdf4e3,
                blue: 0x3a6ea5,
                blue_soft: 0xeef3f8,
                hover: 0xf0f4f3,
            },
            ThemeMode::Dark => Self {
                bg: 0x101614,
                paper: 0x17201d,
                line: 0x293632,
                line2: 0x212c29,
                ink: 0xe8f0ee,
                ink2: 0xa9bbb7,
                ink3: 0x7c8b87,
                teal: 0x2dd4bf,
                teal_soft: 0x13322e,
                teal_ink: 0xa7f3e8,
                red: 0xf87171,
                red_soft: 0x2a1a18,
                amber: 0xf0b866,
                amber_soft: 0x2a2318,
                blue: 0x8fb8e8,
                blue_soft: 0x1a2530,
                hover: 0x1e2a27,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_caret_is_white_and_light_caret_is_ink() {
        assert_eq!(ThemeMode::Dark.caret_color(), 0xffffff);
        assert_eq!(ThemeMode::Light.caret_color(), Palette::of(ThemeMode::Light).ink);
    }
}
