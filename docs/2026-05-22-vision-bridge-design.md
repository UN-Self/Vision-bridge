# Vision Bridge - 设计文档

> 日期：2026-05-22
> 状态：草案

## 问题背景

国内 Claude Code (CC) 和 Codex 用户通常通过第三方中转服务（coding plan）接入模型，这些服务大多**不包含视觉模型**，或者切换到视觉模型后编程能力大幅下降。用户在 CC 对话中粘贴截图或引用图片时，图片信息会丢失或导致请求失败。

## 目标

构建一个轻量级工具，在 CC/Codex 发送 API 请求前自动拦截图片内容，通过视觉模型解析为文本描述，替换原始图片，使无视觉能力的编码模型也能"看到"图片。

## 设计原则

- **零侵入**：不修改 CC/Codex 的源码或配置（不改 `ANTHROPIC_BASE_URL` 等）
- **透明降级**：任何异常静默 fallback，不影响用户正常使用 CC
- **轻量 TUI**：命令行工具，init 即用
- **与中转产品互补**：不处理路由/认证/计费，只做图片→文本转换

## 架构

```
┌─────────────────────────────────────────────────┐
│  vision-bridge init                              │
│  → 交互式配置视觉模型参数                        │
│  → 生成 ~/.vision-bridge/inject.js               │
│  → 在 shell profile 添加 NODE_OPTIONS 环境变量   │
└─────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────┐
│  CC 启动 (Node.js 进程)                          │
│                                                  │
│  NODE_OPTIONS 加载 inject.js                     │
│    ↓                                             │
│  inject.js monkey-patch globalThis.fetch         │
│    ↓                                             │
│  CC 构建带图片的 API 请求                         │
│    ↓                                             │
│  patched fetch 拦截请求:                          │
│    ├── 检测 messages 中的 image blocks            │
│    ├── 提取上下文文本                             │
│    ├── 用 originalFetch 调用视觉模型 API          │
│    ├── 替换 image blocks → text blocks            │
│    └── originalFetch(url, modifiedOpts) 发出请求  │
│    ↓                                             │
│  用户原来的中转服务 → API                         │
└─────────────────────────────────────────────────┘
```

## 组件

### 1. vision-bridge CLI（Rust）

Rust 编写的命令行工具，职责：

| 命令 | 功能 |
|------|------|
| `vision-bridge init` | 交互式配置视觉模型，生成 inject.js，设置环境变量 |
| `vision-bridge status` | 显示当前配置和处理统计 |
| `vision-bridge uninstall` | 清理环境变量和配置文件 |
| `vision-bridge config` | 修改已有配置 |

核心功能：
- 读取/写入 `~/.vision-bridge/config.toml`
- 根据配置生成 `~/.vision-bridge/inject.js`（将 API 地址、密钥、模型名、prompt 模板嵌入脚本）
- 自动检测用户 shell（zsh/bash/fish），在 profile 中添加 `NODE_OPTIONS`
- 可选 TUI 状态面板，通过监控 CC transcript 文件显示图片处理日志

### 2. inject.js（JavaScript，由 Rust 生成）

注入到 CC 进程的脚本，核心逻辑：

```javascript
// 伪代码
const originalFetch = globalThis.fetch

globalThis.fetch = async (url, opts) => {
  try {
    const body = parseRequestBody(opts)

    if (isAnthropicMessagesRequest(url, body)) {
      const images = extractImageBlocks(body.messages)

      if (images.length > 0) {
        const context = extractTextContext(body.messages)
        const descriptions = await callVisionAPI(images, context)
        replaceImagesWithText(body.messages, descriptions)
        rebuildRequestBody(opts, body)
      }
    }
  } catch (e) {
    // 静默降级：任何错误都不影响原始请求
  }

  return originalFetch(url, opts)
}
```

#### 图片检测

遍历请求体中 `messages` 数组的所有 content blocks，匹配结构：

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

#### 图片替换

将每个 image block 替换为 text block：

```json
{
  "type": "text",
  "text": "[图片描述] <视觉模型返回的描述文本>"
}
```

保持 content 数组的顺序不变，不影响其他 text blocks。

#### 异常处理

所有逻辑包裹在 try/catch 中：
- 视觉模型调用失败 → 保留原始图片不替换
- 网络超时（默认 30 秒）→ 降级
- JSON 解析失败 → 降级
- **任何情况下都不阻断原始请求**

## 配置文件

位置：`~/.vision-bridge/config.toml`

```toml
[vision]
# 视觉模型 API（OpenAI 兼容格式）
api_base = "https://api.example.com/v1"
api_key = "sk-xxx"
model = "gpt-4o"
timeout_seconds = 30

[prompt]
# 视觉模型的系统提示词
system = "你是一个专业的图片描述助手，专注于辅助编程场景。描述图片中与代码、UI、错误信息相关的内容，忽略无关细节。"
# 上下文窗口：取最近 N 条消息作为上下文
max_context_messages = 3
```

## 用户使用流程

```bash
# 1. 安装
brew install vision-bridge
# 或
npm install -g vision-bridge

# 2. 初始化（交互式）
vision-bridge init
# ? 视觉模型 API Base URL: https://api.example.com/v1
# ? API Key: sk-xxx
# ? Model: gpt-4o
# ✓ 配置已保存到 ~/.vision-bridge/config.toml
# ✓ inject.js 已生成
# ✓ NODE_OPTIONS 已添加到 ~/.zshrc
# ✓ 请运行 source ~/.zshrc 或重启终端

# 3. 正常使用 CC（无需任何额外操作）
claude

# 4. 查看状态
vision-bridge status

# 5. 卸载
vision-bridge uninstall
```

## 分发

| 渠道 | 包内容 |
|------|--------|
| Homebrew | Rust 编译的二进制文件 |
| npm | 包含预编译二进制 + JS 脚本 |

## 后续迭代

- **Codex 支持**：Codex 同样基于 Node.js，`NODE_OPTIONS` 注入方式理论上通用，后续验证并适配
- **图片缓存**：对相同图片（base64 hash）缓存描述结果，避免重复调用视觉模型
- **自定义 prompt 模板**：允许用户在配置中覆盖默认的视觉模型提示词
- **处理统计**：累计处理图片数量、token 消耗、延迟等指标
- **多视觉模型支持**：除 OpenAI 兼容 API 外，支持 Gemini、本地 Ollama 等

## 关键技术约束

1. **NODE_OPTIONS 兼容性**：CC 使用 Node.js 运行，`NODE_OPTIONS="--require"` 可正常工作；Codex 需验证
2. **fetch patch 时机**：inject.js 必须在 CC 初始化 SDK 之前执行，`--require` 满足此要求
3. **流式请求**：CC 使用 SSE streaming（`stream: true`），但请求体仍是一次性发送的 JSON，patch 在请求发出前执行，不受流式响应影响
4. **并发请求**：CC 可能同时发送多个 API 请求（如 tool_use 后连续调用），inject.js 需支持并发处理
5. **API 格式差异**：Anthropic API 的 image block 格式（`source.type: "base64"`）与 OpenAI 格式（`image_url`）不同，需要正确转换
