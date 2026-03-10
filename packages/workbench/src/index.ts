export type {
  StageTab,
  SidebarThread,
  WorkspaceGroup,
  ChatSection,
  ChatBlock,
  ShellSnapshot,
} from './types';

export { WEB_SNAPSHOT } from './snapshot';
export { loadShellSnapshot } from './runtime';
export {
  PRIMARY_NAV_ITEMS,
  flattenThreads,
  resolveActiveThread,
  resolveWorkspaceName,
  stagePlaceholder,
} from './helpers';
