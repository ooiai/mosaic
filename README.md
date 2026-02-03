# 🎨 Mosaic

Mosaic is an AI-powered visual frontend development platform that revolutionizes how developers create user interfaces. Through conversational AI, developers can generate, modify, and iterate on frontend code with natural language, making UI development more intuitive and efficient.

## ✨ Features

### 🎯 **Phase 1: VIBECODING - Visual Frontend Generation**

- 💬 **Conversational UI Development**: Generate frontend components through natural language dialogue
- 🎨 **Visual Code Editor**: Interactive interface for creating and editing React components
- 🔧 **Targeted Module Updates**: Specify and modify individual components or sections
- ⚡ **Real-time Preview**: See changes instantly as you describe them
- 🧩 **Component Library**: Build and reuse custom components across projects

### 🚀 **Phase 2: Requirements to Frontend Pipeline**

- 📝 **Natural Language Requirements**: Transform plain English requirements into complete frontend applications
- 🎯 **Intelligent Architecture**: Automatically structure projects based on requirements
- 🔄 **Iterative Refinement**: Continuously improve generated code through conversation
- 📊 **Project Management Integration**: Seamlessly connect requirements to implementation

### 🌟 **Phase 3: Full-Stack Generation**

- 🗄️ **Backend Generation**: Create complete backend services from requirements
- 🔗 **API Integration**: Automatically generate and connect frontend to backend APIs
- 🏗️ **Database Design**: Intelligent database schema generation and migration management
- 🚀 **Deployment Ready**: End-to-end application generation ready for production

### 🛠️ **Core Technology Features**

- 🌐 **Universal Frontend**: Single React codebase runs in both web browsers and as a native desktop app
- ⚡ **Fast & Lightweight**: Built with Vite for lightning-fast development and optimized builds
- 🦀 **Rust Backend**: Secure and performant native backend powered by Tauri
- 📱 **Cross-Platform**: Supports Windows, macOS, and Linux
- 🎯 **Modular Architecture**: Clean separation between frontend and desktop concerns
- 🔧 **Developer Friendly**: Hot reload, TypeScript support, and modern tooling

## 🗺️ Product Roadmap

### Phase 1: VIBECODING (Current Focus)

**Visual Frontend Development Platform**

- ✅ Conversational UI generation
- ✅ Component-level modifications
- 🔄 Visual code editor interface
- 🔄 Advanced component library
- 🔄 Project templates and scaffolding

### Phase 2: Requirements Engine

**Natural Language to Frontend Pipeline**

- 📋 Requirement parsing and analysis
- 🏗️ Architecture generation from specs
- 🔄 Iterative requirement refinement
- 📊 Project structure automation
- 🎯 Multi-page application generation

### Phase 3: Full-Stack Generation

**Complete Application Development**

- 🗄️ Backend API generation
- 🔗 Database schema creation
- 🚀 Deployment configuration
- 🔧 DevOps pipeline setup
- 📈 Performance optimization

## 🏗️ Project Structure

```
mosaic/
├── apps/
│   ├── desktop/           # Tauri desktop application
│   │   └── src-tauri/     # Rust backend code
│   └── web/               # React frontend application (VIBECODING UI)
├── packages/              # Shared packages (AI models, code generators)
├── crates/               # Rust crates (code analysis, generation engines)
└── pnpm-workspace.yaml   # Workspace configuration
```

## 🚀 Quick Start

### Prerequisites

