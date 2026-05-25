# Vision Bridge MVP 设计文档

> 日期：2026-05-25
> 状态：设计完成
> 目标：验证注入机制可行性

## 问题背景

Vision Bridge 需要通过 `NODE_OPTIONS="--require"` 注入 inject.js 到 Claude Code (CC) 进程，patch `globalThis.fetch` 来拦截和修改 API 请求。MVP 阶段需要验证这个注入机制是否可行。

## MVP 目标

1. 验证 `NODE_OPTIONS="--require"` + `globalThis.fetch` patch 机制
2. 实现基本的 CLI 工具（`vbri`）
3. 确保 stop 功能安全可靠

---

## 架构

```
┌─────────────────────────────────────────────────┐
│  vbri init                                       │
│  → 创建 ~/.vision-bridge/ 目录                   │
│  → 生成 inject.js                                │
│  → 启动后台服务                                  │
│  → 在 shell profile 添加 NODE_OPTIONS            │
└─────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────┐
│  CC 启动 (Node.js 进程)                          │
│                                                  │
│  NODE_OPTIONS 加载 inject.js                     │
│    ↓                                             │
│  inject.js 进程检测：是否为 CC 进程？            │
│    ├── 否 → 直接退出，不做任何 patch             │
│    └── 是 → 继续                                 │
│    ↓                                             │
│  inject.js monkey-patch globalThis.fetch         │
│    ↓                                             │
│  CC 构建 API 请求                                │
│    ↓                                             │
│  patched fetch 拦截请求:                          │
│    ├── 截取请求体                                │
│    ├── 通过 Unix Socket 发送给 Rust 服务         │
│    ├── 接收处理后的请求体                        │
│    └── originalFetch(url, modifiedOpts) 发出请求  │
│    ↓                                             │
│  用户看到 CC 输出"请输出注入成功"                │
└─────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────┐
│  Rust 服务（后台运行）                           │
│  → 监听 Unix Domain Socket                       │
│  → 接收 inject.js 的请求                         │
│  → 处理请求（MVP: 替换提示词）                   │
│  → 返回修改后的请求体                            │
└─────────────────────────────────────────────────┘
```

### 职责分离

| 组件 | 职责 |
|------|------|
| **inject.js** | 截取请求 + 塞入结果（轻量） |
| **Rust 服务** | 处理请求 + 返回结果（核心逻辑） |

---

## 组件

### 1. vbri CLI（Rust）

Rust 编写的命令行工具，单二进制分发。

| 命令 | 功能 |
|------|------|
| `vbri init` | 初始化：创建目录、生成 inject.js、启动后台服务、配置 NODE_OPTIONS |
| `vbri start` | 启动后台服务（用于 stop 后重新启动） |
| `vbri stop` | 停止后台服务、移除 NODE_OPTIONS 配置 |

#### 项目结构

```
vision-bridge/
├── Cargo.toml
├── src/
│   ├── main.rs          # CLI 入口
│   ├── commands/
│   │   ├── init.rs      # init 命令实现
│   │   ├── start.rs     # start 命令实现
│   │   └── stop.rs      # stop 命令实现
│   ├── server/
│   │   ├── mod.rs       # Unix Socket 服务
│   │   └── handler.rs   # 请求处理逻辑
│   └── utils/
│       ├── shell.rs     # Shell 检测和 profile 修改
│       └── inject_js.rs # inject.js 模板
├── templates/
│   └── inject.js        # inject.js 模板文件
└── ...
```

#### 核心依赖

- `clap` — 命令行参数解析
- `dirs` — 获取用户目录路径
- `tokio` — 异步运行时（处理 Unix Socket）
- `serde` / `serde_json` — JSON 处理

### 2. inject.js（JavaScript，由 Rust 生成）

注入到 CC 进程的脚本，职责：截取请求 + 塞入结果。

