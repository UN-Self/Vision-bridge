# Vision Bridge — 设计文档 v2

> 日期：2026-05-25
> 状态：设计完成
> 基于：2026-05-22 初稿 + 头脑风暴修订

## 问题背景

国内 Claude Code (CC) 用户通过第三方中转服务（coding plan）接入模型，这些服务大多**不包含视觉模型**，或切换到视觉模型后编程能力大幅下降。用户在 CC 中粘贴截图或引用图片时，图片信息会丢失或导致请求失败。

## 目标

构建一个轻量级工具，在 CC 发送 API 请求前自动拦截图片内容，通过视觉模型解析为文本描述，替换原始图片，使无视觉能力的编码模型也能"看到"图片。

## 设计原则

- **零侵入**：不修改 CC 的源码或配置
- **透明降级**：任何异常静默 fallback，不影响用户正常使用 CC
- **轻量 TUI**：命令行工具，init 即用
- **与中转产品互补**：不处理路由/认证/计费，只做图片→文本转换

---

## 架构

### 职责分离

| 组件 | 职责 | 复杂度 |
|------|------|--------|
| **inject.js** | 截取请求 + 塞入结果 | 轻量 |
| **Rust 服务** | 处理请求 + 返回结果 | 核心 |

### 架构图

```
┌─────────────────────────────────────────────────┐
│  vision-bridge init                              │
│  → 交互式配置视觉模型参数                        │
│  → 生成 ~/.vision-bridge/inject.js               │
│  → 在 shell profile 添加 NODE_OPTIONS 环境变量   │
└─────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────┐
│  vision-bridge start                             │
│  → 启动 Rust 服务                                │
│  → 监听 Unix Domain Socket (~/.vision-bridge/vbri.sock) │
│  → 等待 inject.js 发送请求                       │
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
│  CC 构建带图片的 API 请求                         │
│    ↓                                             │
│  patched fetch 拦截请求:                          │
│    ├── 截取请求体                                │
│    ├── 通过 Unix Socket 发送给 Rust 服务         │
│    ├── 接收处理后的请求体                        │
│    └── originalFetch(url, modifiedOpts) 发出请求  │
│    ↓                                             │
│  用户原来的中转服务 → API                         │
└─────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────┐
│  Rust 服务                                       │
│  → 接收 inject.js 的请求                         │
│  → 检测请求体中所有 image blocks（递归）          │
│  → 提取上下文文本                                │
│  → 调用视觉模型 API 转换图片为文本               │
│  → 替换 image blocks → text blocks               │
│  → 返回修改后的请求体                            │
└─────────────────────────────────────────────────┘
```

---

## 组件

### 1. vbri CLI（Rust）

Rust 编写的命令行工具，单二进制分发，零运行时依赖。

| 命令 | 功能 |
|------|------|
| `vbri init` | 交互式配置视觉模型，生成 inject.js，启动后台服务，设置环境变量 |
| `vbri start` | 启动后台服务（用于 stop 后重新启动） |
| `vbri stop` | 停止后台服务，移除 NODE_OPTIONS 配置 |
| `vbri status` | 显示当前配置、服务状态、运行中的 CC 会话及其处理统计 |
| `vbri config` | 修改已有配置 |
| `vbri logs` | 查看处理日志（`--follow` 实时跟踪） |

核心功能：
- 读取/写入 `~/.vision-bridge/config.toml`
- 从 TOML 自动生成 `~/.vision-bridge/config.json`（Rust 服务运行时读取）
- 根据**嵌入模板**（`include_str!`）生成 `~/.vision-bridge/inject.js`
- 自动检测用户 shell（zsh/bash/fish），在 profile 中**追加** `NODE_OPTIONS`（保留用户已有设置）
- 设置 config.toml 和 config.json 文件权限为 `chmod 600`
- 管理后台服务进程（启动/停止/状态查询）

### 2. inject.js（JavaScript，由 Rust 生成）

注入到 CC 进程的脚本，职责：**截取请求 + 塞入结果**（轻量级）。

核心逻辑：
1. 进程检测：是否为 CC 进程
2. Patch `globalThis.fetch`
3. 拦截请求，通过 Unix Socket 发送给 Rust 服务
4. 接收处理后的请求体，转发给原始目标

