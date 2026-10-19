import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { SetupBanner } from "./components/SetupBanner";
import { Sidebar, type Focus } from "./components/Sidebar";
import { setLanguage } from "./i18n";
import { TeamPanel } from "./components/TeamPanel";
import { Home } from "./components/Home";
import { useTeamNotifications } from "./lib/notify";
import type { PlanDraft } from "./lib/team";
import { TitleBar } from "./components/TitleBar";
import { UsageBar } from "./components/UsageBar";
import { addWorkspace, listWorkspaces, removeWorkspace, type Workspace } from "./lib/workspaces";

export default function App() {
  const { t, i18n } = useTranslation();
  const [workspaces, setWorkspaces] = useState<Workspace[]>([]);
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [started, setStarted] = useState<PlanDraft | null>(null);
  const [focus, setFocus] = useState<Focus | null>(null);

  const run = useCallback(async (action: () => Promise<void>) => {
    try {
      setError(null);
      await action();
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    listWorkspaces()
      .then(setWorkspaces)
      .catch((e: unknown) => setError(String(e)));
  }, []);

  useTeamNotifications(workspaces);

  const selected = workspaces.find((w) => w.id === selectedId) ?? null;

  return (
    <main className="flex h-full flex-col overflow-hidden">
      <TitleBar
        language={i18n.language}
        onLanguage={(l) => setLanguage(l)}
        languageLabel={t("app.language")}
      />
      <SetupBanner />
      <div className="flex min-h-0 flex-1">
        <Sidebar
          workspaces={workspaces}
          selectedId={selectedId}
          error={error}
          onSelect={(id) => {
            setStarted(null);
            setFocus(null);
            setSelectedId(id);
          }}
          onHome={() => {
            setFocus(null);
            setSelectedId(null);
          }}
          focus={focus}
          onFocus={(f) => {
            setFocus(f);
            setSelectedId(f.workspaceId);
          }}
          onAdd={(path) =>
            void run(async () => {
              const w = await addWorkspace(path);
              setWorkspaces(await listWorkspaces());
              setSelectedId(w.id);
            })
          }
          onRemove={(id) =>
            void run(async () => {
              await removeWorkspace(id);
              setWorkspaces(await listWorkspaces());
              if (id === selectedId) setSelectedId(null);
              if (id === focus?.workspaceId) setFocus(null);
            })
          }
        />
        <div className="min-h-0 min-w-0 flex-1 p-3">
          {selected ? (
            <TeamPanel
              key={selected.id}
              workspaceId={selected.id}
              workspaceName={selected.name}
              workspacePath={selected.path}
              started={started ?? undefined}
              focus={focus?.workspaceId === selected.id ? focus : null}
            />
          ) : (
            <Home
              onWorkspacesChanged={() =>
                void listWorkspaces()
                  .then(setWorkspaces)
                  .catch((e: unknown) => setError(String(e)))
              }
              onStarted={(w, d) => {
                setStarted(d);
                setSelectedId(w.id);
              }}
            />
          )}
        </div>
      </div>
      <UsageBar />
    </main>
  );
}
