//! Backend strings (tray menu, toasts, window title) and system-language detection.
//! The UI has its own, larger dictionaries in `src/locales`.

use tokenza_core::WindowKind;

pub const SUPPORTED: [&str; 7] = ["en", "ru", "es", "zh", "pt", "de", "fr"];
pub const FALLBACK: &str = "en";

/// First supported language among the preferred ones, else English.
/// Only the primary subtag counts, so `pt-PT` and `zh-Hant-TW` map to Portuguese and Chinese.
pub fn detect<I: IntoIterator<Item = S>, S: AsRef<str>>(preferred: I) -> &'static str {
    for tag in preferred {
        let primary = tag.as_ref().to_lowercase();
        let primary = primary.split(['-', '_']).next().unwrap_or("");
        if let Some(l) = SUPPORTED.iter().find(|l| **l == primary) {
            return l;
        }
    }
    FALLBACK
}

pub fn system_lang() -> &'static str {
    detect(sys_locale::get_locales())
}

pub struct Strings {
    pub open: &'static str,
    pub refresh: &'static str,
    pub settings: &'static str,
    pub quit: &'static str,
    pub settings_title: &'static str,
    /// {0} provider, {1} percent used
    pub used_title: &'static str,
    /// {0} window, {1} percent left
    pub used_body: &'static str,
    /// {0} provider
    pub reset_title: &'static str,
    /// {0} window
    pub reset_body: &'static str,
    pub test_title: &'static str,
    /// {0} warning threshold, percent
    pub test_body: &'static str,
    pub win_session: &'static str,
    pub win_weekly: &'static str,
    pub win_monthly: &'static str,
    pub win_credits: &'static str,
}

const EN: Strings = Strings {
    open: "Open",
    refresh: "Refresh",
    settings: "Settings",
    quit: "Quit",
    settings_title: "Tokenza — Settings",
    used_title: "{0}: {1}% used",
    used_body: "Window \"{0}\" — {1}% left.",
    reset_title: "{0}: limit available again",
    reset_body: "Window \"{0}\" has reset.",
    test_title: "Tokenza: test notification",
    test_body: "This is how a warning looks when usage reaches {0}%.",
    win_session: "5 h",
    win_weekly: "Week",
    win_monthly: "Included usage",
    win_credits: "Credits",
};

const RU: Strings = Strings {
    open: "Открыть",
    refresh: "Обновить",
    settings: "Настройки",
    quit: "Выход",
    settings_title: "Tokenza — Настройки",
    used_title: "{0}: израсходовано {1}%",
    used_body: "Окно «{0}» — осталось {1}%.",
    reset_title: "{0}: лимит снова доступен",
    reset_body: "Окно «{0}» сброшено.",
    test_title: "Tokenza: тестовое уведомление",
    test_body: "Так выглядит предупреждение, когда расход достигает {0}%.",
    win_session: "5 ч",
    win_weekly: "Неделя",
    win_monthly: "Включённый объём",
    win_credits: "Кредиты",
};

const ES: Strings = Strings {
    open: "Abrir",
    refresh: "Actualizar",
    settings: "Ajustes",
    quit: "Salir",
    settings_title: "Tokenza — Ajustes",
    used_title: "{0}: {1}% usado",
    used_body: "Ventana «{0}»: queda el {1}%.",
    reset_title: "{0}: límite disponible de nuevo",
    reset_body: "La ventana «{0}» se ha reiniciado.",
    test_title: "Tokenza: notificación de prueba",
    test_body: "Así se ve un aviso cuando el uso alcanza el {0}%.",
    win_session: "5 h",
    win_weekly: "Semana",
    win_monthly: "Uso incluido",
    win_credits: "Créditos",
};

const ZH: Strings = Strings {
    open: "打开",
    refresh: "刷新",
    settings: "设置",
    quit: "退出",
    settings_title: "Tokenza — 设置",
    used_title: "{0}：已用 {1}%",
    used_body: "“{0}”窗口剩余 {1}%。",
    reset_title: "{0}：额度重新可用",
    reset_body: "“{0}”窗口已重置。",
    test_title: "Tokenza：测试通知",
    test_body: "用量达到 {0}% 时，预警通知就是这样显示的。",
    win_session: "5 小时",
    win_weekly: "每周",
    win_monthly: "包含用量",
    win_credits: "额度",
};

const PT: Strings = Strings {
    open: "Abrir",
    refresh: "Atualizar",
    settings: "Configurações",
    quit: "Sair",
    settings_title: "Tokenza — Configurações",
    used_title: "{0}: {1}% usado",
    used_body: "Janela “{0}”: restam {1}%.",
    reset_title: "{0}: limite disponível novamente",
    reset_body: "A janela “{0}” foi reiniciada.",
    test_title: "Tokenza: notificação de teste",
    test_body: "É assim que um aviso aparece quando o uso chega a {0}%.",
    win_session: "5 h",
    win_weekly: "Semana",
    win_monthly: "Uso incluído",
    win_credits: "Créditos",
};

