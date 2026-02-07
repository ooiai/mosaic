import type { ShellSnapshot } from './types';
import { WEB_SNAPSHOT } from './snapshot';

const tauriWindow = (): Window & { __TAURI__?: unknown; __TAURI_INTERNALS__?: unknown } =>
  window as Window & { __TAURI__?: unknown; __TAURI_INTERNALS__?: unknown };

export const isTauriRuntime = (): boolean => {
  const scope = tauriWindow();
  return Boolean(scope.__TAURI__ || scope.__TAURI_INTERNALS__);
};

export const loadShellSnapshot = async (): Promise<ShellSnapshot> => {
  if (!isTauriRuntime()) {
    return WEB_SNAPSHOT;
  }

  const { invoke } = await import('@tauri-apps/api/core');
  return invoke<ShellSnapshot>('load_shell_snapshot');
};
