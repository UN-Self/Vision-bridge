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
