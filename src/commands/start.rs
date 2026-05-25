use anyhow::Result;
use crate::utils::shell;
use crate::server;

pub async fn execute() -> Result<()> {
    // 1. 配置 NODE_OPTIONS
    let shell = shell::Shell::detect();
    shell.add_to_profile()?;
    println!("✓ NODE_OPTIONS 已配置");

    // 2. 启动服务
    println!("✓ 后台服务已启动");
    server::start_server().await?;

    Ok(())
}
