# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## TOP RULES
HAVE TO USE SUPERPOWER
THINK ABOUT REUSE
ASKING BEFORE DOING
superpower's docs should directly put in docs/, not /superpower/plans or /specs.
WHEN YOU WRITE COMMIT NOTES, DONT WRITE CO-AUTHOR-BY

## Project Overview

Vision Bridge is a lightweight CLI tool that enables Claude Code users on third-party relay services (without vision model support) to "see" images. It monkey-patches `globalThis.fetch` via `NODE_OPTIONS="--require"` to intercept image blocks in API requests, converts them to text descriptions using a vision model API, and replaces the original images before the request reaches the relay.

## Architecture

The project has two main components:

1. **vision-bridge CLI (Rust)** — Single-binary tool for configuration and setup. Generates inject.js from an embedded template, manages `~/.vision-bridge/` config files, and modifies shell profiles to set `NODE_OPTIONS`.

2. **inject.js (JavaScript)** — Runtime injection script loaded into Claude Code's Node.js process via `NODE_OPTIONS="--require"`. Patches `globalThis.fetch` to intercept and transform image blocks.

Key data flow: `CC sends API request → patched fetch detects image blocks → calls vision model API via originalFetch → replaces images with text descriptions → forwards modified request to relay`

## Agent/ Directory

`Agent/` contains a **restored Claude Code source tree** reconstructed from npm package source maps. This is a reference resource for understanding CC internals — it is NOT part of the Vision Bridge build. Key files for understanding CC's API request flow:

- `Agent/src/services/api/client.ts` — `buildFetch()` captures `globalThis.fetch` at construction time (`fetchOverride ?? globalThis.fetch`), which is why our `--require` injection timing works
- `Agent/src/services/api/claude.ts` — API call construction, image block handling, `stripExcessMediaItems()`

## Design Docs

- `docs/2026-05-22-vision-bridge-design.md` — Original design draft
- `docs/2026-05-25-vision-bridge-design-v2.md` — Refined design after brainstorming (the authoritative spec)

## Development

This is an early-stage project. The Rust CLI has not been scaffolded yet.

When implementing:
- Rust CLI uses `include_str!` to embed the inject.js template
- inject.js reads config from `~/.vision-bridge/config.json` at runtime (JSON, not TOML — the CLI converts TOML→JSON on init)
- Process detection via `process.argv` matching against `claude-code` patterns
- Images are detected by recursively traversing `messages[].content[]` including nested `tool_result` blocks
- Body interception only handles JSON string type (the Anthropic SDK always serializes to JSON string)
- Failed image processing preserves the original image block (transparent degradation)

## Platform Support

macOS and Linux only. No Windows support planned for v1.
