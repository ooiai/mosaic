import { Button, Icon, Pill, SegmentedTabs } from '@mosaic/ui';
import {
    PRIMARY_NAV_ITEMS,
    WEB_SNAPSHOT,
    isTauriRuntime,
    loadShellSnapshot,
    resolveActiveThread,
    resolveWorkspaceName,
    stagePlaceholder,
    type ChatSection,
    type ShellSnapshot,
    type StageTab,
} from '@mosaic/workbench';
import { useEffect, useMemo, useState } from 'react';
import './App.css';

const STAGE_OPTIONS: { value: StageTab; label: string }[] = [
  { value: 'unstaged', label: 'Unstaged' },
  { value: 'staged', label: 'Staged' },
];

const renderChatSection = (section: ChatSection, key: string) => {
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
};

function App() {
  const [snapshot, setSnapshot] = useState<ShellSnapshot>(WEB_SNAPSHOT);
  const [activeThreadId, setActiveThreadId] = useState<string>(WEB_SNAPSHOT.activeThreadId);
  const [activeStageTab, setActiveStageTab] = useState<StageTab>(WEB_SNAPSHOT.defaultStageTab);
  const [composerValue, setComposerValue] = useState<string>('');

  useEffect(() => {
    let cancelled = false;

    const run = async () => {
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
          console.error('Unable to load desktop snapshot. Falling back to web snapshot.', error);
        }
      }
    };

    void run();

    return () => {
      cancelled = true;
    };
  }, []);

  const runtimeKind: 'desktop' | 'web' = isTauriRuntime() ? 'desktop' : 'web';

  const activeThread = useMemo(
    () => resolveActiveThread(snapshot, activeThreadId),
    [snapshot, activeThreadId],
  );

  const activeWorkspaceName = useMemo(
    () => resolveWorkspaceName(snapshot.workspaces, activeThread.id),
    [snapshot.workspaces, activeThread.id],
  );

  const activeCount =
    activeStageTab === 'unstaged' ? snapshot.unstagedCount : snapshot.stagedCount;
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
          <Icon name="chevronDown" size={13} className="title-caret" />
        </div>

        <div className="title-actions">
          <Button variant="ghost" size="sm">
            <Icon name="compose" size={14} />
            Open
          </Button>
          <Button variant="ghost" size="sm">
            <Icon name="automation" size={14} />
            Commit
          </Button>
        </div>
      </header>

      <div className="shell-workbench">
        <aside className="left-sidebar">
          <nav className="top-nav" aria-label="Primary navigation">
            {PRIMARY_NAV_ITEMS.map((item) => (
              <Button key={item.id} variant="subtle" size="md" className="top-nav-item">
                <Icon
                  name={item.icon}
                  className="top-nav-icon"
                  size={14}
                />
                {item.label}
              </Button>
            ))}
          </nav>

          <section className="thread-area">
            <header className="thread-heading">
              <span>Threads</span>
              <div className="thread-tools" aria-hidden>
                <Icon name="compose" size={12} />
                <Icon name="automation" size={12} />
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

          <footer className="sidebar-footer">
            <Icon name="settings" size={14} />
            Settings
          </footer>
        </aside>

        <main className="conversation-pane">
          <div className="conversation-scroll">
            {snapshot.chatBlocks.map((block) => (
              <article
                key={block.id}
                className={`chat-block ${block.role === 'user' ? 'is-user' : 'is-assistant'}`}
              >
                {block.sections.map((section, index) =>
                  renderChatSection(section, `${block.id}-${section.type}-${index}`),
                )}
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
                <Pill>GPT-5.3-Codex</Pill>
                <Pill>Extra High</Pill>
              </div>

              <div className="composer-actions">
                <Button variant="subtle" size="sm" aria-label="Toggle voice input">
                  <Icon name="mic" size={14} />
                </Button>
                <Button variant="primary" size="sm" disabled={!canSubmit}>
                  <Icon name="send" size={14} />
                  Send
                </Button>
              </div>
            </div>

            <div className="composer-footer">
              <span>{runtimeKind === 'desktop' ? 'Desktop' : 'Web'}</span>
              <span>{snapshot.runtimeLabel}</span>
              <span className="branch-pill">
                <Icon name="branch" size={13} />
                {snapshot.branch}
              </span>
            </div>
          </div>
        </main>

        <aside className="right-sidebar">
          <header className="right-header">
            <h3>Uncommitted changes</h3>
            <SegmentedTabs
              ariaLabel="Stage selector"
              value={activeStageTab}
              onChange={setActiveStageTab}
              options={STAGE_OPTIONS}
            />
          </header>

          <div className="changes-empty">
            {activeCount === 0 ? (
              <>
                <p>{stagePlaceholder(activeStageTab)}</p>
                <span>
                  {activeStageTab === 'unstaged'
                    ? 'Code changes will appear here'
                    : 'Accept edits to stage them'}
                </span>
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
