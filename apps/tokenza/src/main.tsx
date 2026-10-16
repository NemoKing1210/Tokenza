import { StrictMode, useEffect, useLayoutEffect } from "react";
import { createRoot } from "react-dom/client";
import "./theme.css";
import Flyout from "./Flyout";
import SettingsView from "./Settings";
import { selectLang, useStore } from "./store";
import { t } from "./i18n";

const view = new URLSearchParams(location.search).get("view") ?? "flyout";
document.documentElement.dataset.view = view;

// The web view's browser menu (Back, Reload, Inspect…) has no place in a tray app. Text fields keep
// theirs for copy and paste, and dev builds keep it for the inspector.
if (import.meta.env.PROD) {
  document.addEventListener("contextmenu", (e) => {
    if (!(e.target instanceof Element && e.target.closest("input, textarea, [contenteditable]"))) e.preventDefault();
  });
  // Tauri already leaves the web view's devtools off in release builds (the `devtools` feature is not
  // enabled); this also swallows the shortcuts so they do nothing at all.
  document.addEventListener(
    "keydown",
    (e) => {
      const k = e.key.toLowerCase();
      if (e.key === "F12" || (e.ctrlKey && e.shiftKey && ["i", "j", "c"].includes(k)) || (e.ctrlKey && k === "u")) e.preventDefault();
    },
    true,
  );
}

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
