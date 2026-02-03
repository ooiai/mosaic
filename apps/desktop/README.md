# 🖥️ Mosaic Desktop - VIBECODING Platform

The desktop application component of Mosaic, built with Tauri v2. This native desktop app provides an enhanced VIBECODING experience with local file system access, improved performance, and desktop-specific features while using the shared React frontend from the web application.

## 🏗️ Architecture

This desktop application provides the VIBECODING platform with enhanced capabilities:

- **Frontend**: Uses the VIBECODING React application from `../web/` (no embedded frontend code)
- **Backend**: Rust-based Tauri application providing native system integration for project management
- **Communication**: Secure IPC between React frontend and Rust backend for file operations and AI processing
- **Enhanced Features**: Local project storage, file system access, and future local AI model integration

## 🚀 Quick Start

### Prerequisites

- [Rust](https://rustup.rs/) - Latest stable version
- [Node.js](https://nodejs.org/) - v18 or later
- [pnpm](https://pnpm.io/) - For package management

### Development

Start the desktop application in development mode:

```bash
pnpm dev
```

This will:
1. Start the VIBECODING web development server (`../web/`)
2. Launch the Tauri desktop application with enhanced native features
3. Enable hot reload for both frontend and backend development

### Building

Build the desktop application for production:

```bash
pnpm build
```

This creates platform-specific installers in `src-tauri/target/release/bundle/`:
- **Windows**: `.msi` installer
- **macOS**: `.dmg` disk image  
- **Linux**: `.AppImage`, `.deb`, `.rpm` packages

## 🔧 Configuration

### Tauri Configuration (`src-tauri/tauri.conf.json`)

Key configuration points:

- **Development URL**: `http://localhost:5173` (web dev server)
- **Build Output**: `../web/dist` (web app build artifacts)
- **Window Settings**: 800x600 default size
- **Bundle Identifier**: `com.ooiai.mosaic`

### Available Tauri Commands

The Rust backend exposes these commands to the VIBECODING frontend:

- `greet(name: string)` - Example greeting command
- `save_project(project: ProjectData)` - Save project to local file system
- `load_project(path: string)` - Load project from local file system
- `export_component(component: ComponentData)` - Export generated component to file
- `import_components(path: string)` - Import existing components from directory

## 📁 Project Structure

```
apps/desktop/
├── src-tauri/           # Rust backend
│   ├── src/
│   │   ├── lib.rs      # Main application logic
│   │   └── main.rs     # Entry point
│   ├── Cargo.toml      # Rust dependencies
│   └── tauri.conf.json # Tauri configuration
├── package.json        # npm scripts and dev dependencies
└── README.md           # This file
```

## 🛠️ Development

### Adding New VIBECODING Commands

1. Define the command in `src-tauri/src/lib.rs`:
```rust
#[tauri::command]
fn save_generated_component(name: &str, code: &str) -> Result<String, String> {
    // Save component to local file system
    std::fs::write(format!("components/{}.tsx", name), code)
        .map(|_| format!("Component {} saved successfully", name))
        .map_err(|e| e.to_string())
}
```

2. Register it in the invoke handler:
```rust
.invoke_handler(tauri::generate_handler![greet, save_generated_component])
```

3. Call from the VIBECODING frontend:
```typescript
import { invoke } from "@tauri-apps/api/core";
const result = await invoke("save_generated_component", { 
    name: "Button", 
    code: generatedCode 
});
```

### Adding Tauri Plugins for VIBECODING

1. Add to `src-tauri/Cargo.toml`:
```toml
[dependencies]
tauri-plugin-fs = "2"        # File system operations
tauri-plugin-dialog = "2"    # File dialogs for project management
tauri-plugin-shell = "2"     # Execute build commands
```

2. Initialize in `src-tauri/src/lib.rs`:
```rust
.plugin(tauri_plugin_fs::init())
.plugin(tauri_plugin_dialog::init())
.plugin(tauri_plugin_shell::init())
```

### VIBECODING Development Features

- **Project Management**: Local project storage and organization
- **Component Export**: Direct export to file system
- **Build Integration**: Execute build processes for generated code
- **File Watching**: Monitor changes in generated components

### Debugging

- **Rust Backend**: Use `cargo run` in `src-tauri/` directory for native features
- **VIBECODING Frontend**: Inspect element in the running desktop app
- **AI Integration**: Check console for code generation logs
- **File Operations**: Monitor terminal output for file system operations

## 🚀 Deployment

### Local Testing

Test the VIBECODING platform locally:

```bash
pnpm build
# Run the generated executable from target/release/
# Test AI-powered component generation and file operations
```

### Distribution

The build process creates VIBECODING platform installers:

- **Developer Distribution**: Direct download for development teams
- **Enterprise**: Custom deployment for organizations
- **App Stores**: Mac App Store, Microsoft Store distribution
- **Auto-updates**: Keep AI models and features current using Tauri's updater

### Deployment Considerations

- **AI Model Integration**: Bundle or configure access to AI services
- **File System Permissions**: Ensure proper access for project management
- **Performance Optimization**: Local caching for generated components

## 🔒 Security

VIBECODING platform security considerations:

- **CSP**: Content Security Policy for AI service communication
- **API Allowlist**: Controlled access to file system and external services
- **IPC Security**: Secure communication for code generation processes
- **Code Sandboxing**: Safe execution of generated code for preview
- **AI Service Security**: Encrypted communication with AI providers
- **Project Isolation**: Secure separation between different projects

## 📚 Resources

### Platform Documentation
- [Tauri Documentation](https://tauri.app/develop/)
- [Tauri API Reference](https://tauri.app/develop/api/)
- [Rust Learning Resources](https://doc.rust-lang.org/book/)

### VIBECODING Development
- [AI Integration Guide](../web/docs/ai-integration.md)
- [Component Generation Patterns](../web/docs/component-patterns.md)
- [Desktop Features Documentation](docs/desktop-features.md)

## 🐛 Troubleshooting

### Common VIBECODING Issues

1. **AI Service Connection**: Verify API keys and network connectivity
2. **File System Access**: Check permissions for project directories
3. **Component Generation Failures**: Review AI service logs and input validation
4. **Build Failures**: Ensure Rust toolchain supports required features
5. **Dev Server Issues**: Verify VIBECODING web app is running on port 5173

### Performance Issues

1. **Slow Code Generation**: Check AI service response times
2. **Large Project Handling**: Monitor memory usage for complex projects
3. **File System Operations**: Optimize for large component libraries

### Getting Help

- Check the [Tauri Discord](https://discord.com/invite/tauri) for technical issues
- Review [Tauri GitHub Issues](https://github.com/tauri-apps/tauri/issues)
- Consult the main project README for VIBECODING platform setup
- Check AI service documentation for generation issues

---

**VIBECODING Desktop Platform** - Bringing conversational UI development to your desktop with enhanced native capabilities.
