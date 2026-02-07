import type { ShellSnapshot } from './types';

export const WEB_SNAPSHOT: ShellSnapshot = {
  shellTitle: 'Fix SlideStrip duplicate action',
  activeThreadId: 'thread-topedu-1',
  workspaces: [
    {
      id: 'ws-topedu',
      name: 'topedu',
      threads: [
        { id: 'thread-topedu-1', title: 'Fix SlideStrip duplicate action', updatedAt: '2h' },
        { id: 'thread-topedu-2', title: 'Install skill-installer', updatedAt: '2h' },
      ],
    },
    {
      id: 'ws-mosaic',
      name: 'mosaic',
      threads: [
        { id: 'thread-mosaic-1', title: 'Build Phase 1 UI framework', updatedAt: '2h' },
        { id: 'thread-mosaic-2', title: 'Send friendly greeting reply', updatedAt: '2h' },
      ],
    },
  ],
  chatBlocks: [
    {
      id: 'assistant-report',
      role: 'assistant',
      sections: [
        { type: 'paragraph', text: 'The duplicate slide action is fixed and verified with local typecheck.' },
        { type: 'heading', text: 'What changed' },
        {
          type: 'list',
          items: [
            'Added a safe active-slide update path to avoid stale references during quick navigation.',
            'Replaced direct active slide writes with a synchronized helper in editor flow.',
            'Kept existing behavior unchanged for normal edit workflows.',
          ],
        },
        { type: 'heading', text: 'Validation steps' },
        {
          type: 'list',
          items: [
            'Create a new page and type text in it.',
            'Click next page and immediately go back to previous page.',
            'Confirm text remains stable and is not overwritten.',
          ],
        },
        {
          type: 'code',
          code: 'pnpm -C /Users/jerrychir/Desktop/dev/coding/topeduction/topedu/frontend --filter @edublocks/core typecheck',
        },
      ],
    },
    {
      id: 'user-followup',
      role: 'user',
      sections: [
        {
          type: 'paragraph',
          text: 'Create a new page, switch quickly, then return. The previous page content should never be replaced.',
        },
      ],
    },
    {
      id: 'assistant-next',
      role: 'assistant',
      sections: [
        { type: 'paragraph', text: 'I can continue with the next UI phase and wire the component editing workflow.' },
      ],
    },
  ],
  branch: 'main',
  runtimeLabel: 'Web runtime',
  defaultStageTab: 'unstaged',
  unstagedCount: 0,
  stagedCount: 0,
};
