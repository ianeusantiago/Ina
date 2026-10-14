import { useTranslation } from "react-i18next";
import { TerminalPane } from "./components/TerminalPane";

export default function App() {
  const { t } = useTranslation();
  return (
    <main className="flex h-full flex-col">
      <header className="border-b border-line px-4 py-2 text-sm font-semibold tracking-wide">
        {t("app.name")}
      </header>
      <div className="min-h-0 flex-1 p-2">
        <TerminalPane title="shell" />
      </div>
    </main>
  );
}