- [Node.js](https://nodejs.org/) (v18 or later)
- [pnpm](https://pnpm.io/) (recommended package manager)
- [Rust](https://rustup.rs/) (for desktop app development)

### Installation

1. Clone the repository:

   ```bash
   git clone git@github.com:ooiai/mosaic.git
   cd mosaic
   ```

2. Install dependencies:
   ```bash
   pnpm install
   ```

## 🛠️ Development

### Web Application

Run the web version for fast development:

```bash
cd apps/web
pnpm dev
```

The web app will be available at `http://localhost:5173`

### Desktop Application

Run the desktop application with hot reload:

```bash
cd apps/desktop
pnpm dev
```

This will:

1. Start the web development server
2. Launch the Tauri desktop app
3. Enable hot reload for both frontend and backend changes

## 📦 Building

### Web Application

Build the optimized web application:

```bash
cd apps/web
pnpm build
```

### Desktop Application

Build the desktop application for your platform:

```bash
cd apps/desktop
pnpm build
```

This will create platform-specific installers in `apps/desktop/src-tauri/target/release/bundle/`

## 🧪 Available Scripts

### Root Level

- `pnpm dev:web` - Start web development server
- `pnpm build:web` - Build web application
- `pnpm preview:web` - Preview built web application
- `pnpm lint:web` - Lint web application code

### Desktop App (`apps/desktop`)

- `pnpm dev` - Start desktop app in development mode
- `pnpm build` - Build desktop app for production
- `pnpm tauri` - Run Tauri CLI commands

### Web App (`apps/web`)

- `pnpm dev` - Start development server
- `pnpm build` - Build for production
- `pnpm preview` - Preview production build
- `pnpm lint` - Lint code

## 🏛️ Architecture

### Frontend (React + Vite)

- **Location**: `apps/web/`
- **Framework**: React 19 with TypeScript
- **Build Tool**: Vite (using Rolldown)
- **Features**:
  - Environment detection (web vs Tauri)
  - Conditional Tauri API usage
  - Hot module replacement

### Desktop Backend (Rust + Tauri)

- **Location**: `apps/desktop/src-tauri/`
- **Framework**: Tauri v2
- **Features**:
  - Native system integration
  - Secure IPC communication
  - Cross-platform compatibility

### Key Design Decisions

1. **Shared Frontend**: The same React application runs in both environments
2. **Environment Detection**: Runtime detection of Tauri vs web environment
3. **Graceful Degradation**: Web features work standalone, desktop features enhance the experience
4. **Monorepo Structure**: Organized for scalability and code sharing

## 🔧 Configuration

### Tauri Configuration

The desktop app configuration is in `apps/desktop/src-tauri/tauri.conf.json`:

- **Development**: Uses web dev server at `localhost:5173`
- **Production**: Uses built web assets from `apps/web/dist`
- **Window**: 800x600 default size, customizable

### Vite Configuration

Web application uses standard Vite configuration with React plugin.

## 🚀 Deployment

### Web Deployment

The web application can be deployed to any static hosting service:

```bash
cd apps/web
pnpm build
# Deploy the dist/ folder
```

Recommended platforms:

- Vercel
- Netlify
- GitHub Pages
- Cloudflare Pages

### Desktop Distribution

Desktop applications are built as platform-specific installers:

- **Windows**: `.msi` installer
- **macOS**: `.dmg` disk image
- **Linux**: `.AppImage`, `.deb`, or `.rpm` packages

## 🤝 Contributing

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🎯 Vision

Mosaic aims to democratize frontend development by making it as simple as describing what you want to build. Our vision is to eliminate the complexity barrier in web development, enabling anyone to create sophisticated user interfaces through natural conversation.

By combining AI-powered code generation with visual development tools, we're building the future where ideas can be transformed into working applications in minutes, not months.

## 🤖 AI Integration

Mosaic leverages cutting-edge AI technologies to understand developer intent and generate high-quality, maintainable code:

- **Natural Language Processing**: Understands complex UI requirements and design specifications
- **Code Intelligence**: Generates semantic, accessible, and performant React components
- **Context Awareness**: Maintains project consistency and follows established patterns
- **Iterative Learning**: Improves suggestions based on user feedback and preferences

## 🔗 Links

- **Repository**: [https://github.com/ooiai/mosaic](https://github.com/ooiai/mosaic)
- **Issues**: [https://github.com/ooiai/mosaic/issues](https://github.com/ooiai/mosaic/issues)
- **Releases**: [https://github.com/ooiai/mosaic/releases](https://github.com/ooiai/mosaic/releases)
- **Organization**: [OOIAI on GitHub](https://github.com/ooiai)

## 🙏 Acknowledgments

- [Tauri](https://tauri.app/) - For the amazing Rust-based desktop framework
- [React](https://reactjs.org/) - For the powerful frontend library
- [Vite](https://vitejs.dev/) - For the blazing-fast build tool
- [pnpm](https://pnpm.io/) - For efficient package management

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
  🎨 Revolutionizing Frontend Development with AI<br>
  Built with ❤️ by <a href="https://github.com/ooiai">OOIAI</a> using React and Tauri
</p>
