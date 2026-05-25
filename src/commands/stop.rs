use anyhow::Result;
use std::path::PathBuf;
use crate::utils::shell;

pub async fn execute() -> Result<()> {
    // 1. 停止服务（删除 socket 文件）
    let socket_path = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".vision-bridge/vbri.sock");

    if socket_path.exists() {
        std::fs::remove_file(&socket_path)?;
        println!("✓ 后台服务已停止");
    } else {
        println!("服务未运行");
    }

    // 2. 移除 NODE_OPTIONS
    let shell = shell::Shell::detect();
    shell.remove_from_profile()?;
    println!("✓ NODE_OPTIONS 已移除");

    println!("二进制文件请通过包管理器卸载");

    Ok(())
}
