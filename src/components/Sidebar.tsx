import { useState, type FormEvent } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { useTranslation } from "react-i18next";
import type { Workspace } from "../lib/workspaces";
import { field, ghostButton, primaryButton } from "./ui";
import { Chevron, WorkspaceTeams, type Focus } from "./SidebarTree";

export type { Focus };

interface Props {
  workspaces: Workspace[];
  selectedId: number | null;
  error: string | null;
  onSelect: (id: number) => void;
  onHome: () => void;
  focus: Focus | null;
  onFocus: (focus: Focus) => void;
  onAdd: (path: string) => void;
  onRemove: (id: number) => void;
}

export function Sidebar({
  workspaces,
  selectedId,
  error,
  onSelect,
  onHome,
  focus,
  onFocus,
  onAdd,
  onRemove,
}: Props) {
  const { t } = useTranslation();
  const [path, setPath] = useState("");
  const [query, setQuery] = useState("");
  const filter = query.trim().toLowerCase();
  const [expanded, setExpanded] = useState<ReadonlySet<number>>(() => new Set());
  const toggle = (id: number) =>
    setExpanded((cur) => {
      const next = new Set(cur);
      if (!next.delete(id)) next.add(id);
      return next;
    });

  const submit = (e: FormEvent) => {
    e.preventDefault();
    if (path.trim()) {
      onAdd(path.trim());
      setPath("");
    }
  };

  return (
    <aside className="flex w-64 shrink-0 flex-col aero-glass m-2 mr-0 rounded-2xl bg-sidebar text-[13px]">
      <div className="px-2.5 pt-2.5">
        <button
          className={`flex w-full items-center gap-2 rounded-md px-2.5 py-1.5 text-left transition ${
            selectedId === null ? "bg-accent font-medium text-white" : "text-fg hover:bg-hover"
          }`}
          aria-current={selectedId === null}
          onClick={onHome}
        >
          <svg aria-hidden viewBox="0 0 16 16" className="size-3.5 shrink-0 opacity-90">
            <path
              fill="currentColor"
              d="M8 1.8 1.5 7.2V14a.8.8 0 0 0 .8.8h3.9V10h3.6v4.8h3.9a.8.8 0 0 0 .8-.8V7.2z"
            />
          </svg>
          {t("home.nav")}
        </button>
      </div>
      <h2 className="px-5 pt-3 pb-1.5 text-[11px] font-semibold text-muted">
        {t("workspaces.title")}
      </h2>
      <div className="px-2.5 pb-1.5">
        <input
          type="search"
          aria-label={t("workspaces.filter")}
          placeholder={t("workspaces.filter")}
          className={`w-full px-2 py-1 text-xs ${field}`}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
      </div>
      <ul className="min-h-0 flex-1 overflow-y-auto px-2.5" aria-label={t("workspaces.title")}>
        {workspaces.length === 0 && (
          <li className="px-2.5 py-2 text-muted">{t("workspaces.empty")}</li>
        )}
        {workspaces.map((w) => {
          // While filtering, every workspace opens so matching teams show; names match too.
          const nameMatches = !filter || w.name.toLowerCase().includes(filter);
          const open = expanded.has(w.id) || (!!filter && !nameMatches);
          const current = w.id === selectedId && !focus;
          return (
            <li key={w.id} className="mb-0.5">
              <div className="group flex items-center rounded-md">
                <button
                  className="flex size-6 shrink-0 items-center justify-center rounded text-muted hover:text-fg"
                  aria-expanded={open}
                  aria-label={t(open ? "workspaces.collapse" : "workspaces.expand", {
                    name: w.name,
                  })}
                  onClick={() => toggle(w.id)}
                >
                  <Chevron open={open} />
                </button>
                <button
                  className={`flex min-w-0 flex-1 items-center gap-2 truncate rounded-md px-2 py-1.5 text-left transition ${
                    current ? "bg-accent font-medium text-white" : "text-fg hover:bg-hover"
                  }`}
                  title={w.path}
                  aria-current={current}
                  onClick={() => {
                    onSelect(w.id);
                    if (!open) toggle(w.id);
                  }}
                >
                  <svg aria-hidden viewBox="0 0 16 16" className="size-3.5 shrink-0 opacity-80">
                    <path
                      fill="currentColor"
                      d="M1.5 4A1.5 1.5 0 0 1 3 2.5h3.1l1.5 1.5H13A1.5 1.5 0 0 1 14.5 5.5v6.5A1.5 1.5 0 0 1 13 13.5H3A1.5 1.5 0 0 1 1.5 12z"
                    />
                  </svg>
                  <span className="truncate">{w.name}</span>
                </button>
                <button
                  className="ml-0.5 rounded-full px-1.5 text-muted opacity-0 transition group-hover:opacity-100 hover:bg-hover hover:text-fg focus:opacity-100"
                  aria-label={t("workspaces.remove", { name: w.name })}
                  title={t("workspaces.remove", { name: w.name })}
                  onClick={() => onRemove(w.id)}
                >
                  ×
                </button>
              </div>
              {open && (
                <WorkspaceTeams
                  workspaceId={w.id}
                  focus={focus}
                  onFocus={onFocus}
                  filter={nameMatches ? "" : filter}
                />
              )}
            </li>
          );
        })}
      </ul>
      <form onSubmit={submit} className="border-t border-line p-3">
        <label className="mb-1.5 block text-[11px] font-medium text-muted" htmlFor="ws-path">
          {t("workspaces.addLabel")}
        </label>
        <div className="flex gap-1">
          <input
            id="ws-path"
            className={`min-w-0 flex-1 px-2 py-1 text-xs ${field}`}
            placeholder="C:\projetos\app"
            value={path}
            onChange={(e) => setPath(e.target.value)}
          />
          <button className={`text-xs ${primaryButton}`} type="submit">
            {t("workspaces.add")}
          </button>
        </div>
        <button
          type="button"
          className={`mt-1.5 w-full text-xs ${ghostButton}`}
          onClick={() =>
            void openDialog({ directory: true, title: t("workspaces.pickTitle") })
              .then((dir) => {
                if (typeof dir === "string") onAdd(dir);
              })
              .catch((e: unknown) => console.error("[dialog]", e))
          }
        >
          {t("workspaces.pick")}
        </button>
        {error && (
          <p className="mt-1.5 text-xs text-danger" role="alert">
            {error}
          </p>
        )}
      </form>
    </aside>
  );
}
