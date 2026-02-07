import { useEffect, useMemo, useState } from 'react';
import './App.css';

type StageTab = 'unstaged' | 'staged';

type SidebarThread = {
  id: string;
  title: string;
  updatedAt: string;
};

type WorkspaceGroup = {
  id: string;
  name: string;
  threads: SidebarThread[];
};

type ChatSection =
  | { type: 'heading'; text: string }
  | { type: 'paragraph'; text: string }
  | { type: 'list'; items: string[] }
  | { type: 'code'; code: string };

type ChatBlock = {
  id: string;
  role: 'assistant' | 'user';
  sections: ChatSection[];
};

type ShellSnapshot = {
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

const WEB_SNAPSHOT: ShellSnapshot = {
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
        { type: 'code', code: 'pnpm -C /Users/jerrychir/Desktop/dev/coding/topeduction/topedu/frontend --filter @edublocks/core typecheck' },
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

const PRIMARY_NAV = ['New thread', 'Automations', 'Skills'];

const isTauriRuntime = (): boolean => {
  const scope = window as Window & { __TAURI__?: unknown; __TAURI_INTERNALS__?: unknown };
  return Boolean(scope.__TAURI__ || scope.__TAURI_INTERNALS__);
};

const loadShellSnapshot = async (): Promise<ShellSnapshot> => {
  if (!isTauriRuntime()) {
    return WEB_SNAPSHOT;
  }

  const { invoke } = await import('@tauri-apps/api/core');
  return invoke<ShellSnapshot>('load_shell_snapshot');
};

function App() {
  const [snapshot, setSnapshot] = useState<ShellSnapshot>(WEB_SNAPSHOT);
  const [activeThreadId, setActiveThreadId] = useState<string>(WEB_SNAPSHOT.activeThreadId);
  const [activeStageTab, setActiveStageTab] = useState<StageTab>(WEB_SNAPSHOT.defaultStageTab);
  const [composerValue, setComposerValue] = useState<string>('');
  const runtimeKind: 'desktop' | 'web' = isTauriRuntime() ? 'desktop' : 'web';

  useEffect(() => {
    let cancelled = false;
    const loadSnapshot = async () => {
      try {
        const nextSnapshot = await loadShellSnapshot();
        if (!cancelled) {
          setSnapshot(nextSnapshot);
          setActiveThreadId(nextSnapshot.activeThreadId);
          setActiveStageTab(nextSnapshot.defaultStageTab);
        }
      } catch (error) {
        if (!cancelled) {
          setSnapshot(WEB_SNAPSHOT);
          setActiveThreadId(WEB_SNAPSHOT.activeThreadId);
          setActiveStageTab(WEB_SNAPSHOT.defaultStageTab);
          console.error('Unable to load desktop snapshot, fallback to web snapshot', error);
        }
      }
    };

    void loadSnapshot();

    return () => {
      cancelled = true;
    };
  }, []);

  const flattenedThreads = useMemo(
    () => snapshot.workspaces.flatMap((workspace) => workspace.threads),
    [snapshot.workspaces],
  );

  const activeThread = useMemo(
    () =>
      flattenedThreads.find((thread) => thread.id === activeThreadId) ??
      flattenedThreads[0] ??
      { id: 'fallback-thread', title: snapshot.shellTitle, updatedAt: 'now' },
    [activeThreadId, flattenedThreads, snapshot.shellTitle],
  );

  const activeWorkspaceName = useMemo(() => {
    const hostWorkspace = snapshot.workspaces.find((workspace) =>
      workspace.threads.some((thread) => thread.id === activeThread.id),
    );
    return hostWorkspace?.name ?? 'workspace';
  }, [activeThread.id, snapshot.workspaces]);

  const activeCount = activeStageTab === 'unstaged' ? snapshot.unstagedCount : snapshot.stagedCount;
  const activeStagePlaceholder =
    activeStageTab === 'unstaged'
      ? 'No unstaged changes'
      : 'No staged changes';

  const canSubmit = composerValue.trim().length > 0;

  return (
    <div className="codex-shell">
      <header className="shell-titlebar">
        <div className="traffic-lights" aria-hidden>
          <span className="dot dot-close" />
          <span className="dot dot-minimize" />
          <span className="dot dot-expand" />
        </div>
        <div className="title-main">
          <h1>{activeThread.title}</h1>
          <span>{activeWorkspaceName}</span>
        </div>
        <div className="title-actions">
          <button type="button" className="ghost-button">Open</button>
          <button type="button" className="ghost-button">Commit</button>
        </div>
      </header>

      <div className="shell-workbench">
        <aside className="left-sidebar">
          <nav className="top-nav" aria-label="Primary navigation">
            {PRIMARY_NAV.map((item) => (
              <button key={item} type="button" className="top-nav-item">
                <span className="tiny-glyph" aria-hidden />
                {item}
              </button>
            ))}
          </nav>

          <section className="thread-area">
            <header className="thread-heading">
              <span>Threads</span>
              <div className="thread-tools" aria-hidden>
                <span />
                <span />
              </div>
            </header>

            {snapshot.workspaces.map((workspace) => (
              <div key={workspace.id} className="workspace-group">
                <p className="workspace-name">{workspace.name}</p>
                <div className="workspace-list">
                  {workspace.threads.map((thread) => (
                    <button
                      key={thread.id}
                      type="button"
                      className={`thread-row ${activeThreadId === thread.id ? 'is-active' : ''}`}
                      onClick={() => setActiveThreadId(thread.id)}
                    >
                      <span>{thread.title}</span>
                      <time>{thread.updatedAt}</time>
                    </button>
                  ))}
                </div>
              </div>
            ))}
          </section>

          <footer className="sidebar-footer">Settings</footer>
        </aside>

        <main className="conversation-pane">
          <div className="conversation-scroll">
            {snapshot.chatBlocks.map((block) => (
              <article key={block.id} className={`chat-block ${block.role === 'user' ? 'is-user' : 'is-assistant'}`}>
                {block.sections.map((section, index) => {
                  const key = `${block.id}-${section.type}-${index}`;
                  if (section.type === 'heading') {
                    return <h2 key={key}>{section.text}</h2>;
                  }
                  if (section.type === 'paragraph') {
                    return <p key={key}>{section.text}</p>;
                  }
                  if (section.type === 'code') {
                    return (
                      <pre key={key}>
                        <code>{section.code}</code>
                      </pre>
                    );
                  }
                  return (
                    <ul key={key}>
                      {section.items.map((item) => (
                        <li key={item}>{item}</li>
                      ))}
                    </ul>
                  );
                })}
              </article>
            ))}
          </div>

          <div className="composer">
            <textarea
              value={composerValue}
              onChange={(event) => setComposerValue(event.target.value)}
              placeholder="Ask for follow-up changes"
              rows={3}
            />
            <div className="composer-controls">
              <div className="composer-meta">
                <span>GPT-5.3-Codex</span>
                <span>Extra High</span>
              </div>
              <button type="button" className="send-button" disabled={!canSubmit}>
                Send
              </button>
            </div>
            <div className="composer-footer">
              <span>{runtimeKind === 'desktop' ? 'Desktop' : 'Web'}</span>
              <span>{snapshot.runtimeLabel}</span>
              <span>{snapshot.branch}</span>
            </div>
          </div>
        </main>

        <aside className="right-sidebar">
          <header className="right-header">
            <h3>Uncommitted changes</h3>
            <div className="stage-tabs" role="tablist" aria-label="Stage selector">
              <button
                type="button"
                role="tab"
                aria-selected={activeStageTab === 'unstaged'}
                className={activeStageTab === 'unstaged' ? 'is-active' : ''}
                onClick={() => setActiveStageTab('unstaged')}
              >
                Unstaged
              </button>
              <button
                type="button"
                role="tab"
                aria-selected={activeStageTab === 'staged'}
                className={activeStageTab === 'staged' ? 'is-active' : ''}
                onClick={() => setActiveStageTab('staged')}
              >
                Staged
              </button>
            </div>
          </header>

          <div className="changes-empty">
            {activeCount === 0 ? (
              <>
                <p>{activeStagePlaceholder}</p>
                <span>{activeStageTab === 'unstaged' ? 'Code changes will appear here' : 'Accept edits to stage them'}</span>
              </>
            ) : (
              <p>{activeCount} file changes</p>
            )}
          </div>
        </aside>
      </div>
    </div>
  );
}

export default App;
