import { useEffect, useState } from "react";
import { inTauri } from "./store";
import { t } from "./i18n";
import type { Lang } from "./format";
import { Row } from "./controls";

const REPO = "https://github.com/NemoKing1210/Tokenza";
const AUTHOR = { name: "NemoKing", url: "https://github.com/NemoKing1210" };

interface AboutInfo {
  version: string;
  license: string;
}

// Outside Tauri (UI dev in a browser) there is no backend to ask.
const FALLBACK: AboutInfo = { version: "dev", license: "MIT" };

async function openLink(url: string) {
  if (!inTauri) {
    window.open(url, "_blank", "noopener");
    return;
  }
  const { invoke } = await import("@tauri-apps/api/core");
  await invoke("open_url", { url }).catch(() => {});
}

function Link({ url, children }: { url: string; children: string }) {
  return (
    <button className="link-btn" onClick={() => void openLink(url)} title={url}>
      {children}
    </button>
  );
}

export default function AboutPane({ lang, quit }: { lang: Lang; quit: () => void }) {
  const [info, setInfo] = useState<AboutInfo>(FALLBACK);
  useEffect(() => {
    if (!inTauri) return;
    void import("@tauri-apps/api/core")
      .then(({ invoke }) => invoke<AboutInfo>("about_info"))
      .then(setInfo)
      .catch(() => {});
  }, []);

  return (
    <>
      <p className="lead">{t(lang, "aboutText")}</p>
      <Row title={t(lang, "aboutVersion")}>
        <span>{info.version}</span>
      </Row>
      <Row title={t(lang, "aboutAuthor")}>
        <Link url={AUTHOR.url}>{AUTHOR.name}</Link>
      </Row>
      <Row title={t(lang, "aboutRepo")}>
        <Link url={REPO}>GitHub</Link>
      </Row>
      <Row title={t(lang, "aboutLicense")}>
        <Link url={`${REPO}/blob/main/LICENSE`}>{info.license}</Link>
      </Row>
      <Row title={t(lang, "aboutChangelog")}>
        <Link url={`${REPO}/blob/main/CHANGELOG.md`}>CHANGELOG.md</Link>
      </Row>
      <Row title={t(lang, "aboutIssues")} hint={t(lang, "aboutIssuesHint")}>
        <Link url={`${REPO}/issues`}>{t(lang, "aboutIssuesBtn")}</Link>
      </Row>
      <Row title={t(lang, "quit")}>
        <button className="btn" onClick={quit}>
          {t(lang, "quit")}
        </button>
      </Row>
    </>
  );
}
