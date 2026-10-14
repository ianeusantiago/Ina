import { useState, type FormEvent } from "react";
import { useTranslation } from "react-i18next";
import type { Workspace } from "../lib/workspaces";

interface Props {
  workspaces: Workspace[];
  selectedId: number | null;
  error: string | null;
  onSelect: (id: number) => void;
  onAdd: (path: string) => void;
  onRemove: (id: number) => void;
}

export function Sidebar({ workspaces, selectedId, error, onSelect, onAdd, onRemove }: Props) {
  const { t } = useTranslation();
  const [path, setPath] = useState("");

  const submit = (e: FormEvent) => {
    e.preventDefault();
    if (path.trim()) {
      onAdd(path.trim());
      setPath("");
    }
  };

  return (
    <aside className="flex w-64 shrink-0 flex-col border-r border-line bg-panel text-sm">
      <h2 className="px-3 pt-3 pb-2 text-xs font-semibold tracking-wider text-muted uppercase">
        {t("workspaces.title")}
      </h2>
      <ul className="min-h-0 flex-1 overflow-y-auto" aria-label={t("workspaces.title")}>
        {workspaces.length === 0 && (
          <li className="px-3 py-2 text-muted">{t("workspaces.empty")}</li>
        )}
        {workspaces.map((w) => (
          <li key={w.id} className="group flex items-center">
            <button
              className={`min-w-0 flex-1 truncate px-3 py-1.5 text-left hover:bg-line ${
                w.id === selectedId ? "bg-line text-zinc-100" : "text-zinc-300"
              }`}
              title={w.path}
              aria-current={w.id === selectedId}
              onClick={() => onSelect(w.id)}
            >
              {w.name}
            </button>
            <button
              className="px-2 text-muted opacity-0 group-hover:opacity-100 hover:text-zinc-100 focus:opacity-100"
              aria-label={t("workspaces.remove", { name: w.name })}
              title={t("workspaces.remove", { name: w.name })}
              onClick={() => onRemove(w.id)}
            >
              ×
            </button>
          </li>
        ))}
      </ul>
      <form onSubmit={submit} className="border-t border-line p-2">
        <label className="mb-1 block text-xs text-muted" htmlFor="ws-path">
          {t("workspaces.addLabel")}
        </label>
        <div className="flex gap-1">
          <input
            id="ws-path"
            className="min-w-0 flex-1 rounded border border-line bg-ink px-2 py-1 text-xs outline-none focus:border-accent"
            placeholder="C:\projetos\app"
            value={path}
            onChange={(e) => setPath(e.target.value)}
          />
          <button className="rounded border border-line px-2 text-xs hover:bg-line" type="submit">
            {t("workspaces.add")}
          </button>
        </div>
        {error && (
          <p className="mt-1 text-xs text-red-400" role="alert">
            {error}
          </p>
        )}
      </form>
    </aside>
  );
}
