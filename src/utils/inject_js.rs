use std::path::PathBuf;
use anyhow::Result;

pub fn generate_inject_js() -> Result<()> {
    let bridge_dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".vision-bridge");

    // 创建目录
    std::fs::create_dir_all(&bridge_dir)?;

    // 写入 inject.js
    let inject_js = include_str!("../../templates/inject.js");
    std::fs::write(bridge_dir.join("inject.js"), inject_js)?;

    Ok(())
}
