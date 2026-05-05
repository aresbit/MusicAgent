# MusicAgent - AI 电台桌面小组件

## 项目概述

MusicAgent 是一个基于 opencc（Codex CLI 逆向工程版本）构建的 AI 电台桌面应用。后端使用 opencc CLI.js 作为 AI 引擎，Electron 作为桌面壳。

## 关键配置

- **opencc CLI 入口**: `packages/opencc/dist/cli.js`
- **MusicAgent 系统提示词**: `packages/opencc/music-agent-prompt.txt`
- **Windows 启动脚本**: `packages/opencc/opencc.bat`（自动加载 MusicAgent 提示词）
- **项目 package.json**: 根目录 `package.json`
- **opencc 包**: `packages/opencc/`
- **gpui-widget**: `gpui-widget/`（桌面 UI 组件）

## 构建与运行

```bash
# 构建 opencc CLI
cd packages/opencc && bun run build

# dev 模式运行 opencc
cd packages/opencc && bun run dev

# 桌面应用构建
bun run build:desktop
```

## 注意事项

- `opencc.bat` 会自动传入 `--dangerously-skip-permissions` 和 `--system-prompt-file`
- 如果用户显式传入了 `--system-prompt` 或 `--system-prompt-file`，则不会覆盖用户的设置
- 所有 feature flag 均返回 false（`src/entrypoints/cli.tsx` 中的 polyfill）