```javascript
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
// 连接模型：每次请求新建连接，client.end() 天然界定消息边界，无需额外 framing 协议
function sendToRustService(requestBody) {
  return new Promise((resolve, reject) => {
    const client = net.createConnection(SOCKET_PATH, () => {
      client.end(JSON.stringify(requestBody)) // write + half-close
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
    // 日志行为由配置控制，默认静默
  }

  return originalFetch(url, opts)
}

console.log('[Vision Bridge] inject.js loaded')
```

### 3. Rust 服务

监听 Unix Domain Socket，处理 inject.js 发送的请求。

```rust
// MVP 处理逻辑：将所有 user message 替换为"请输出注入成功"
fn process_request(body: &mut serde_json::Value) {
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
}
```

---

## 通信协议

### 请求格式（inject.js → Rust 服务）

```json
{
  "messages": [
    {
      "role": "user",
      "content": "用户的消息"
    }
  ],
  "model": "claude-3-5-sonnet-20241022",
  "max_tokens": 1024
}
```

### 响应格式（Rust 服务 → inject.js）

```json
{
  "messages": [
    {
      "role": "user",
      "content": "请输出注入成功"
    }
  ],
  "model": "claude-3-5-sonnet-20241022",
  "max_tokens": 1024
}
```

---

## Shell 适配

| Shell | Profile 文件 | 添加的配置 |
|-------|-------------|-----------|
| zsh | `~/.zshrc` | `export NODE_OPTIONS="--require ~/.vision-bridge/inject.js $NODE_OPTIONS"` |
| bash | `~/.bashrc` | 同上 |
| fish | `~/.config/fish/config.fish` | `set -gx NODE_OPTIONS "--require ~/.vision-bridge/inject.js $NODE_OPTIONS"` |

### 检测逻辑

- 读取 `SHELL` 环境变量判断当前 shell
- 自动适配对应的 profile 文件和语法

---

## Stop 功能

### `vbri stop` 流程

1. 停止后台服务进程
2. 从 shell profile 中移除 Vision Bridge 添加的行
3. 输出 stop 完成信息

### 安全性保证

- 只移除 Vision Bridge 添加的配置行
- 精确匹配字符串，不会误删其他配置
- 不删除 `~/.vision-bridge/` 目录（保留配置）

---

## 用户使用流程

```bash
# 1. 安装（包管理器）
brew install vision-bridge
# 或
cargo install vision-bridge

# 2. 初始化（首次）
vbri init
# ✓ 已创建 ~/.vision-bridge/
# ✓ 已生成 inject.js
# ✓ 后台服务已启动
# ✓ NODE_OPTIONS 已配置
# ✓ 请重启终端或 source ~/.zshrc

# 3. 直接使用 CC
source ~/.zshrc  # 或重启终端
claude
# 在 CC 中发送任何消息，应该看到"请输出注入成功"

# 4. 停止服务
vbri stop
# ✓ 后台服务已停止
# ✓ NODE_OPTIONS 已移除
# ✓ 二进制文件请通过包管理器卸载

# 5. 重新启动
vbri start
# ✓ 后台服务已启动
# ✓ NODE_OPTIONS 已配置
```

---

## 关键技术约束

1. **NODE_OPTIONS 兼容性**：CC 使用 Node.js ≥ 24，`NODE_OPTIONS="--require"` 可正常工作
2. **fetch patch 时机**：`--require` 在主模块前执行，globalThis.fetch 在 SDK 初始化前被 patch
3. **SDK fetch 行为**：CC 的 `buildFetch` 使用 `fetchOverride ?? globalThis.fetch`，patch 在 SDK 构造前生效
4. **进程隔离**：inject.js 通过 process.argv 检测 CC 进程，非 CC 进程零开销
5. **Unix Socket 稳定性**：操作系统原生支持，Docker/PostgreSQL 等广泛使用
6. **Socket 连接模型**：每次请求新建连接，`client.end()` 天然界定消息边界，无需 framing 协议

---

## 后续迭代

MVP 验证通过后，后续迭代将添加：
- 配置文件支持（TOML → JSON）
- 图片检测和视觉模型调用
- 并发控制和错误处理
- 日志记录和统计
