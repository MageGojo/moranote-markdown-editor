fn main() {
    // 仅在 Windows 上把应用图标嵌入可执行文件（任务栏 / 资源管理器显示用）。
    #[cfg(target_os = "windows")]
    {
        // app.rc 引用了 assets/app-icons/moranote.ico。
        let _ = embed_resource::compile("assets/windows/app.rc", embed_resource::NONE);
        println!("cargo:rerun-if-changed=assets/windows/app.rc");
        println!("cargo:rerun-if-changed=assets/app-icons/moranote.ico");
    }
}
