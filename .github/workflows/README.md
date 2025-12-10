# GitHub Actions Workflows

## Release Workflow

手动触发的跨平台二进制发布工作流。

### 使用方法

1. **触发 Release Workflow**
   - 进入 GitHub 仓库的 "Actions" 页面
   - 选择 "Release" workflow
   - 点击 "Run workflow" 按钮
   - 填写参数：
     - `version`: 版本号（不带 `v` 前缀，例如 `0.1.1`）
     - `prerelease`: 是否标记为预发布版本（可选，默认 false）

2. **构建产物**

   Workflow 会为以下平台构建二进制文件：
   - Linux x86_64 (GNU libc)
   - Linux x86_64 (musl - 静态链接)
   - Windows x86_64
   - macOS x86_64 (Intel)
   - macOS ARM64 (Apple Silicon)

3. **Release 内容**

   每个平台的 release 包含：
   - 二进制文件 (`droid-mcp-rs` 或 `droid-mcp-rs.exe`)
   - LICENSE 文件
   - README.md 文档
   - SHA256 校验和文件 (`.sha256`)

4. **Release 管理**

   - Release 默认创建为 **Draft（草稿）** 状态
   - 自动生成 Release Notes（基于 commits）
   - 所有构建完成后，手动 publish release
   - 支持重新运行（`--clobber` 覆盖已有文件）

### 工作流特性

- ✅ **手动触发** - 完全控制发布时机
- ✅ **并行构建** - 5 个平台同时构建（fail-fast: false）
- ✅ **原生编译** - 每个平台使用对应的 runner（无交叉编译）
- ✅ **Cargo 缓存** - 使用 `rust-cache` 加速构建
- ✅ **SHA256 校验** - 每个文件自动生成校验和
- ✅ **幂等性** - 支持安全重试和覆盖上传
- ✅ **并发控制** - 防止同一版本的多次构建冲突

### 校验下载的文件

**Linux/macOS:**
```bash
sha256sum -c droid-mcp-rs-x86_64-unknown-linux-gnu.tar.gz.sha256
# 或
shasum -a 256 -c droid-mcp-rs-x86_64-apple-darwin.tar.gz.sha256
```

**Windows (PowerShell):**
```powershell
$expected = (Get-Content droid-mcp-rs-x86_64-pc-windows-msvc.zip.sha256).Split()[0]
$actual = (Get-FileHash droid-mcp-rs-x86_64-pc-windows-msvc.zip -Algorithm SHA256).Hash.ToLower()
$expected -eq $actual
```

### 故障排查

**权限错误 (403)**
- Workflow 已配置 `permissions: contents: write`
- 确认 repository 的 Actions 权限设置正确

**构建失败**
- 查看具体平台的构建日志
- `fail-fast: false` 确保其他平台继续构建
- 可以单独重新运行失败的 job

**版本冲突**
- Workflow 会检查 release 是否已存在
- 已存在的 release 会复用（不重复创建）
- 使用 `--clobber` 覆盖已上传的文件

### 本地测试

测试特定平台的构建：

```bash
# Linux GNU
cargo build --release --locked --target x86_64-unknown-linux-gnu

# Linux musl (需要安装 musl-tools)
cargo build --release --locked --target x86_64-unknown-linux-musl

# Windows (在 Windows 上)
cargo build --release --locked --target x86_64-pc-windows-msvc

# macOS
cargo build --release --locked --target x86_64-apple-darwin
cargo build --release --locked --target aarch64-apple-darwin
```

### Release Checklist

发布新版本前的检查清单：

- [ ] 更新 `Cargo.toml` 中的版本号
- [ ] 更新 `CHANGELOG.md`（如果有）
- [ ] 确保所有测试通过 (`cargo test`)
- [ ] 确保代码格式化 (`cargo fmt --check`)
- [ ] 确保 clippy 无警告 (`cargo clippy`)
- [ ] 提交所有更改并 push 到 GitHub
- [ ] 触发 Release workflow
- [ ] 等待所有构建完成
- [ ] 测试下载的二进制文件
- [ ] 编辑 release notes（如需要）
- [ ] Publish release

---

更多信息请参考主项目 [README.md](../../README.md)
