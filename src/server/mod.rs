pub mod handler;

use tokio::net::UnixListener;
use std::path::PathBuf;
use anyhow::Result;

pub async fn start_server() -> Result<()> {
    let socket_path = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".vision-bridge/vbri.sock");

    // 删除旧的 socket 文件
    if socket_path.exists() {
        std::fs::remove_file(&socket_path)?;
    }

    let listener = UnixListener::bind(&socket_path)?;
    println!("✓ 服务已启动，监听 {}", socket_path.display());

    loop {
        match listener.accept().await {
            Ok((stream, _addr)) => {
                tokio::spawn(async move {
                    if let Err(e) = handler::handle_connection(stream).await {
                        eprintln!("处理连接错误: {}", e);
                    }
                });
            }
            Err(e) => {
                eprintln!("接受连接错误: {}", e);
            }
        }
    }
}
