import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { SetupBanner } from "./components/SetupBanner";
import { Sidebar } from "./components/Sidebar";
import { LANGUAGES, setLanguage } from "./i18n";
import { TeamPanel } from "./components/TeamPanel";
import { addWorkspace, listWorkspaces, removeWorkspace, type Workspace } from "./lib/workspaces";

export default function App() {
  const { t, i18n } = useTranslation();
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
      <header className="flex items-center border-b border-line px-4 py-2 text-sm">
        <span className="font-semibold tracking-wide">{t("app.name")}</span>
        <span className="flex-1" />
        <label className="flex items-center gap-2 text-xs text-muted">
          {t("app.language")}
          <select
            className="rounded border border-line bg-panel px-1 py-0.5 text-zinc-100"
            value={i18n.language}
            onChange={(e) => setLanguage(e.target.value as (typeof LANGUAGES)[number])}
          >
            {LANGUAGES.map((l) => (
              <option key={l} value={l}>
                {t(`app.languages.${l}`)}
              </option>
            ))}
          </select>
        </label>
      </header>
      <SetupBanner />
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
            <TeamPanel
              key={selected.id}
              workspaceId={selected.id}
              workspaceName={selected.name}
              workspacePath={selected.path}
            />
          ) : (
            <p className="p-4 text-sm text-muted">{t("workspaces.selectHint")}</p>
          )}
        </div>
      </div>
    </main>
  );
}
