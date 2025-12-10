# droid-mcp-rs

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust Version](https://img.shields.io/badge/rust-1.70%2B-blue.svg)](https://www.rust-lang.org)
[![MCP Compatible](https://img.shields.io/badge/MCP-Compatible-green.svg)](https://modelcontextprotocol.io)

🚀 高性能 Rust MCP 服务器，封装 [Factory.ai Droid CLI](https://factory.ai) 提供 AI 辅助编程能力

---

## 🎯 完整功能列表

### ✅ 核心能力

1. **MCP 协议支持** - 无缝集成 Claude Code 和其他 MCP 客户端
2. **完整 Droid 集成** - 支持所有 `droid exec` 命令行参数
3. **自主性级别控制** - 细粒度权限管理（DEFAULT/low/medium/high）
4. **会话管理** - 多轮对话与会话恢复（SESSION_ID）
5. **DROID.md 支持** - 项目特定上下文自动注入
6. **自定义模型系统** - 自动读取 `~/.factory/config.json` 配置
7. **安全优先** - 默认只读，需明确指定权限
8. **资源限制** - 自动超时和大小限制
9. **跨平台** - Windows、Linux、macOS 全平台支持

### ✅ 高级功能

1. **推理模式** (`reasoning_effort`) - low/medium/high 三档推理深度
2. **规范模式** (`use_spec` + `spec_model`) - 先规划后执行
3. **权限跳过** (`skip_permissions_unsafe`) - 隔离环境无限制执行（危险！）
4. **输出格式** (`output_format`) - stream-json（默认）或 stream-jsonrpc
5. **文件输入** (`file`) - 从文件读取任务提示
6. **工具控制** (`enabled_tools` / `disabled_tools`) - 精细控制可用工具

---

## 📦 安装

### 从源码构建

```bash
git clone https://github.com/zcg/droid-mcp-rs.git
cd droid-mcp-rs
cargo build --release
```

二进制文件位置：
- **Linux/macOS**: `target/release/droid-mcp-rs`
- **Windows**: `target/release/droid-mcp-rs.exe`

### 配置 Claude Code

```bash
claude mcp add droid-rs -s user --transport stdio -- /path/to/droid-mcp-rs
```

### 配置 Claude Desktop

编辑 `settings.json`：

```json
{
  "mcpServers": {
    "droid": {
      "command": "/path/to/droid-mcp-rs"
    }
  }
}
```

---

## 🎨 完整参数列表

| 参数                      | 类型      | 说明                    | CLI 映射                    | 默认值           |
|-------------------------|---------|-----------------------|---------------------------|---------------|
| `PROMPT`                | string  | 任务提示（与 file 互斥）       | 位置参数                      | -             |
| `file`                  | path    | 从文件读取提示（与 PROMPT 互斥）  | `-f <path>`               | -             |
| `auto`                  | string  | 自主性级别                 | `--auto <level>`          | `high`        |
| `SESSION_ID`            | string  | 会话恢复 ID               | `--session-id <id>`       | -             |
| `cwd`                   | path    | 工作目录                  | `--cwd <path>`            | 当前目录          |
| `model`                 | string  | 模型选择                  | `--model <id>`            | 第一个 GPT 模型    |
| `enabled_tools`         | string  | 启用工具列表（逗号/空格分隔）       | `--enabled-tools <list>`  | -             |
| `disabled_tools`        | string  | 禁用工具列表（逗号/空格分隔）       | `--disabled-tools <list>` | -             |
| `timeout_secs`          | number  | 超时秒数                  | -                         | 600（10分钟）     |
| `reasoning_effort`      | string  | 推理级别（low/medium/high）  | `-r <level>`              | -             |
| `use_spec`              | boolean | 启用规范模式（先规划后执行）        | `--use-spec`              | `false`       |
| `spec_model`            | string  | 规范阶段使用的模型             | `--spec-model <id>`       | -             |
| `skip_permissions_unsafe` | boolean | 跳过所有权限检查（⚠️ 危险！）     | `--skip-permissions-unsafe` | `false`       |
| `output_format`         | string  | 输出格式（stream-json/jsonrpc） | `-o <format>`             | `stream-json` |

**互斥参数：**
- `PROMPT` 和 `file` 不能同时指定
- `skip_permissions_unsafe` 和 `auto` 不能同时指定

---

## 🔐 自主性级别

| 级别        | 说明              | 操作权限                    |
|-----------|-----------------|-------------------------|
| `DEFAULT` | 只读模式（无 `auto` 参数） | cat, git status, ls     |
| `low`     | 文件编辑            | 项目目录内文件创建/编辑            |
| `medium`  | 本地构建            | 包安装、git commit、本地编译     |
| `high`    | 完全权限            | git push、生产部署、脚本执行      |

⚠️ **注意**：默认 `auto=high`，可在配置文件中设置 `allow_high_autonomy=false` 禁用。

---

## 🚀 使用示例

### 场景 1️⃣：基础查询（使用所有默认值）

```typescript
await use_mcp_tool("droid", {
  PROMPT: "分析这个项目的代码质量"
});
// 自动使用：auto=high, 第一个 GPT 模型, stream-json
```

### 场景 2️⃣：深度推理 + 规范模式

```typescript
await use_mcp_tool("droid", {
  PROMPT: "重构认证模块，提升安全性",
  reasoning_effort: "high",  // 深度推理
  use_spec: true,            // 先规划后执行
  spec_model: "gpt-5.1"      // 使用 GPT-5.1 规划
});
```

### 场景 3️⃣：多轮会话

```typescript
// 第一轮：开始任务
const result1 = await use_mcp_tool("droid", {
  PROMPT: "创建用户认证功能"
});

// 第二轮：继续会话
const result2 = await use_mcp_tool("droid", {
  PROMPT: "添加单元测试",
  SESSION_ID: result1.SESSION_ID  // 恢复上下文
});

// 第三轮：继续完善
const result3 = await use_mcp_tool("droid", {
  PROMPT: "优化性能",
  SESSION_ID: result2.SESSION_ID
});
```

### 场景 4️⃣：从文件读取任务

```typescript
// prompt.md 内容：
// # 任务
// 重构整个项目架构...

await use_mcp_tool("droid", {
  file: "./prompt.md",      // 从文件读取
  reasoning_effort: "high"
});
```

### 场景 5️⃣：自定义模型

```typescript
await use_mcp_tool("droid", {
  PROMPT: "生成 API 文档",
  model: "custom:Sonnet-4.5-[88code]-0"  // 使用自定义模型
});
```

### 场景 6️⃣：隔离环境无限制执行

```typescript
// ⚠️ 仅在 Docker 容器等隔离环境使用！
await use_mcp_tool("droid", {
  PROMPT: "系统级配置修改",
  skip_permissions_unsafe: true  // 跳过所有权限检查
});
```

### 场景 7️⃣：精细控制工具

```typescript
await use_mcp_tool("droid", {
  PROMPT: "分析代码但不要修改",
  enabled_tools: "read,grep,find",   // 仅允许读取类工具
  disabled_tools: "write,edit,bash"  // 禁用写入类工具
});
```

---

## ⚙️ 配置系统

### 配置文件：`droid-mcp.config.json`

在工作目录创建（或通过 `DROID_MCP_CONFIG_PATH` 环境变量指定）：

```json
{
  "additional_args": [],
  "timeout_secs": 600,
  "max_timeout_secs": 3600,
  "default_auto": "high",
  "default_model": "claude-opus-4-5-20251101",
  "allow_high_autonomy": true
}
```

**字段说明：**

| 字段                  | 类型       | 说明              | 默认值  |
|---------------------|----------|-----------------|------|
| `additional_args`   | string[] | 每次调用附加的 CLI 参数  | `[]` |
| `timeout_secs`      | number   | 默认超时秒数          | 600  |
| `max_timeout_secs`  | number   | 最大允许超时          | 3600 |
| `default_auto`      | string   | 默认自主性级别         | high |
| `default_model`     | string   | 默认模型（备用）        | -    |
| `allow_high_autonomy` | boolean  | 是否允许 high 级别    | true |

### 环境变量

| 变量                     | 说明             | 默认值                                   |
|------------------------|----------------|---------------------------------------|
| `DROID_BIN`            | droid 二进制路径    | `droid`（Linux/macOS）或 `droid.exe`（Windows） |
| `DROID_MCP_CONFIG_PATH` | 配置文件路径         | `./droid-mcp.config.json`             |

---

## 🎭 自定义模型系统

### 配置文件位置

- **Linux/macOS**: `~/.factory/config.json`
- **Windows**: `C:\Users\<user>\.factory\config.json`

### 配置格式

```json
{
  "custom_models": [
    {
      "model_display_name": "Sonnet 4.5 1M [88code]",
      "model": "claude-sonnet-4-5-20250929-thinking[1m]",
      "base_url": "https://www.88code.org/droid",
      "api_key": "your-api-key",
      "provider": "anthropic"
    },
    {
      "model_display_name": "GPT-5.1-Codex [88code]",
      "model": "gpt-5.1-codex",
      "base_url": "https://www.88code.org/droid/v1",
      "api_key": "your-api-key",
      "provider": "openai"
    }
  ]
}
```

### 使用自定义模型

模型引用格式：`custom:Display-Name-Index`

```typescript
// 第一个模型（索引 0）
await use_mcp_tool("droid", {
  PROMPT: "代码分析",
  model: "custom:Sonnet-4.5-1M-[88code]-0"
});

// 第二个模型（索引 1）
await use_mcp_tool("droid", {
  PROMPT: "生成文档",
  model: "custom:GPT-5.1-Codex-[88code]-1"
});
```

**特性：**
- ✅ 自动列出所有可用模型在 MCP 工具说明中
- ✅ 执行前在日志中显示使用的模型
- ✅ 在结果中返回模型信息（`model_info` 字段）
- ✅ 支持按任务切换不同模型以获得最佳效果
- ✅ 默认优先选择 GPT 模型（智能模型选择）

**日志示例：**
```
droid-mcp-rs: Sonnet 4.5 1M [88code] [anthropic] (claude-sonnet-4-5-20250929-thinking[1m])
```

---

## 📝 DROID.md 系统提示

在工作目录放置 `DROID.md` 文件，定义项目特定的上下文：

```markdown
# 项目上下文

这是一个使用 React 和 Next.js 的 TypeScript 项目。

## 开发指南
- 使用函数式组件和 Hooks
- 遵循现有文件结构
- 为新功能编写测试
- 使用 TypeScript 严格模式
```

**特性：**
- 内容自动作为 `<system_prompt>...</system_prompt>` 注入到每个提示前
- 最大大小：1 MB
- 超过限制自动截断（UTF-8 字符边界安全）

---

## 🔒 安全特性

- ✅ **默认只读** - 无 `auto` 参数时仅允许读取操作
- ✅ **显式权限** - 需明确指定 `auto` 级别才能修改
- ✅ **超时强制** - 防止无限执行（默认 10 分钟）
- ✅ **高权限保护** - `high` 级别需配置文件允许
- ✅ **资源限制** - 自动大小和内存限制
  - Agent 消息：10 MB
  - 所有消息：50 MB
  - DROID.md：1 MB
  - stderr：100 KB

---

## 🛠️ 开发指南

### 构建命令

```bash
cargo build              # 调试构建
cargo build --release    # 优化构建（启用 LTO）
cargo clean              # 清理构建产物
```

### 测试和代码质量

```bash
cargo test               # 运行测试
cargo clippy             # 代码检查
cargo fmt                # 代码格式化
cargo check              # 快速类型检查
```

### 开发要求

- Rust 1.70+
- Droid CLI 已安装并在 PATH 中

---

## 🏗️ 架构概览

### 数据流

```
Claude Code (MCP Client)
    ↓ stdio transport
MCP Server (main.rs)
    ↓ clap CLI parsing
Tool Handler (server.rs::droid)
    ↓ parameter validation + MCP protocol
CLI Wrapper (droid.rs::run)
    ↓ async process spawn + JSON stream parsing
droid exec CLI (subprocess)
```

### 核心文件

| 文件              | 行数    | 职责                      |
|-----------------|-------|-------------------------|
| `src/main.rs`   | ~93   | MCP 服务器入口点，CLI 参数解析    |
| `src/server.rs` | ~369  | MCP 工具定义，参数验证，TOON 编码  |
| `src/droid.rs`  | ~820  | Droid CLI 封装，流解析，配置管理   |
| `src/lib.rs`    | ~3    | 模块声明                    |

### 关键设计模式

1. **延迟配置加载** - 使用 `OnceLock` 缓存静态配置
2. **流式处理** - 异步逐行解析 JSON 流
3. **大小限制** - 多层截断边界（10MB/50MB/1MB）
4. **超时包装** - `tokio::time::timeout` 强制超时
5. **自定义模型系统** - 智能 GPT 优先 + 索引引用

---

## 📁 配置文件位置总结

| 文件                        | 位置                                          | 用途        |
|---------------------------|---------------------------------------------|-----------|
| `droid-mcp.config.json`   | 工作目录或 `$DROID_MCP_CONFIG_PATH`              | 服务器配置     |
| `~/.factory/config.json`  | `~/.factory/` 或 `%USERPROFILE%\.factory\` | 自定义模型配置   |
| `DROID.md`                | `cwd` 指定的工作目录                               | 项目特定上下文   |

---

## 🎉 功能完整性检查

### ✓ 核心功能
- [x] MCP 协议完整支持
- [x] 所有 `droid exec` 参数映射
- [x] 会话管理（SESSION_ID）
- [x] 自定义模型系统
- [x] DROID.md 注入
- [x] 智能模型选择（GPT 优先）

### ✓ 高级功能
- [x] 推理模式（reasoning_effort）
- [x] 规范模式（use_spec）
- [x] 权限跳过（skip_permissions_unsafe）
- [x] 输出格式控制（output_format）
- [x] 文件输入（file）
- [x] 工具控制（enabled_tools/disabled_tools）

### ✓ 质量保证
- [x] 零警告编译
- [x] Clippy 全通过
- [x] 跨平台支持（Windows/Linux/macOS）
- [x] 完整错误处理
- [x] 资源限制和安全控制

---

## 📜 许可证

MIT License - 详见 [LICENSE](LICENSE)

---

## 🔗 相关项目

- [codex-mcp-rs](../codex-mcp-rs/) - Codex CLI MCP wrapper
- [gemini-mcp-rs](../gemini-mcp-rs/) - Gemini CLI MCP wrapper
- [Factory.ai Droid](https://factory.ai) - 官方 Droid CLI 工具

---

## 💬 支持与反馈

- **问题反馈**: [GitHub Issues](https://github.com/zcg/droid-mcp-rs/issues)
- **MCP 文档**: [modelcontextprotocol.io](https://modelcontextprotocol.io)
- **Droid 文档**: [docs.factory.ai](https://docs.factory.ai)

---

## 🚀 快速开始

```bash
# 1. 克隆仓库
git clone https://github.com/zcg/droid-mcp-rs.git
cd droid-mcp-rs

# 2. 构建 Release 版本
cargo build --release

# 3. 配置 Claude Code
claude mcp add droid-rs -s user --transport stdio -- ./target/release/droid-mcp-rs

# 4. 重启 Claude Code MCP 服务
claude mcp restart

# 5. 开始使用！
```

现在可以在 Claude Code 中直接使用 `droid` 工具了！🎉
