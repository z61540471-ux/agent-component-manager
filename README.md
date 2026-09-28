# Agent Component Manager

Agent Component Manager 是一个面向 Windows 的本地 Agent 组件管理工具，用于统一查看和管理 Codex、Claude Code 中的 Skill、MCP 注册和 Plugin。

项目采用 Tauri、React、TypeScript 和 Rust 构建。当前仍处于开发阶段，建议先在临时配置目录中验证功能，再操作真实的 Agent 配置。

## 功能概览

- 扫描本机 Skill、MCP 注册和 Plugin，并展示名称、位置、来源、作用域和诊断结果。
- 支持 Codex、Claude Code 以及共享 Skill 目录。
- 支持用户级和项目级配置，区分直接注册、继承绑定、本地绑定和插件所有权。
- 从 GitHub、官方 MCP Registry 和 Claude 插件市场发现组件。
- 先生成变更预览，再确认应用；写入前会检查状态并创建本地备份。
- 提供操作历史、恢复记录和脱敏后的清单导出。

### 能力边界

- 扫描只读取本地清单，不会启动 MCP 服务或执行第三方脚本。
- MCP 注册不代表服务器已经安装或正在运行。
- 市场中的 Skill、MCP 或 Plugin 不一定支持当前 Agent 的自动安装。
- Claude Plugin 操作目前只提供经过检查的手动 CLI 指引，不会修改插件缓存，也不会自动执行命令。
- Codex Plugin 配置目前只读，安装和删除仍需通过 Codex 自身管理。
- 预览会隐藏凭据、完整 URL、参数和环境变量；原始配置只保存在本机备份中。

## 快速开始

### 环境要求

在 Windows 上安装以下工具：

- Node.js 和 npm
- Rust stable MSVC 工具链
- Visual Studio C++ Build Tools
- WebView2 Runtime

### 安装依赖并启动桌面应用

在 PowerShell 中执行：

```powershell
cd D:\develop\package\agent-component-manager
npm ci
npm run tauri dev
```

首次启动可能需要编译 Rust 依赖，请耐心等待。启动成功后会打开 Tauri 桌面窗口。

`npm run dev` 只会启动 Vite 前端预览，不能扫描本机文件，也不能执行真实的 MCP 或 Skill 配置操作。需要使用 `npm run tauri dev` 才能启用原生能力。

## 第一次使用

1. 启动桌面应用后进入“设置”。
2. 填写用户主目录，并按需填写 Codex 目录、Claude Code 目录和项目目录。
3. 保存设置，回到“本机组件”并点击“扫描本机”。
4. 点击组件查看详情。组件的发现状态不代表 Agent 已在当前会话中加载。
5. 对支持修改的组件先点击预览，再审阅文件变化并确认应用。

目录解析规则如下：

- Codex 目录优先使用设置中的路径，其次使用 `CODEX_HOME`，最后使用用户目录下的 `.codex`。
- Claude Code 组件目录使用设置中的路径，未填写时使用用户目录下的 `.claude`。
- Claude 用户级 MCP 配置仍位于用户目录下的 `.claude.json`。
- 项目级配置只对设置中明确选择的项目生效。

## 常用开发命令

```powershell
# 安装或更新前端依赖
npm ci

# 启动 Tauri 开发模式
npm run tauri dev

# 只启动前端预览
npm run dev

# 静态检查、构建和前端测试
npm run lint
npm run build
npm test

# Rust 测试
cargo test --manifest-path src-tauri/Cargo.toml --locked

# 构建 Windows 应用和 NSIS 安装包
npm run tauri build
```

Windows 构建产物会输出到 Tauri 的目标目录。当前构建是未签名的开发版本，不代表正式发布包。

## 数据和安全

- 清单扫描是只读操作，不会启动 MCP 进程。
- 配置修改必须经过预览、确认、状态复核、备份和结果校验。
- 应用会拒绝通过 Windows junction 或其他不安全链接路径写入。
- 备份可能包含凭据，只保存在本机应用数据目录，不会进入 IPC 预览、导出文件或 Git。
- 导出的 JSON 只包含清单元数据，不包含 MCP 原始配置、参数、环境变量或备份字节。
- 请勿将真实 token、MCP 环境变量、未脱敏清单或 SQLite 数据库提交到仓库。

更多安全约束请参阅 [SECURITY.md](SECURITY.md)。

## 项目结构

```text
src/                 React 界面、组件策略和 Tauri IPC 调用
src-tauri/           Rust 扫描器、规划器、持久化、备份和恢复逻辑
docs/                产品需求、设计、实现、验证和桥接契约
```

重点文档：

- [产品需求](docs/prd.md)
- [设计说明](docs/design.md)
- [实现说明](docs/implement.md)
- [验证记录](docs/validation.md)
- [桌面桥接契约](docs/bridge-contracts.md)
- [原生实现契约](docs/native-contracts.md)

## 故障排查

### 点击预览没有反应

先查看组件详情抽屉中的错误提示。项目级 MCP 必须关联到已配置的项目根目录；如果配置路径发生变化，请重新保存设置并扫描。

### 扫描结果为空

确认用户主目录和项目目录填写正确，然后点击“保存设置”并重新扫描。Codex 和 Claude Code 的目录可以单独配置。

### 只能看到预览，不能应用

检查当前平台和 `capabilities` 返回的写入能力。非 Windows 环境、只读观察项、插件所有权资源和继承绑定不会提供独立写入操作。

### Rust 依赖下载失败

确认网络可以访问 crates.io，或配置可用的 Cargo 镜像后重新运行：

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --locked
```

## 贡献代码

提交修改前请使用虚构配置和临时目录编写测试，禁止使用真实 Agent 配置、凭据、机器路径、数据库或备份。提交前运行 lint、构建、前端测试和 Rust 测试。

详细规则见 [CONTRIBUTING.md](CONTRIBUTING.md)。

## 许可证

本项目采用 MIT License，详见 [LICENSE](LICENSE)。
