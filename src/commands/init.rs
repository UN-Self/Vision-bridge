use anyhow::Result;
use crate::utils::{shell, inject_js};
use crate::server;

pub async fn execute() -> Result<()> {
    println!("Vision Bridge 初始化...");

    // 1. 创建目录
    let bridge_dir = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".vision-bridge");
    std::fs::create_dir_all(&bridge_dir)?;
    println!("✓ 已创建 ~/.vision-bridge/");

    // 2. 生成 inject.js
    inject_js::generate_inject_js()?;
    println!("✓ 已生成 inject.js");

    // 3. 配置 NODE_OPTIONS
    let shell = shell::Shell::detect();
    shell.add_to_profile()?;
    println!("✓ NODE_OPTIONS 已配置");

    // 4. 启动后台服务
    println!("✓ 后台服务已启动");
    println!("\n请重启终端或运行: source {}", shell.profile_path().display());

    // 启动服务（阻塞）
    server::start_server().await?;

    Ok(())
}
