# 🎨 Mosaic

Mosaic 是一个基于 AI 的可视化前端开发平台，通过对话式 AI 革命性地改变开发者创建用户界面的方式。开发者可以用自然语言生成、修改和迭代前端代码，让 UI 开发变得更加直观和高效。

## ✨ 功能特性

### 🎯 **第一期：VIBECODING - 可视化前端生成**

- 💬 **对话式 UI 开发**：通过自然语言对话生成前端组件
- 🎨 **可视化代码编辑器**：交互式界面用于创建和编辑 React 组件
- 🔧 **精准模块修改**：指定并修改单个组件或部分
- ⚡ **实时预览**：在你描述的同时即时查看变化
- 🧩 **组件库**：在项目中构建和复用自定义组件

### 🚀 **第二期：需求到前端流水线**

- 📝 **自然语言需求**：将简单的中文需求转换为完整的前端应用
- 🎯 **智能架构**：根据需求自动构建项目结构
- 🔄 **迭代式完善**：通过对话持续改进生成的代码
- 📊 **项目管理集成**：无缝连接需求到实现

### 🌟 **第三期：全栈生成**

- 🗄️ **后端生成**：从需求创建完整的后端服务
- 🔗 **API 集成**：自动生成并连接前端到后端 API
- 🏗️ **数据库设计**：智能数据库架构生成和迁移管理
- 🚀 **部署就绪**：端到端应用程序生成，可直接投入生产

### 🛠️ **核心技术特性**

- 🌐 **通用前端**：单一 React 代码库可在浏览器和桌面应用中运行
- ⚡ **快速轻量**：使用 Vite 构建，开发快如闪电，构建优化
- 🦀 **Rust 后端**：基于 Tauri 的安全高性能原生后端
- 📱 **跨平台**：支持 Windows、macOS 和 Linux
- 🎯 **模块化架构**：前端和桌面应用职责清晰分离
- 🔧 **开发者友好**：热重载、TypeScript 支持和现代工具链

## 🗺️ 产品路线图

### 第一期：VIBECODING（当前重点）

**可视化前端开发平台**

- ✅ 对话式 UI 生成
- ✅ 组件级修改
- 🔄 可视化代码编辑器界面
- 🔄 高级组件库
- 🔄 项目模板和脚手架

### 第二期：需求引擎

**自然语言到前端流水线**

- 📋 需求解析和分析
- 🏗️ 根据规范生成架构
- 🔄 需求迭代完善
- 📊 项目结构自动化
- 🎯 多页面应用生成

### 第三期：全栈生成

**完整应用开发**

- 🗄️ 后端 API 生成
- 🔗 数据库架构创建
- 🚀 部署配置
- 🔧 DevOps 流水线设置
- 📈 性能优化

## 🏗️ 项目结构

```
mosaic/
├── apps/
│   ├── desktop/           # Tauri 桌面应用
│   │   └── src-tauri/     # Rust 后端代码
│   └── web/               # React 前端应用（VIBECODING UI）
├── packages/              # 共享包（AI 模型、代码生成器）
├── crates/               # Rust 包（代码分析、生成引擎）
└── pnpm-workspace.yaml   # 工作空间配置
```

## 🚀 快速开始

### 前置要求

