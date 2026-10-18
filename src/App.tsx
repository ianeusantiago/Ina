import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Sidebar } from "./components/Sidebar";
import { TeamPanel } from "./components/TeamPanel";
import { addWorkspace, listWorkspaces, removeWorkspace, type Workspace } from "./lib/workspaces";

export default function App() {
  const { t } = useTranslation();
  const [workspaces, setWorkspaces] = useState<Workspace[]>([]);
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);

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

  const selected = workspaces.find((w) => w.id === selectedId) ?? null;

  return (
    <main className="flex h-full flex-col">
      <header className="border-b border-line px-4 py-2 text-sm font-semibold tracking-wide">
        {t("app.name")}
      </header>
      <div className="flex min-h-0 flex-1">
        <Sidebar
          workspaces={workspaces}
          selectedId={selectedId}
          error={error}
          onSelect={setSelectedId}
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
            })
          }
        />
        <div className="min-h-0 min-w-0 flex-1 p-2">
          {selected ? (
            <TeamPanel key={selected.id} workspaceId={selected.id} workspaceName={selected.name} />
          ) : (
            <p className="p-4 text-sm text-muted">{t("workspaces.selectHint")}</p>
          )}
        </div>
      </div>
    </main>
  );
}
