# [Vision-bridge Skill + CLI] Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Convert Vision-bridge into a high-priority Skill + CLI architecture, where the CLI is a standalone Rust product and the Skill acts as the integration and instruction layer.

**Architecture:** The system consists of two main parts:
1. **CLI (Rust Product):** A standalone binary (`vbri`) that handles image-to-text conversion with context awareness.
2. **Skill (Integration Layer):** A OpenCode Skill that automatically downloads the CLI binary during installation and provides high-priority instructions to the Agent, forcing it to use the CLI for image processing.

**Tech Stack:** Rust (CLI), TypeScript/Node.js (Skill wrapper/installer), OpenCode Skill format.

---

### Task 1: Skill Directory Structure & Binary Manager

**Files:**
- Create: `.opencode/skills/vision-bridge/SKILL.md`
- Create: `.opencode/skills/vision-bridge/install.ts` (Installation logic)
- Create: `.opencode/skills/vision-bridge/bin/` (Binary directory)

- [ ] **Step 1: Create Skill Directory Structure**

```bash
mkdir -p .opencode/skills/vision-bridge/bin
```

- [ ] **Step 2: Create `install.ts` (Binary Downloader)**

```typescript
import { exec } from 'child_process';
import path from 'path');
import fs from 'fs');
import os from 'os');

const VERSION = '0.1.0';
const BASE_URL = `https://github.com/UN-Self/Vision-bridge/releases/download/v${VERSION}`;
const INSTALL_DIR = path.join(__dirname, 'bin');

function getPlatform() {
  switch (os.platform()) {
    case 'darwin': return 'apple-darwin';
    case 'linux': return 'unknown-linux-gnu';
    case 'win32': return 'pc-windows-msc';
    default: throw new Error(`Unsupported platform: ${os.platform()}`);
  }
}

async function install() {
  const platform = getPlatform();
  const ext = os.platform() === 'win32' ? '.exe' : '';
  const binaryName = `vision-bridge-${platform}${ext}`;
  const downloadUrl = `${BASE_URL}/${binaryName}`;
  const targetPath = path.join(INSTALL_DIR, `vision-bridge${ext}`);

  console.log(`Downloading Vision-bridge CLI from ${downloadUrl}...`);
  
  // Use curl or wget to download
  exec(`curl -L -o "${targetPath}" "${downloadUrl}"`, (error) => {
    if (error) {
      console.error('Failed to download CLI:', error);
      process.exit(1);
    }
    fs.chmodSync(targetPath, '755');
    console.log('Vision-bridge CLI installed successfully at:', targetPath);
  });
}

install();
```

- [ ] **Step 3: Verify Installation Script**

Run: `npx ts-node .opencode/skills/vision-bridge/install.ts`
Expected: Binary downloaded to `bin/` directory.

---

### Task 2: Implement SKILL.md Instructions

**Files:**
- Create: `.opencode/skills/vision-bridge/SKILL.md`

- [ ] **Step 1: Create SKILL.md with High-Priority Prompt**

```markdown
---
name: vision-bridge
description: CRITICAL: Use when processing ANY image input. MUST use vision-bridge CLI to convert images to text descriptions.
---

# Vision Bridge - Image Processing Skill

**CRITICAL INSTRUCTION:** 
When a user provides an image or references a screenshot, you **MUST** use the `vision-bridge` CLI tool to process the image before attempting to analyze it. 
Do NOT assume you can "see" the image directly if your model lacks vision capabilities.

## Usage

To process an image, run the following command:

```bash
vision-bridge describe --image "<path_to_image>" --context "<recent_conversation_context>"
```

- `--image`: Path to the local image file.
- `--context`: (Optional but Recommended) Recent conversation text to help the vision model understand the goal.

## Example

User: "Here is a screenshot of the login page error. Fix it."

1. Save the screenshot to a temporary file (if provided as base64/URL).
2. Run:
   ```bash
   vision-bridge describe --image "./tmp/login_error.png" --context "User reports a login error on the main page."
   ```
3. Use the text output from the command as the source of truth for the image content.
4. Proceed with code generation based on the text description.

## Installation

If the `vision-bridge` command is not found, run the installation script:
```bash
npx ts-node <skill_path>/install.ts
```
```

- [ ] **Step 2: Verify Skill Loading**

Use OpenCode's `skill` tool to list and verify `vision-bridge` is visible.

---

### Task 3: Implement Plugin Hook (Auto-Intercept)

**Files:**
- Create: `.opencode/skills/vision-bridge/plugin.ts` (OpenCode Plugin)

- [ ] **Step 1: Create `plugin.ts` with Hook Logic**

```typescript
import type { Plugin } from "@opencode-ai/plugin";
import { execSync } from 'child_process');
import path from 'path');

export default (async ({ client, project, directory, $ }) => {
  return {
    "experimental.chat.messages.transform": async (input, output) => {
      const messages = output.messages;
      let hasImages = false;
      
      // 1. Scan for images
      for (const msg of messages) {
        for (const part of msg.parts) {
          if (part.type === "file" && part.mime.startsWith("image/")) {
            hasImages = true;
            break;
          }
        }
        if (hasImages) break;
      }

      if (hasImages) {
        // 2. Extract context (last 3 text messages)
        const context = messages
          .filter(m => m.info.role === "user")
          .slice(-3)
          .map(m => m.parts.filter(p => p.type === "text").map(p => p.text).join("\n"))
          .join("\n");

        // 3. Process images
        for (const msg of messages) {
          for (const part of msg.parts) {
            if (part.type === "file" && part.mime.startsWith("image/")) {
              try {
                // Call CLI
                const description = execSync(
                  `vision-bridge describe --image "${part.url}" --context "${context}"`,
                  { encoding: 'utf-8' }
                );
                
                // Replace image with text
                part.type = "text";
                part.text = `[Image Description]: ${description}`;
                // Remove file-specific properties
                delete part.url;
                delete part.mime;
              } catch (e) {
                console.error("Failed to process image:", e);
              }
            }
          }
        }
      }
    }
  }
}) satisfies Plugin;
```

- [ ] **Step 2: Verify Plugin Registration**

Ensure the plugin is loaded by checking OpenCode logs.

---

### Task 4: Final Integration & Documentation

**Files:**
- Modify: `.opencode/skills/vision-bridge/SKILL.md`

- [ ] **Step 1: Update SKILL.md with Final Usage**

Ensure the `SKILL.md` clearly states:
1. The skill is **mandatory** for image processing.
2. The context parameter is **critical** for code-related image analysis.
3. The installation is automatic via the skill's `install.ts`.

- [ ] **Step 2: Commit Changes**

```bash
git add .opencode/skills/vision-bridge/
git commit -m "feat: add vision-bridge skill with CLI wrapper and installation logic"
```

---