const DE: Strings = Strings {
    open: "Öffnen",
    refresh: "Aktualisieren",
    settings: "Einstellungen",
    quit: "Beenden",
    settings_title: "Tokenza — Einstellungen",
    used_title: "{0}: {1} % verbraucht",
    used_body: "Zeitfenster „{0}“ – {1} % übrig.",
    reset_title: "{0}: Limit wieder verfügbar",
    reset_body: "Zeitfenster „{0}“ wurde zurückgesetzt.",
    test_title: "Tokenza: Testbenachrichtigung",
    test_body: "So sieht eine Warnung aus, wenn die Nutzung {0} % erreicht.",
    win_session: "5 Std.",
    win_weekly: "Woche",
    win_monthly: "Inklusive Nutzung",
    win_credits: "Guthaben",
};

const FR: Strings = Strings {
    open: "Ouvrir",
    refresh: "Actualiser",
    settings: "Paramètres",
    quit: "Quitter",
    settings_title: "Tokenza — Paramètres",
    used_title: "{0} : {1} % utilisé",
    used_body: "Fenêtre « {0} » : il reste {1} %.",
    reset_title: "{0} : limite de nouveau disponible",
    reset_body: "La fenêtre « {0} » a été réinitialisée.",
    test_title: "Tokenza : notification de test",
    test_body: "Voici à quoi ressemble un avertissement quand l’usage atteint {0} %.",
    win_session: "5 h",
    win_weekly: "Semaine",
    win_monthly: "Utilisation incluse",
    win_credits: "Crédits",
};

pub fn strings(lang: &str) -> &'static Strings {
    match lang {
        "ru" => &RU,
        "es" => &ES,
        "zh" => &ZH,
        "pt" => &PT,
        "de" => &DE,
        "fr" => &FR,
        _ => &EN,
    }
}

/// Replaces `{0}`, `{1}`… in a template.
pub fn fill(template: &str, args: &[&str]) -> String {
    let mut out = template.to_string();
    for (i, a) in args.iter().enumerate() {
        out = out.replace(&format!("{{{i}}}"), a);
    }
    out
}

/// Localized window name; a model suffix after " · " (for example "Week · Opus") is kept.
pub fn window_label(lang: &str, kind: WindowKind, label: &str) -> String {
    let s = strings(lang);
    let suffix = label
        .split_once(" · ")
        .map(|(_, rest)| format!(" · {rest}"))
        .unwrap_or_default();
    match kind {
        WindowKind::Session5h => s.win_session.to_string(),
        WindowKind::Weekly => format!("{}{suffix}", s.win_weekly),
        WindowKind::Monthly => s.win_monthly.to_string(),
        WindowKind::Credits => s.win_credits.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_first_supported_preferred_language() {
        assert_eq!(detect(["ru-RU", "en-US"]), "ru");
        assert_eq!(
            detect(["ja-JP", "de-DE", "en-US"]),
            "de",
            "skips unsupported ones"
        );
        assert_eq!(detect(["pt-PT"]), "pt");
        assert_eq!(detect(["zh-Hant-TW"]), "zh");
        assert_eq!(detect(["es_419"]), "es");
        assert_eq!(detect(["FR-ca"]), "fr");
    }

    #[test]
    fn falls_back_to_english() {
        assert_eq!(detect(["ja-JP", "ko-KR"]), "en");
        assert_eq!(detect(Vec::<String>::new()), "en");
        assert_eq!(strings("xx").open, "Open");
    }

    #[test]
    fn every_language_fills_its_placeholders() {
        for l in SUPPORTED {
            let s = strings(l);
            assert!(
                s.used_title.contains("{0}") && s.used_title.contains("{1}"),
                "{l}"
            );
            assert!(
                s.used_body.contains("{0}") && s.used_body.contains("{1}"),
                "{l}"
            );
            assert!(
                s.reset_title.contains("{0}") && s.reset_body.contains("{0}"),
                "{l}"
            );
            assert!(s.test_body.contains("{0}"), "{l}");
        }
        assert_eq!(fill("{0}: {1}%", &["Codex", "82"]), "Codex: 82%");
    }

    #[test]
    fn window_label_keeps_model_suffix() {
        assert_eq!(
            window_label("ru", WindowKind::Weekly, "Week · Opus"),
            "Неделя · Opus"
        );
        assert_eq!(window_label("en", WindowKind::Session5h, "5 h"), "5 h");
    }
}
