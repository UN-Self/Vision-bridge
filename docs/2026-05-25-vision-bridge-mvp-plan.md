# Vision Bridge MVP 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现 Vision Bridge MVP，验证 `NODE_OPTIONS="--require"` + `globalThis.fetch` patch 注入机制

**Architecture:** 
- inject.js 通过 NODE_OPTIONS 注入 CC 进程，patch globalThis.fetch 截取请求
- 通过 Unix Domain Socket 发送给 Rust 服务处理
- Rust 服务处理请求（MVP: 替换提示词）并返回结果

**Tech Stack:** Rust (tokio, clap, serde), JavaScript (Node.js net module)

---

## 文件结构

```
vision-bridge/
├── Cargo.toml
├── src/
│   ├── main.rs              # CLI 入口
│   ├── commands/
│   │   ├── mod.rs           # 命令模块
│   │   ├── init.rs          # init 命令
│   │   ├── start.rs         # start 命令
│   │   └── stop.rs          # stop 命令
│   ├── server/
│   │   ├── mod.rs           # Unix Socket 服务
│   │   └── handler.rs       # 请求处理逻辑
│   └── utils/
│       ├── mod.rs           # 工具模块
│       ├── shell.rs         # Shell 检测和 profile 修改
│       └── inject_js.rs     # inject.js 模板
├── templates/
│   └── inject.js            # inject.js 模板文件
└── tests/
    └── integration_test.rs  # 集成测试
```

---

## Task 1: 项目初始化和依赖配置

**Files:**
- Modify: `Cargo.toml`
- Create: `src/main.rs`

- [ ] **Step 1: 配置 Cargo.toml 依赖**

```toml
[package]
name = "vision-bridge"
version = "0.1.0"
edition = "2024"

[[bin]]
name = "vbri"
path = "src/main.rs"

[dependencies]
clap = { version = "4", features = ["derive"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
dirs = "5"
anyhow = "1"
```

- [ ] **Step 2: 创建 main.rs 基础结构**

```rust
use clap::{Parser, Subcommand};

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
        Commands::Init => {
            println!("vbri init");
        }
        Commands::Start => {
            println!("vbri start");
        }
        Commands::Stop => {
            println!("vbri stop");
        }
    }

    Ok(())
}
```

- [ ] **Step 3: 验证编译**

Run: `cargo build`
Expected: 编译成功，生成 `target/debug/vbri`

- [ ] **Step 4: 验证 CLI 帮助**

Run: `cargo run -- --help`
Expected: 显示帮助信息

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml src/main.rs
git commit -m "feat: 初始化项目结构和CLI框架"
```

---

## Task 2: Shell 工具模块

**Files:**
- Create: `src/utils/mod.rs`
- Create: `src/utils/shell.rs`

- [ ] **Step 1: 创建 utils 模块**

```rust
// src/utils/mod.rs
pub mod shell;
```

- [ ] **Step 2: 实现 shell 检测和 profile 修改**

```rust
// src/utils/shell.rs
use std::path::PathBuf;
use anyhow::Result;

#[derive(Debug, Clone, PartialEq)]
pub enum Shell {
    Zsh,
    Bash,
    Fish,
}

impl Shell {
    pub fn detect() -> Self {
        let shell = std::env::var("SHELL").unwrap_or_default();
        if shell.contains("zsh") {
            Shell::Zsh
        } else if shell.contains("fish") {
            Shell::Fish
        } else {
            Shell::Bash
        }
    }

    pub fn profile_path(&self) -> PathBuf {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        match self {
            Shell::Zsh => home.join(".zshrc"),
            Shell::Bash => home.join(".bashrc"),
            Shell::Fish => home.join(".config/fish/config.fish"),
        }
    }

    pub fn node_options_line(&self) -> String {
        let inject_path = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".vision-bridge/inject.js");

