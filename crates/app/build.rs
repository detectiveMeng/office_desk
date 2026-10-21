//! 构建脚本：把图标资源编进 exe。
//!
//! GPUI（gpui-pre-windows）启动时用 `LoadImageW(模块句柄, 序号 1, IMAGE_ICON)`
//! 读取 exe 内图标，作为窗口类图标供标题栏 / 任务栏使用；因此资源 ID 必须是 1。

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.rc");
    println!("cargo:rerun-if-changed=assets/icon.ico");
    embed_resource::compile("assets/icon.rc", embed_resource::NONE)
        .manifest_required()
        .unwrap_or_else(|err| panic!("图标资源编译失败：{err}"));
}
