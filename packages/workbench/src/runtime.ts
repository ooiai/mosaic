import type { ShellSnapshot } from './types';
import { WEB_SNAPSHOT } from './snapshot';

export const loadShellSnapshot = async (): Promise<ShellSnapshot> => WEB_SNAPSHOT;