- [Node.js](https://nodejs.org/) (v18 或更高版本)
- [pnpm](https://pnpm.io/) (推荐的包管理器)
- [Rust](https://rustup.rs/) (用于桌面应用开发)

### 安装

1. 克隆仓库：

   ```bash
   git clone git@github.com:ooiai/mosaic.git
   cd mosaic
   ```

2. 安装依赖：

   ```bash
   pnpm install
   ```

## 🛠️ 开发

### Web 应用

运行 web 版本进行快速开发：

```bash
cd apps/web
pnpm dev
```

web 应用将在 `http://localhost:5173` 可用

### 桌面应用

运行带有热重载的桌面应用：

```bash
cd apps/desktop
pnpm dev
```

这将会：

1. 启动 web 开发服务器
2. 启动 Tauri 桌面应用
3. 启用前端和后端变化的热重载

## 📦 构建

### Web 应用

构建优化的 web 应用：

```bash
cd apps/web
pnpm build
```

### 桌面应用

为你的平台构建桌面应用：

```bash
cd apps/desktop
pnpm build
```

这将在 `apps/desktop/src-tauri/target/release/bundle/` 中创建平台特定的安装包

## 🧪 可用脚本

### 根目录

- `pnpm dev:web` - 启动 web 开发服务器
- `pnpm build:web` - 构建 web 应用
- `pnpm preview:web` - 预览构建的 web 应用
- `pnpm lint:web` - 代码检查 web 应用

### 桌面应用 (`apps/desktop`)

- `pnpm dev` - 以开发模式启动桌面应用
- `pnpm build` - 构建生产版桌面应用
- `pnpm tauri` - 运行 Tauri CLI 命令

### Web 应用 (`apps/web`)

- `pnpm dev` - 启动开发服务器
- `pnpm build` - 构建生产版本
- `pnpm preview` - 预览生产构建
- `pnpm lint` - 代码检查

## 🏛️ 架构

### 前端 (React + Vite)

- **位置**：`apps/web/`
- **框架**：React 19 配合 TypeScript
- **构建工具**：Vite（使用 Rolldown）
- **特性**：
  - 环境检测（web vs Tauri）
  - 条件式 Tauri API 使用
  - 热模块替换

### 桌面后端 (Rust + Tauri)

- **位置**：`apps/desktop/src-tauri/`
- **框架**：Tauri v2
- **特性**：
  - 原生系统集成
  - 安全的 IPC 通信
  - 跨平台兼容性

### 关键设计决策

1. **共享前端**：同一个 React 应用在两个环境中运行
2. **环境检测**：运行时检测 Tauri vs web 环境
3. **优雅降级**：web 功能独立工作，桌面功能增强体验
4. **单体仓库结构**：为可扩展性和代码共享而组织

## 🔧 配置

### Tauri 配置

桌面应用配置位于 `apps/desktop/src-tauri/tauri.conf.json`：

- **开发**：使用 `localhost:5173` 的 web 开发服务器
- **生产**：使用 `apps/web/dist` 的构建资源
- **窗口**：800x600 默认大小，可自定义

### Vite 配置

Web 应用使用标准的 Vite 配置配合 React 插件。

## 🚀 部署

### Web 部署

web 应用可以部署到任何静态托管服务：

```bash
cd apps/web
pnpm build
# 部署 dist/ 文件夹
```

推荐平台：

- Vercel
- Netlify
- GitHub Pages
- Cloudflare Pages

### 桌面分发

桌面应用构建为平台特定的安装包：

- **Windows**：`.msi` 安装程序
- **macOS**：`.dmg` 磁盘镜像
- **Linux**：`.AppImage`、`.deb` 或 `.rpm` 包

## 🎯 愿景

Mosaic 旨在通过让前端开发变得像描述你想要构建的东西一样简单来民主化前端开发。我们的愿景是消除 web 开发中的复杂性障碍，让任何人都能通过自然对话创建复杂的用户界面。

通过结合 AI 驱动的代码生成和可视化开发工具，我们正在构建一个未来，想法可以在几分钟内（而不是几个月）转化为工作的应用程序。

## 🤖 AI 集成

Mosaic 利用尖端的 AI 技术来理解开发者意图并生成高质量、可维护的代码：

- **自然语言处理**：理解复杂的 UI 需求和设计规范
- **代码智能**：生成语义化、可访问且高性能的 React 组件
- **上下文感知**：维护项目一致性并遵循既定模式
- **迭代学习**：根据用户反馈和偏好改进建议

## 🔗 链接

- **仓库地址**: [https://github.com/ooiai/mosaic](https://github.com/ooiai/mosaic)
- **问题反馈**: [https://github.com/ooiai/mosaic/issues](https://github.com/ooiai/mosaic/issues)
- **发布版本**: [https://github.com/ooiai/mosaic/releases](https://github.com/ooiai/mosaic/releases)
- **组织主页**: [OOIAI on GitHub](https://github.com/ooiai)

## 🤝 贡献

1. Fork 仓库
2. 创建你的功能分支 (`git checkout -b feature/amazing-feature`)
3. 提交你的更改 (`git commit -m 'Add some amazing feature'`)
4. 推送到分支 (`git push origin feature/amazing-feature`)
5. 开启 Pull Request

## 📄 许可证

此项目基于 MIT 许可证 - 查看 [LICENSE](LICENSE) 文件了解详情。

## 🙏 致谢

- [Tauri](https://tauri.app/) - 提供出色的基于 Rust 的桌面框架
- [React](https://reactjs.org/) - 提供强大的前端库
- [Vite](https://vitejs.dev/) - 提供极速的构建工具
- [pnpm](https://pnpm.io/) - 提供高效的包管理

---

<p align="center">
  <a href="https://github.com/ooiai/mosaic">
    <img src="https://img.shields.io/github/stars/ooiai/mosaic?style=social" alt="GitHub stars">
  </a>
  <a href="https://github.com/ooiai/mosaic/blob/main/LICENSE">
    <img src="https://img.shields.io/github/license/ooiai/mosaic" alt="License">
  </a>
  <a href="https://github.com/ooiai/mosaic/releases">
    <img src="https://img.shields.io/github/v/release/ooiai/mosaic" alt="Latest Release">
  </a>
</p>

<p align="center">
  🎨 用 AI 革命化前端开发<br>
  由 <a href="https://github.com/ooiai">OOIAI</a> 用 ❤️、React 和 Tauri 构建
</p>
