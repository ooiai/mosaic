import type { ShellSnapshot, StageTab, WorkspaceGroup, SidebarThread } from './types';

export const PRIMARY_NAV_ITEMS = [
  { id: 'new-thread', label: 'New thread', icon: 'compose' },
  { id: 'automations', label: 'Automations', icon: 'automation' },
  { id: 'skills', label: 'Skills', icon: 'skills' },
] as const;

export const flattenThreads = (workspaces: WorkspaceGroup[]): SidebarThread[] =>
  workspaces.flatMap((workspace) => workspace.threads);

export const resolveActiveThread = (
  snapshot: ShellSnapshot,
  activeThreadId: string,
): SidebarThread => {
  const allThreads = flattenThreads(snapshot.workspaces);
  return (
    allThreads.find((thread) => thread.id === activeThreadId) ??
    allThreads[0] ??
    { id: 'fallback-thread', title: snapshot.shellTitle, updatedAt: 'now' }
  );
};

export const resolveWorkspaceName = (
  workspaces: WorkspaceGroup[],
  activeThreadId: string,
): string => {
  const hostWorkspace = workspaces.find((workspace) =>
    workspace.threads.some((thread) => thread.id === activeThreadId),
  );
  return hostWorkspace?.name ?? 'workspace';
};

export const stagePlaceholder = (stageTab: StageTab): string =>
  stageTab === 'unstaged' ? 'No unstaged changes' : 'No staged changes';
