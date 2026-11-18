import { useEffect, useState } from "react";
import { inTauri } from "./store";
import { t } from "./i18n";
import type { Lang } from "./format";
import { ExternalLink as Link, Row } from "./controls";

const REPO = "https://github.com/NemoKing1210/Tokenza";
const AUTHOR = { name: "NemoKing", url: "https://github.com/NemoKing1210" };

interface AboutInfo {
  version: string;
  license: string;
}

// Outside Tauri (UI dev in a browser) there is no backend to ask.
const FALLBACK: AboutInfo = { version: "dev", license: "MIT" };

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
