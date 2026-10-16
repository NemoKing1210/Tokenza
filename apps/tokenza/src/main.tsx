import { StrictMode, useEffect, useLayoutEffect } from "react";
import { createRoot } from "react-dom/client";
import "./theme.css";
import Flyout from "./Flyout";
import SettingsView from "./Settings";
import { selectLang, useStore } from "./store";
import { t } from "./i18n";

const view = new URLSearchParams(location.search).get("view") ?? "flyout";
document.documentElement.dataset.view = view;

function App() {
  const data = useStore((s) => s.data);
  useEffect(() => void useStore.getState().init(), []);

  // Theme override and language reach the document root so CSS and screen readers follow the settings.
  // A layout effect, so a view transition's "after" snapshot already has the new theme.
  useLayoutEffect(() => {
    if (!data) return;
    const { theme } = data.settings;
    const lang = selectLang(data);
    if (theme === "system") delete document.documentElement.dataset.theme;
    else document.documentElement.dataset.theme = theme;
    document.documentElement.lang = lang;
    if (view === "settings") document.title = t(lang, "settingsTitle");
  }, [data?.settings.theme, data?.settings.lang, data?.resolved_lang]);

  return view === "settings" ? <SettingsView /> : <Flyout />;
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