```javascript
const net = require('net')
const path = require('path')

// ===== 进程检测 =====
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
      client.end(JSON.stringify(requestBody)) // write + half-close，Rust 端 read_to_end 读完即完整请求
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

    client.setTimeout(5000, () => {
      client.destroy()
      reject(new Error('Socket timeout'))
    })
  })
}

// ===== Patch fetch =====
globalThis.fetch = async (url, opts) => {
  try {
    if (opts && opts.body && typeof opts.body === 'string') {
      const body = JSON.parse(opts.body)

      if (body.messages && Array.isArray(body.messages)) {
        // 发送给 Rust 服务处理
        const processedBody = await sendToRustService(body)
        opts.body = JSON.stringify(processedBody)
      }
    }
  } catch (e) {
    // 静默降级：任何错误都不影响原始请求
    // 日志行为由配置控制，默认静默（不输出到 console）
  }

  return originalFetch(url, opts)
}

// 日志输出受配置控制，默认不输出
// 调试时可通过 config.toml [debug] verbose = true 开启
console.log('[Vision Bridge] inject.js loaded')
```

### 3. Rust 服务

监听 Unix Domain Socket，处理 inject.js 发送的请求。职责：**处理请求 + 返回结果**（核心逻辑）。

#### 通信协议

**连接模型**：每次请求新建 Socket 连接，`client.end()` 天然界定消息边界，无需额外 framing 协议（换行分隔 / 长度前缀）。后续如需持久连接，可添加换行分隔，改动很小。

**请求格式**（inject.js → Rust 服务）：
```json
{
  "messages": [
    {
      "role": "user",
      "content": [
        {
          "type": "image",
          "source": {
            "type": "base64",
            "media_type": "image/png",
            "data": "..."
          }
        }
      ]
    }
  ],
  "model": "claude-3-5-sonnet-20241022",
  "max_tokens": 1024
}
```

**响应格式**（Rust 服务 → inject.js）：
```json
{
  "messages": [
    {
      "role": "user",
      "content": [
        {
          "type": "text",
          "text": "[图片描述] 这是一个代码截图，显示了..."
        }
      ]
    }
  ],
  "model": "claude-3-5-sonnet-20241022",
  "max_tokens": 1024
}
```

#### 核心处理流程

1. 接收 inject.js 的请求
2. 检测请求体中所有 image blocks（递归）
3. 提取上下文文本
4. 调用视觉模型 API 转换图片为文本
5. 替换 image blocks → text blocks
6. 返回修改后的请求体

#### 进程检测

通过 `process.argv` 判断当前进程是否为 Claude Code：

```javascript
function isClaudeCodeProcess() {
  const argv = process.argv.join(' ')
  return (
    argv.includes('@anthropic-ai/claude-code') ||
    argv.includes('claude-code') ||
    argv.includes('claude_code')
  )
}
```

非 CC 进程（如 `npm`、`node script.js` 等）直接跳过 patch，零开销。

#### 配置加载

inject.js 运行时从 `~/.vision-bridge/config.json` 读取配置（使用 `JSON.parse`，零依赖）。

配置生成流程：
1. 用户通过 `vision-bridge init` 填写配置，保存为 `~/.vision-bridge/config.toml`（用户友好的格式）
2. Rust CLI 自动从 TOML 转换生成 `~/.vision-bridge/config.json`（inject.js 运行时读取）
3. 两个文件都设置 `chmod 600`

#### 图片检测（递归）

递归遍历请求体中 `messages` 数组的所有 content blocks，包括：
- 顶层 content 中的 image blocks
- `tool_result` block 内部嵌套的 image blocks
- 任意深度的嵌套结构

匹配结构：
```json
{
  "type": "image",
  "source": {
    "type": "base64",
    "media_type": "image/png",
    "data": "..."
  }
}
```

支持格式：png, jpg, jpeg, gif, webp

#### 上下文提取

为视觉模型提供上下文，提升描述质量：
- 当前 user message 中的文本部分
- 最近 3 条消息的文本内容（从 messages 数组末尾向前取）
- 拼接为上下文字符串传给视觉模型

> **待评估**：上下文的裁剪策略（按字符数截断 vs token 估算）以及 AI 功能的深度整合，后续迭代中评估。

#### 视觉模型调用

使用 OpenAI 兼容格式（`/v1/chat/completions`）：

```json
{
  "model": "<用户配置的模型>",
  "messages": [
    {
      "role": "system",
      "content": "<用户配置的 prompt 模板>"
    },
    {
      "role": "user",
      "content": [
        { "type": "text", "text": "结合以下上下文描述图片：\n<上下文文本>" },
        { "type": "image_url", "image_url": { "url": "data:image/png;base64,..." } }
      ]
    }
  ]
}
```

