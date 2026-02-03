# 🌐 Mosaic Web - VIBECODING Platform

The visual frontend development platform component of Mosaic, built with React and Vite. This application provides the VIBECODING interface where developers can create and modify UI components through conversational AI. It runs both as a standalone web app and as the UI for the desktop Tauri application.

## ✨ Features

### 🎨 VIBECODING Platform Features

- 💬 **Conversational UI Generation**: Create React components by describing them in natural language
- 🔧 **Interactive Code Editor**: Visual interface for editing generated components
- 🎯 **Targeted Modifications**: Precisely modify specific parts of components through dialogue
- ⚡ **Real-time Preview**: See component changes instantly as you describe them
- 🧩 **Component Library**: Save and reuse generated components across projects
- 🔄 **Iterative Development**: Continuously refine components through conversation

### 🛠️ Technical Features

- ⚛️ **React 19**: Latest React with modern hooks and features
- ⚡ **Vite**: Lightning-fast development with Rolldown bundler
- 🎯 **TypeScript**: Full type safety and excellent DX
- 🔧 **ESLint**: Code quality and consistency
- 🖥️ **Tauri Integration**: Seamlessly works with desktop app
- 🌍 **Universal**: Runs in browsers and as desktop app UI

## 🚀 Quick Start

### Prerequisites

