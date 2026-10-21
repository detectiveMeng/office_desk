//! 办公工作台入口：读取配置 → 打开 SQLite → 首次种子 → 跨天自动顺延 → 主窗口。

// 发布版不弹控制台窗口（调试版保留，便于看日志）
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod seed;
mod state;
mod storage;
mod theme;
mod views;

use chrono::Local;
use gpui_kit::*;
use office_desk_core::domain::todo::run_auto_rollover;

use state::Workspace;
use storage::Storage;

fn main() -> anyhow::Result<()> {
    let config = config::Config::load();

    let mut storage = Storage::open_default()?;
    if !storage.integrity_ok() {
        anyhow::bail!("数据库完整性检查未通过，请先备份后重建数据库");
    }

    let mut data = storage.load()?;
    let now = Local::now().naive_local();

    // 首次启动写入示例数据；已有任何数据的旧库不会被覆盖
    if data.seeded_at.is_none() {
        data = seed::seed(now.date(), now);
        storage.save(&data)?;
    }

    if run_auto_rollover(&mut data, now.date(), now) {
        storage.save(&data)?;
    }

    gpui_kit::application().run(move |cx| {
        gpui_kit::init(cx);

        let options = WindowOptions {
            titlebar: Some(TitlebarOptions {
                title: Some("办公工作台".into()),
                // 去掉系统标题栏，改由应用自绘（见 Workspace::render_titlebar），
                // 让窗口顶部与内容区成为同一块连续表面。
                appears_transparent: true,
                ..Default::default()
            }),
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: point(px(40.), px(20.)),
                size: size(px(1120.), px(600.)),
            })),
            ..Default::default()
        };

        let data = data.clone();
        gpui_kit::open_window(options, cx, move |window, cx| {
            cx.new(|cx| Workspace::new(window, cx, storage, data.clone(), config))
        })
        .expect("打开主窗口失败");
    });

    Ok(())
}
