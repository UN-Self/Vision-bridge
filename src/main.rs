use clap::{Parser, Subcommand};

mod commands;
mod server;
mod utils;

#[derive(Parser)]
#[command(name = "vbri")]
#[command(about = "Vision Bridge - Claude Code 图片转文本注入工具")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 初始化配置并启动服务
    Init,
    /// 启动后台服务
    Start,
    /// 停止后台服务
    Stop,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init => commands::init::execute().await?,
        Commands::Start => commands::start::execute().await?,
        Commands::Stop => commands::stop::execute().await?,
    }

    Ok(())
}