- [Node.js](https://nodejs.org/) v18 or later
- [pnpm](https://pnpm.io/) (recommended package manager)

### Installation

From the root of the monorepo:

```bash
pnpm install
```

### Development

Start the development server:

```bash
pnpm dev
```

The application will be available at `http://localhost:5173`

### Building

Build for production:

```bash
pnpm build
```

Preview the production build:

```bash
pnpm preview
```

## 🏗️ Architecture

### VIBECODING Platform Architecture

The platform is designed around conversational code generation:

- **AI Integration Layer**: Processes natural language inputs and generates code
- **Code Editor Interface**: Visual editing and preview of generated components
- **Component Management**: Storage and organization of created components
- **Real-time Compilation**: Instant feedback on generated and modified code

### Environment Detection

The app automatically detects whether it's running in:
- **Web Environment**: Browser-based VIBECODING platform
- **Tauri Environment**: Desktop app with enhanced native features

### Tauri Integration

When running in the desktop app, the platform gains additional capabilities:
- File system access for project management
- Native code execution and compilation
- Enhanced performance for large projects
- Local AI model integration (future)

Example usage:

```typescript
import { invoke } from "@tauri-apps/api/core";

// Enhanced desktop features
async function saveProject() {
  if (window.__TAURI__) {
    const result = await invoke("save_project", { project: data });
    console.log("Project saved locally");
  } else {
    console.log("Cloud save in web mode");
  }
}
```

## 📁 Project Structure

```
apps/web/
├── src/
│   ├── components/      # VIBECODING UI components
│   │   ├── Editor/      # Code editor interface
│   │   ├── Chat/        # Conversational interface
│   │   ├── Preview/     # Component preview area
│   │   └── Library/     # Component library browser
│   ├── services/        # AI integration and API services
│   ├── utils/           # Code generation and parsing utilities
│   ├── hooks/           # React hooks for VIBECODING features
│   ├── assets/          # Static assets
│   ├── App.tsx          # Main VIBECODING platform
│   ├── main.tsx         # Application entry point
│   ├── index.css        # Global styles
│   └── vite-env.d.ts    # Vite and Tauri type definitions
├── public/              # Public static files
├── dist/                # Build output (used by desktop app)
├── index.html           # HTML template
├── vite.config.ts       # Vite configuration
├── tsconfig.json        # TypeScript configuration
├── eslint.config.js     # ESLint configuration
└── package.json         # Dependencies and scripts
```

## 🛠️ Development

### Available Scripts

- `pnpm dev` - Start development server with HMR
- `pnpm build` - Build for production
- `pnpm preview` - Preview production build locally
- `pnpm lint` - Run ESLint for code quality

### Development Features

- **Hot Module Replacement**: Instant updates during development
- **TypeScript**: Full type checking and IntelliSense
- **ESLint**: Automatic code quality checks
- **Modern React**: Latest React 19 features and patterns

### Adding New VIBECODING Features

1. **AI Integration**: Add new conversation capabilities
```typescript
// Example: Adding a new AI command
const handleGenerateComponent = async (description: string) => {
  const response = await aiService.generateComponent({
    description,
    framework: 'react',
    typescript: true
  });
  setGeneratedCode(response.code);
};
```

2. **Desktop-enhanced features**: Use environment detection for advanced capabilities:

```typescript
const isDesktop = !!window.__TAURI__;

if (isDesktop) {
  // Enhanced desktop features: file system, local AI models
  await invoke("save_component_to_disk", { component });
} else {
  // Web features: cloud storage, online AI APIs
  await saveToCloud(component);
}
```

## 🎨 Styling

The application uses CSS modules and standard CSS. Key files:

- `src/index.css` - Global styles and CSS reset
- `src/App.css` - Component-specific styles

### Adding Styles

1. **Global styles**: Add to `src/index.css`
2. **Component styles**: Create `.module.css` files
3. **Inline styles**: Use for dynamic styling

## 📦 Dependencies

### Core Dependencies

- **react** & **react-dom**: UI framework
- **@tauri-apps/api**: Desktop app integration
- **@tauri-apps/plugin-opener**: System integration plugin

### Development Dependencies

- **vite**: Build tool and dev server
- **@vitejs/plugin-react**: React support for Vite
- **typescript**: Type checking
- **eslint**: Code linting
- **@types/***: TypeScript definitions

### AI and Code Generation Dependencies (Future)

- **@babel/parser**: Code parsing for modifications
- **prettier**: Code formatting for generated components
- **monaco-editor**: Advanced code editor integration
- **ai/openai**: AI service integration for conversation

## 🚀 Deployment

### Web Deployment

The built application (`dist/` folder) can be deployed to any static hosting:

**Recommended platforms:**
- [Vercel](https://vercel.com/)
- [Netlify](https://netlify.com/)
- [GitHub Pages](https://pages.github.com/)
- [Cloudflare Pages](https://pages.cloudflare.com/)

**Deployment steps:**
```bash
pnpm build
# Upload dist/ folder to your hosting platform
```

### Desktop Integration

When used with the desktop app:
- Development: Runs on `localhost:5173`
- Production: Bundled into the desktop app from `dist/`

## 🔧 Configuration

### Vite Configuration

The `vite.config.ts` uses standard React plugin configuration:

```typescript
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

export default defineConfig({
  plugins: [react()],
})
```

### TypeScript Configuration

- `tsconfig.json`: Main TypeScript config
- `tsconfig.app.json`: Application-specific config
- `tsconfig.node.json`: Node.js tooling config

### ESLint Configuration

Modern flat config with React-specific rules and best practices.

## 🐛 Troubleshooting

### Common Issues

1. **Port conflicts**: Vite dev server uses port 5173 by default
2. **Tauri API errors**: Check if running in desktop environment
3. **Build failures**: Ensure all dependencies are installed

### Development Tips

- Use browser dev tools for debugging web features
- Use desktop app's dev tools for Tauri-specific debugging
- Check console for environment detection logs

## 📚 Resources

- [React Documentation](https://react.dev/)
- [Vite Documentation](https://vitejs.dev/)
- [TypeScript Handbook](https://www.typescriptlang.org/docs/)
- [Tauri Frontend Guide](https://tauri.app/develop/frontend/)

## 🤝 Contributing

When contributing to the web application:

1. Ensure compatibility with both web and desktop environments
2. Test in both contexts when making changes
3. Follow the established code style and patterns
4. Update tests and documentation as needed

## 🎯 VIBECODING Vision

This web application serves as the core VIBECODING platform, revolutionizing how developers create frontend components:

- **Natural Conversation**: Describe components in plain English
- **Instant Generation**: See your ideas become code immediately
- **Iterative Refinement**: Perfect components through dialogue
- **Visual Development**: Bridge the gap between design and code

### Future Enhancements

- **Advanced AI Models**: Support for specialized design and coding models
- **Multi-framework Support**: Extend beyond React to Vue, Angular, and more
- **Design System Integration**: Automatic adherence to design system guidelines
- **Collaborative Features**: Real-time collaboration on component generation

---

This web application is the foundation of the VIBECODING experience, making frontend development more intuitive and accessible through conversational AI.