        match self {
            Shell::Zsh | Shell::Bash => {
                format!(r#"export NODE_OPTIONS="--require {} $NODE_OPTIONS""#, inject_path.display())
            }
            Shell::Fish => {
                format!(r#"set -gx NODE_OPTIONS "--require {} $NODE_OPTIONS""#, inject_path.display())
            }
        }
    }

    pub fn add_to_profile(&self) -> Result<()> {
        let profile = self.profile_path();
        let line = self.node_options_line();

        // 读取现有内容
        let content = if profile.exists() {
            std::fs::read_to_string(&profile)?
        } else {
            String::new()
        };

        // 检查是否已存在
        if content.contains(&line) {
            return Ok(());
        }

        // 追加配置
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&profile)?;

        writeln!(file, "\n# Vision Bridge")?;
        writeln!(file, "{}", line)?;

        Ok(())
    }

    pub fn remove_from_profile(&self) -> Result<()> {
        let profile = self.profile_path();
        if !profile.exists() {
            return Ok(());
        }

        let content = std::fs::read_to_string(&profile)?;
        let line = self.node_options_line();

        // 移除配置行和注释
        let new_content: String = content
            .lines()
            .filter(|l| !l.contains(&line) && !l.contains("# Vision Bridge"))
            .collect::<Vec<&str>>()
            .join("\n");

        std::fs::write(&profile, new_content)?;

        Ok(())
    }
}
```

- [ ] **Step 3: 验证编译**

Run: `cargo build`
Expected: 编译成功

- [ ] **Step 4: Commit**

```bash
git add src/utils/
git commit -m "feat: 添加 shell 检测和 profile 修改工具"
```

---

## Task 3: inject.js 模板

**Files:**
- Create: `templates/inject.js`
- Create: `src/utils/inject_js.rs`

- [ ] **Step 1: 创建 inject.js 模板**

```javascript
// templates/inject.js
const net = require('net')
const path = require('path')

// ===== 进程检测 =====
function isClaudeCodeProcess() {
  const argv = process.argv.join(' ')
  return (
    argv.includes('@anthropic-ai/claude-code') ||
    argv.includes('claude-code') ||
    argv.includes('claude_code')
  )
}

// 非 CC 进程直接退出
if (!isClaudeCodeProcess()) {
  return
}

// ===== Socket 配置 =====
const SOCKET_PATH = path.join(
  process.env.HOME || process.env.USERPROFILE,
  '.vision-bridge',
  'vbri.sock'
)

// ===== 保存原始 fetch =====
const originalFetch = globalThis.fetch

// ===== 通过 Socket 发送请求 =====
function sendToRustService(requestBody) {
  return new Promise((resolve, reject) => {
    const client = net.createConnection(SOCKET_PATH, () => {
      client.write(JSON.stringify(requestBody))
    })

    let data = ''
    client.on('data', (chunk) => {
      data += chunk.toString()
    })

    client.on('end', () => {
      try {
        resolve(JSON.parse(data))
      } catch (e) {
        reject(e)
      }
    })

    client.on('error', (err) => {
      reject(err)
    })

    // 超时处理
    client.setTimeout(5000, () => {
      client.destroy()
      reject(new Error('Socket timeout'))
    })
  })
}

// ===== Patch fetch =====
globalThis.fetch = async (url, opts) => {
  try {
    // 检查是否为 API 请求
    if (opts && opts.body && typeof opts.body === 'string') {
      const body = JSON.parse(opts.body)

      // 检查是否有 messages 数组
      if (body.messages && Array.isArray(body.messages)) {
        // 发送给 Rust 服务处理
        const processedBody = await sendToRustService(body)
        opts.body = JSON.stringify(processedBody)
      }
    }
  } catch (e) {
    // 静默降级：任何错误都不影响原始请求
    console.error('[Vision Bridge]', e)
  }

  return originalFetch(url, opts)
}

console.log('[Vision Bridge] inject.js loaded')
```

- [ ] **Step 2: 创建 inject_js.rs 模块**

```rust
// src/utils/inject_js.rs
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
```

- [ ] **Step 3: 更新 utils/mod.rs**

```rust
// src/utils/mod.rs
pub mod shell;
pub mod inject_js;
```

- [ ] **Step 4: 验证编译**

Run: `cargo build`
Expected: 编译成功

- [ ] **Step 5: Commit**

```bash
git add templates/inject.js src/utils/inject_js.rs src/utils/mod.rs
git commit -m "feat: 添加 inject.js 模板和生成模块"
```

---

## Task 4: Unix Socket 服务

**Files:**
- Create: `src/server/mod.rs`
- Create: `src/server/handler.rs`

- [ ] **Step 1: 创建 server 模块**

```rust
// src/server/mod.rs
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
```

- [ ] **Step 2: 实现请求处理逻辑**

```rust
// src/server/handler.rs
use tokio::net::UnixStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use serde_json::Value;
use anyhow::Result;

