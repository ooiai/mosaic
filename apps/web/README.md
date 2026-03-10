# Mosaic Web

React + Vite workbench prototype for Mosaic.

`apps/web` is now a standalone web surface. The native desktop client lives in [`/Users/jerrychir/Desktop/dev/coding/ooiai/mosaic/apps/macos`](/Users/jerrychir/Desktop/dev/coding/ooiai/mosaic/apps/macos); this package is no longer embedded into a separate desktop shell.

## Scope

- Renders the workbench snapshot UI used for browser-based iteration.
- Depends on `@mosaic/ui` and `@mosaic/workbench`.
- Uses mock/workbench snapshot data rather than native desktop bindings.

## Commands

From the repository root:

```bash
pnpm install
pnpm dev:web
pnpm build:web
pnpm lint:web
pnpm typecheck:web
```

From `apps/web` directly:

```bash
pnpm dev
pnpm build
pnpm lint
pnpm typecheck
```

## Structure

```text
apps/web/
├── src/
│   ├── App.tsx
│   ├── App.css
│   ├── assets/
│   └── main.tsx
├── index.html
├── vite.config.ts
├── tsconfig.app.json
└── package.json
```

## Notes

- Legacy desktop bridge dependencies have been removed.
- `@mosaic/workbench` now resolves to the web snapshot runtime in browser builds.
- Native macOS work happens in the Swift package under [`/Users/jerrychir/Desktop/dev/coding/ooiai/mosaic/apps/macos`](/Users/jerrychir/Desktop/dev/coding/ooiai/mosaic/apps/macos).
