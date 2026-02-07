export type StageTab = 'unstaged' | 'staged';

export type SidebarThread = {
  id: string;
  title: string;
  updatedAt: string;
};

export type WorkspaceGroup = {
  id: string;
  name: string;
  threads: SidebarThread[];
};

export type ChatSection =
  | { type: 'heading'; text: string }
  | { type: 'paragraph'; text: string }
  | { type: 'list'; items: string[] }
  | { type: 'code'; code: string };

export type ChatBlock = {
  id: string;
  role: 'assistant' | 'user';
  sections: ChatSection[];
};

export type ShellSnapshot = {
  shellTitle: string;
  activeThreadId: string;
  workspaces: WorkspaceGroup[];
  chatBlocks: ChatBlock[];
  branch: string;
  runtimeLabel: string;
  defaultStageTab: StageTab;
  unstagedCount: number;
  stagedCount: number;
};