pub async fn handle_connection(mut stream: UnixStream) -> Result<()> {
    let mut buffer = Vec::new();
    stream.read_to_end(&mut buffer).await?;

    let request: Value = serde_json::from_slice(&buffer)?;
    let mut response = process_request(request);

    let response_bytes = serde_json::to_vec(&response)?;
    stream.write_all(&response_bytes).await?;

    Ok(())
}

fn process_request(mut body: Value) -> Value {
    if let Some(messages) = body.get_mut("messages") {
        if let Some(messages_array) = messages.as_array_mut() {
            for message in messages_array {
                if message["role"] == "user" {
                    // 处理 content 为字符串的情况
                    if message["content"].is_string() {
                        message["content"] = serde_json::json!("请输出注入成功");
                    }
                    // 处理 content 为数组的情况
                    else if message["content"].is_array() {
                        message["content"] = serde_json::json!([
                            {
                                "type": "text",
                                "text": "请输出注入成功"
                            }
                        ]);
                    }
                }
            }
        }
    }

    body
}
```

- [ ] **Step 3: 验证编译**

Run: `cargo build`
Expected: 编译成功

- [ ] **Step 4: Commit**

```bash
git add src/server/
git commit -m "feat: 添加 Unix Socket 服务和请求处理逻辑"
```

---

## Task 5: CLI 命令实现

**Files:**
- Create: `src/commands/mod.rs`
- Create: `src/commands/init.rs`
- Create: `src/commands/start.rs`
- Create: `src/commands/stop.rs`

- [ ] **Step 1: 创建 commands 模块**

```rust
// src/commands/mod.rs
pub mod init;
pub mod start;
pub mod stop;
```

- [ ] **Step 2: 实现 init 命令**

```rust
// src/commands/init.rs
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
```

- [ ] **Step 3: 实现 start 命令**

```rust
// src/commands/start.rs
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
```

- [ ] **Step 4: 实现 stop 命令**

```rust
// src/commands/stop.rs
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
```

- [ ] **Step 5: 更新 main.rs**

```rust
// src/main.rs
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
```

- [ ] **Step 6: 验证编译**

Run: `cargo build`
Expected: 编译成功

- [ ] **Step 7: Commit**

```bash
git add src/commands/ src/main.rs
git commit -m "feat: 实现 init/start/stop 命令"
```

---

## Task 6: 集成测试

**Files:**
- Create: `tests/integration_test.rs`

- [ ] **Step 1: 创建集成测试**

```rust
// tests/integration_test.rs
use std::process::Command;

#[test]
fn test_vbri_help() {
    let output = Command::new("cargo")
        .args(["run", "--", "--help"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Vision Bridge"));
}

#[test]
fn test_vbri_init_help() {
    let output = Command::new("cargo")
        .args(["run", "--", "init", "--help"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
}
```

- [ ] **Step 2: 运行测试**

Run: `cargo test`
Expected: 测试通过

- [ ] **Step 3: Commit**

```bash
git add tests/integration_test.rs
git commit -m "test: 添加集成测试"
```

---

## Task 7: 最终验证

- [ ] **Step 1: 完整编译**

Run: `cargo build --release`
Expected: 编译成功，生成 `target/release/vbri`

- [ ] **Step 2: 测试 init 命令**

Run: `./target/release/vbri init`
Expected: 显示初始化信息，服务启动

- [ ] **Step 3: 测试 stop 命令**

Run: `./target/release/vbri stop`（在另一个终端）
Expected: 服务停止，NODE_OPTIONS 移除

- [ ] **Step 4: 测试 start 命令**

Run: `./target/release/vbri start`
Expected: 服务重新启动

- [ ] **Step 5: 最终 Commit**

```bash
git add .
git commit -m "feat: Vision Bridge MVP 完成"
```