调用使用 `originalFetch`（未被 patch 的原始 fetch），避免递归。

#### 请求体处理

Anthropic SDK 将请求体序列化为 JSON 字符串后传给 fetch（`Content-Type: application/json`）。inject.js 的处理流程：

1. 检查 `opts.body` 是否为 string 类型
2. `JSON.parse(opts.body)` 解析为对象
3. 修改 image blocks
4. `JSON.stringify(body)` 重新序列化
5. 赋回 `opts.body`

非 string 类型的 body（如 ReadableStream、FormData）直接跳过，不处理。

#### 配置读取时机

inject.js 在 CC 进程启动时读取一次 `~/.vision-bridge/config.json`，之后不再重新读取。用户修改配置后需重启 CC 生效。

#### 并发处理

- **默认**：无限制并发，每个请求独立处理
- **可配置**：通过 config.toml 设置最大并发数（信号量限流）
- **事件队列**：超出并发限制的请求排队等待
- **阻塞语义**：CC 的 API 请求会阻塞等待其所有图片处理完成

#### 图片替换

将每个 image block 替换为 text block：
```json
{
  "type": "text",
  "text": "[图片描述] <视觉模型返回的描述文本>"
}
```

保持 content 数组的顺序不变，不影响其他 text blocks。

#### 部分失败处理

- 成功的图片 → 替换为文本描述
- 失败的图片 → **保留原始 image block 不替换**，让 CC 自行处理（透明降级）

#### 异常处理

所有逻辑包裹在 try/catch 中：
- 视觉模型调用失败 → 保留原始图片不替换
- 网络超时（默认 30 秒）→ 降级
- JSON 解析失败 → 降级
- **任何情况下都不阻断原始请求**

#### 日志记录

**inject.js 侧**：默认静默，不向 console 输出任何内容。通过 config.toml `[debug] verbose = true` 可开启调试日志，输出到 `~/.vision-bridge/debug.log`。

**Rust 服务侧**：所有处理日志写入 `~/.vision-bridge/debug.log`，包括：
- 每次图片检测的结果（发现几张图片）
- 视觉模型调用的耗时和结果（成功/失败）
- 错误信息（如有）

日志格式：`[ISO 时间] [级别] 消息`

用户通过 `vbri logs` 查看日志，`vbri logs --follow` 实时跟踪。

#### 运行时统计

inject.js 将每个 CC 会话的处理统计写入 `~/.vision-bridge/stats/{pid}.json`：

```json
{
  "pid": 12345,
  "started_at": "2026-05-25T10:00:00Z",
  "images_processed": 5,
  "images_failed": 1,
  "total_time_ms": 12500
}
```

`vision-bridge status` 读取所有 stats 文件，显示所有运行中的 CC 会话。自动清理已退出进程的 stats 文件（通过 PID 存活检测）。

#### 多会话隔离

每个 CC 进程独立运行 inject.js 实例，并发限制为**每进程独立**。用户开 3 个 CC 终端，每个终端各自有独立的并发限制。

---

## 配置文件

### 用户配置（TOML）

位置：`~/.vision-bridge/config.toml`
权限：`chmod 600`（仅所有者可读写）

```toml
[vision]
# 视觉模型 API（OpenAI 兼容格式）
api_base = "https://api.example.com/v1"
api_key = "sk-xxx"
model = "gpt-4o"
timeout_seconds = 30

[concurrency]
# 最大并发视觉模型调用数（0 = 无限制）
max_concurrent = 0

[prompt]
# 视觉模型的系统提示词
system = "你是一个专业的图片描述助手，专注于辅助编程场景。描述图片中与代码、UI、错误信息相关的内容，忽略无关细节。"
# 上下文窗口：取最近 N 条消息作为上下文
max_context_messages = 3

[debug]
# 调试模式：开启后 inject.js 输出日志到 debug.log
verbose = false
```

### 运行时配置（JSON，由 CLI 自动生成）

位置：`~/.vision-bridge/config.json`
权限：`chmod 600`

Rust CLI 在 `init`/`config` 时自动从 TOML 生成，inject.js 运行时读取此文件。

### Shell Profile 修改

`vision-bridge init` 自动检测用户 shell 并修改对应的 profile 文件：

| Shell | Profile 文件 |
|-------|-------------|
| zsh | `~/.zshrc` |
| bash | `~/.bashrc`（优先）或 `~/.bash_profile` |
| fish | `~/.config/fish/config.fish` |

