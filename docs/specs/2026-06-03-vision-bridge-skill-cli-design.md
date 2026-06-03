# Vision Bridge - Skill + CLI 设计文档

> 日期：2026-06-03
> 状态：设计中

## 问题背景

Vision-bridge 旨在解决 Coding Model 不支持视觉输入的问题。用户在 OpenCode/Claude Code 中引用图片时，由于底层模型限制，图片信息会丢失或导致请求失败。

## 设计目标

将 Vision-bridge 设计为 **Skill + CLI** 架构：
1. **独立产品**：CLI 是一个基于 Rust 的轻量级独立工具产品，负责图片→文字转换。
2. **Skill 集成**：Skill 作为 Agent 的指导层，安装时自动下载 CLI 二进制文件，并教导 Agent 如何使用。
3. **强制机制**：通过 Plugin Hook 自动拦截或高优先级 Skill 指令确保图片被处理。

## 架构总览

```
┌─────────────────────────────────────────────────┐
│  OpenCode / Claude Code                         │
│  ┌─────────────────────────────────────────┐    │
│  │  Plugin Layer (自动拦截)                │    │
│  │  ├─ Hook: chat.message / transform      │    │
│  │  ├─ 检测 Image Blocks                   │    │
│  │  └─ 自动调用 CLI (subprocess)           │    │
│  └─────────────────────────────────────────┘    │
│  ┌─────────────────────────────────────────┐    │
│  │  Skill Layer (指导层/保底)              │    │
│  │  ├─ SKILL.md (Instructions)             │    │
│  │  ├─ bin/vision-bridge (Binary)          │    │
│  │  └─ Agent 强制读取 Skill 并执行 CLI     │    │
│  └─────────────────────────────────────────┘    │
└─────────────────────────────────────────────────┘
```

## 组件详细设计

### 1. CLI 产品 (Rust)

基于 Rust 实现的独立二进制工具，位于 Skill 目录的 `bin/` 下。目前已有 `vbri init/start/stop`，需新增 `describe` 命令。

**新增能力：**
- **`vbri describe`**：接收图片路径/URL，输出文本描述。
- **上下文感知**：接收 `-c` 或 `--context` 参数传入当前对话上下文，辅助视觉模型生成更精准的描述（针对代码/UI）。
- **配置管理**：支持读取 `~/.vision-bridge/config.json` 中的 API Key 和模型设置。

**接口设计：**
```bash
vbri describe \
  --image "./screenshot.png" \
  --context "User is asking about the login button style" \
  --output-format text
```

### 2. Skill 层 (SKILL.md)

**功能：**
1. **自动下载**：安装 Skill 时，根据平台（macOS/Linux/Windows）自动下载对应的 Rust 二进制文件到 `.opencode/skills/vision-bridge/bin/`。
2. **强力指令**：在 SKILL.md 中使用强硬措辞（如 "CRITICAL", "MUST"），指示 Agent 在检测到图片且无法直接理解时，**必须**调用 `vbri describe`。
3. **使用教程**：详细的 CLI 调用示例，特别是如何传递上下文。

### 3. Plugin 层 (Hook 拦截)

**功能：**
利用 OpenCode 的 `experimental.chat.messages.transform` Hook 实现自动化：
1. 扫描消息队列中的 `FilePart` (MIME type 为 image/*)。
2. 提取最近几条消息的文本作为 Context。
3. 调用 `vbri describe` CLI。
4. 将 `FilePart` 替换为包含描述的 `TextPart`。

## 关键技术约束

1. **Rust 二进制分发**：需构建 CI/CD 编译多平台二进制，并提供稳定下载链接。
2. **Hook 兼容性**：`experimental.chat.messages.transform` 标记为实验性，需做好降级方案（回退到 Skill 指令）。
3. **上下文长度**：CLI 接收的上下文需要截断，避免视觉模型 token 溢出。
