import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { SetupBanner } from "./components/SetupBanner";
import { Sidebar } from "./components/Sidebar";
import { setLanguage } from "./i18n";
import { TeamPanel } from "./components/TeamPanel";
import inaIcon from "./assets/ina-icon.svg";
import { TitleBar } from "./components/TitleBar";
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
    <main className="flex h-full flex-col overflow-hidden bg-ink">
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
        <div className="min-h-0 min-w-0 flex-1 p-3">
          {selected ? (
            <TeamPanel
              key={selected.id}
              workspaceId={selected.id}
              workspaceName={selected.name}
              workspacePath={selected.path}
            />
          ) : (
            <div className="flex h-full flex-col items-center justify-center gap-3 text-center">
              <img src={inaIcon} alt="" className="size-20 drop-shadow-lg" />
              <p className="text-[22px] font-semibold tracking-tight text-fg">{t("app.name")}</p>
              <p className="max-w-xs text-[13px] text-muted">{t("workspaces.selectHint")}</p>
            </div>
          )}
        </div>
      </div>
    </main>
  );
}