追加方式（保留用户已有 NODE_OPTIONS）：
```bash
# zsh/bash
export NODE_OPTIONS="--require ~/.vision-bridge/inject.js $NODE_OPTIONS"

# fish
set -gx NODE_OPTIONS "--require ~/.vision-bridge/inject.js $NODE_OPTIONS"
```

`vbri stop` 自动移除这行配置。

### 平台支持

- **macOS**：完全支持
- **Linux**：完全支持
- **Windows**：不支持（后续迭代考虑）

---

## 用户使用流程

```bash
# 1. 安装（包管理器）
brew install vision-bridge
# 或
cargo install vision-bridge

# 2. 初始化（首次）
vbri init
# ? 视觉模型 API Base URL: https://api.example.com/v1
# ? API Key: sk-xxx
# ? Model: gpt-4o
# ✓ 配置已保存到 ~/.vision-bridge/config.toml (chmod 600)
# ✓ config.json 已生成
# ✓ inject.js 已生成
# ✓ 后台服务已启动
# ✓ NODE_OPTIONS 已添加到 ~/.zshrc
# ✓ 请重启终端或 source ~/.zshrc

# 3. 直接使用 CC
source ~/.zshrc  # 或重启终端
claude
# 自动注入，无需额外操作

# 4. 查看状态
vbri status
# ✓ 配置: gpt-4o @ https://api.example.com/v1
# ✓ 服务状态: 运行中
# ✓ 运行中的 CC 会话: 2
#   - PID 12345: 已处理 5 张图片, 1 张失败, 运行 2 小时
#   - PID 12346: 已处理 12 张图片, 0 张失败, 运行 30 分钟

# 5. 查看日志
vbri logs
vbri logs --follow  # 实时跟踪

# 6. 停止服务
vbri stop
# ✓ 后台服务已停止
# ✓ NODE_OPTIONS 已移除
# ✓ 二进制文件请通过包管理器卸载

# 7. 重新启动
vbri start
# ✓ 后台服务已启动
# ✓ NODE_OPTIONS 已配置
```

---

## 分发

| 渠道 | 包内容 |
|------|--------|
| Homebrew | Rust 编译的二进制文件（macOS/Linux） |
| crates.io | Rust 源码，cargo install 编译 |
| npm | 预编译多平台二进制（darwin-x64, darwin-arm64, linux-x64, linux-arm64）+ inject.js 模板，postinstall 脚本自动选择对应平台 |

---

## 关键技术约束

1. **NODE_OPTIONS 兼容性**：CC 使用 Node.js ≥ 24，`NODE_OPTIONS="--require"` 可正常工作
2. **fetch patch 时机**：`--require` 在主模块前执行，globalThis.fetch 在 SDK 初始化前被 patch
3. **SDK fetch 行为**：CC 的 `buildFetch` 使用 `fetchOverride ?? globalThis.fetch`，patch 在 SDK 构造前生效；SDK 将请求体序列化为 JSON string 传给 fetch
4. **流式请求**：CC 使用 SSE streaming，但请求体是一次性发送的 JSON，patch 在请求发出前执行
5. **并发请求**：CC 可能同时发送多个 API 请求，Rust 服务通过异步处理支持并发
6. **API 格式差异**：Anthropic API 的 image block 格式与 OpenAI 格式不同，Rust 服务内部处理转换
7. **进程隔离**：inject.js 通过 process.argv 检测 CC 进程，非 CC 进程零开销
8. **配置读取**：Rust 服务启动时读取配置，inject.js 不需要读取配置
9. **平台限制**：仅支持 macOS 和 Linux
10. **Unix Socket 稳定性**：操作系统原生支持，Docker/PostgreSQL 等广泛使用，比 HTTP 轻量

---

## 后续迭代

- **上下文裁剪优化**：评估按字符数截断 vs token 估算，优化视觉模型输入
- **图片缓存**：对相同图片（base64 hash）缓存描述结果，避免重复调用视觉模型
- **自定义 prompt 模板**：允许用户在配置中覆盖默认的视觉模型提示词
- **多视觉模型支持**：除 OpenAI 兼容 API 外，支持 Gemini、本地 Ollama 等
- **Codex 支持**：验证 NODE_OPTIONS 注入对 Codex 的兼容性
- **Windows 支持**：适配 Windows 的 shell profile 和路径格式
a