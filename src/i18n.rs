//! Interface languages and localized date names.
//!
//! Strings are looked up by their English text, so anything not yet translated simply
//! shows in English. To add a language: add it to `Lang`, `LANGS`, and a column to `TABLE`.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Lang {
    En = 0,
    ZhHans = 1,
    ZhHant = 2,
    Ja = 3,
    Ko = 4,
    Es = 5,
    Fr = 6,
    De = 7,
}

/// (code stored in the config, native name, language)
pub const LANGS: [(&str, &str, Lang); 8] = [
    ("en", "English", Lang::En),
    ("zh-Hans", "简体中文", Lang::ZhHans),
    ("zh-Hant", "繁體中文", Lang::ZhHant),
    ("ja", "日本語", Lang::Ja),
    ("ko", "한국어", Lang::Ko),
    ("es", "Español", Lang::Es),
    ("fr", "Français", Lang::Fr),
    ("de", "Deutsch", Lang::De),
];

static UI_LANG: AtomicU8 = AtomicU8::new(0);
static DATE_LANG: AtomicU8 = AtomicU8::new(0);

fn from_u8(v: u8) -> Lang {
    LANGS.iter().map(|l| l.2).find(|l| *l as u8 == v).unwrap_or(Lang::En)
}

/// The system's preferred language, mapped to one we support.
pub fn system_lang() -> Lang {
    let tag = sys_locale::get_locale().unwrap_or_default().to_lowercase();
    from_tag(&tag)
}

/// Maps a BCP-47-ish tag ("zh-Hant-TW", "ja_JP", "fr") to a supported language.
pub fn from_tag(tag: &str) -> Lang {
    let tag = tag.to_lowercase().replace('_', "-");
    if tag.starts_with("zh") {
        let traditional = ["hant", "-tw", "-hk", "-mo"].iter().any(|t| tag.contains(t));
        return if traditional { Lang::ZhHant } else { Lang::ZhHans };
    }
    match tag.split('-').next().unwrap_or("") {
        "ja" => Lang::Ja,
        "ko" => Lang::Ko,
        "es" => Lang::Es,
        "fr" => Lang::Fr,
        "de" => Lang::De,
        _ => Lang::En,
    }
}

/// Resolves a config value: "" means "follow the system".
pub fn resolve(code: &str) -> Lang {
    if code.trim().is_empty() {
        return system_lang();
    }
    LANGS.iter().find(|l| l.0.eq_ignore_ascii_case(code)).map(|l| l.2).unwrap_or_else(|| from_tag(code))
}

/// Applies the language settings from the config to this process.
pub fn apply(ui: &str, dates: &str) {
    let ui_lang = resolve(ui);
    let date_lang = if dates.trim().is_empty() { ui_lang } else { resolve(dates) };
    UI_LANG.store(ui_lang as u8, Ordering::Relaxed);
    DATE_LANG.store(date_lang as u8, Ordering::Relaxed);
}

pub fn ui_lang() -> Lang {
    from_u8(UI_LANG.load(Ordering::Relaxed))
}

pub fn date_lang() -> Lang {
    from_u8(DATE_LANG.load(Ordering::Relaxed))
}

/// The chrono locale used for weekday and month names.
pub fn date_locale() -> chrono::Locale {
    match date_lang() {
        Lang::En => chrono::Locale::en_US,
        Lang::ZhHans => chrono::Locale::zh_CN,
        Lang::ZhHant => chrono::Locale::zh_TW,
        Lang::Ja => chrono::Locale::ja_JP,
        Lang::Ko => chrono::Locale::ko_KR,
        Lang::Es => chrono::Locale::es_ES,
        Lang::Fr => chrono::Locale::fr_FR,
        Lang::De => chrono::Locale::de_DE,
    }
}

/// Translates an English interface string into the current language.
pub fn t(english: &'static str) -> &'static str {
    translate(ui_lang(), english)
}

/// Translates an English interface string into `lang`.
pub fn translate(lang: Lang, english: &'static str) -> &'static str {
    if lang == Lang::En {
        return english;
    }
    static MAP: OnceLock<HashMap<&'static str, &'static [&'static str; 7]>> = OnceLock::new();
    let map = MAP.get_or_init(|| TABLE.iter().map(|(k, v)| (*k, v)).collect());
    match map.get(english) {
        Some(row) => row[lang as usize - 1],
        None => english,
    }
}

/// Translates a string containing `{}` placeholders, then fills them in order.
pub fn tf(english: &'static str, args: &[&str]) -> String {
    let mut out = String::new();
    let mut parts = t(english).split("{}");
    if let Some(first) = parts.next() {
        out.push_str(first);
    }
    for (i, part) in parts.enumerate() {
        out.push_str(args.get(i).copied().unwrap_or(""));
        out.push_str(part);
    }
    out
}

/// English → [简体中文, 繁體中文, 日本語, 한국어, Español, Français, Deutsch]
#[rustfmt::skip]
static TABLE: &[(&str, [&str; 7])] = &[
    // ---- Menu ----
    ("Show Clocks & Calendar", ["显示时钟和日历", "顯示時鐘和日曆", "時計とカレンダーを表示", "시계 및 캘린더 보기", "Mostrar relojes y calendario", "Afficher horloges et calendrier", "Uhren & Kalender anzeigen"]),
    ("Copy All Times", ["复制所有时间", "複製所有時間", "すべての時刻をコピー", "모든 시간 복사", "Copiar todas las horas", "Copier toutes les heures", "Alle Zeiten kopieren"]),
    ("Settings…", ["设置…", "設定…", "設定…", "설정…", "Ajustes…", "Réglages…", "Einstellungen…"]),
    ("Show Config File", ["显示配置文件", "顯示設定檔", "設定ファイルを表示", "설정 파일 보기", "Mostrar archivo de configuración", "Afficher le fichier de configuration", "Konfigurationsdatei zeigen"]),
    ("Open at Login", ["登录时打开", "登入時開啟", "ログイン時に開く", "로그인 시 열기", "Abrir al iniciar sesión", "Ouvrir à la connexion", "Beim Anmelden öffnen"]),
    ("Quit TimeStrip", ["退出 TimeStrip", "結束 TimeStrip", "TimeStrip を終了", "TimeStrip 종료", "Salir de TimeStrip", "Quitter TimeStrip", "TimeStrip beenden"]),
    ("No regions yet — add some in Settings", ["还没有地区 — 请在设置中添加", "尚無地區 — 請在設定中新增", "地域がありません — 設定で追加してください", "지역이 없습니다 — 설정에서 추가하세요", "Aún no hay regiones — añádelas en Ajustes", "Aucune région — ajoutez-en dans les Réglages", "Noch keine Regionen — in den Einstellungen hinzufügen"]),
    ("same time", ["时间相同", "時間相同", "同じ時刻", "같은 시간", "misma hora", "même heure", "gleiche Zeit"]),

    // ---- Panel ----
    ("Local", ["本地", "本地", "ローカル", "현지", "Local", "Local", "Lokal"]),
    ("Show this region's calendar", ["显示此地区的日历", "顯示此地區的日曆", "この地域のカレンダーを表示", "이 지역의 캘린더 보기", "Mostrar el calendario de esta región", "Afficher le calendrier de cette région", "Kalender dieser Region anzeigen"]),
    ("Drag to see other times", ["拖动查看其他时间", "拖曳查看其他時間", "ドラッグして別の時刻を表示", "드래그하여 다른 시간 보기", "Arrastra para ver otras horas", "Faites glisser pour voir d'autres heures", "Ziehen, um andere Zeiten zu sehen"]),
    ("{} from now", ["{}后", "{}後", "{}後", "{} 후", "dentro de {}", "dans {}", "in {}"]),
    ("{} ago", ["{}前", "{}前", "{}前", "{} 전", "hace {}", "il y a {}", "vor {}"]),
    ("Now", ["现在", "現在", "現在", "지금", "Ahora", "Maintenant", "Jetzt"]),
    ("Calendar for", ["日历地区", "日曆地區", "カレンダーの地域", "캘린더 지역", "Calendario de", "Calendrier de", "Kalender für"]),
    ("Today", ["今天", "今天", "今日", "오늘", "Hoy", "Aujourd'hui", "Heute"]),
    ("Wk", ["周", "週", "週", "주", "Sem", "Sem", "KW"]),
    ("See your calendar events here, shown in every region's time.", ["在这里查看日历事件，并以每个地区的时间显示。", "在這裡查看日曆事件，並以每個地區的時間顯示。", "カレンダーの予定を各地域の時刻で表示します。", "캘린더 일정을 각 지역 시간으로 여기에 표시합니다.", "Ve aquí tus eventos, con la hora de cada región.", "Voyez ici vos événements, à l'heure de chaque région.", "Sieh hier deine Termine in der Zeit jeder Region."]),
    ("Show my calendar events", ["显示我的日历事件", "顯示我的日曆事件", "カレンダーの予定を表示", "내 캘린더 일정 보기", "Mostrar mis eventos", "Afficher mes événements", "Meine Termine anzeigen"]),
    ("TimeStrip isn't allowed to read your calendars. Turn it on in System Settings → Privacy & Security → Calendars.", ["TimeStrip 无权读取你的日历。请在“系统设置 → 隐私与安全性 → 日历”中开启。", "TimeStrip 無權讀取你的日曆。請在「系統設定 → 隱私權與安全性 → 日曆」中開啟。", "TimeStrip はカレンダーを読み取れません。「システム設定 → プライバシーとセキュリティ → カレンダー」でオンにしてください。", "TimeStrip에 캘린더 접근 권한이 없습니다. 시스템 설정 → 개인정보 보호 및 보안 → 캘린더에서 켜세요.", "TimeStrip no puede leer tus calendarios. Actívalo en Ajustes del Sistema → Privacidad y seguridad → Calendarios.", "TimeStrip n'a pas accès à vos calendriers. Activez-le dans Réglages Système → Confidentialité et sécurité → Calendriers.", "TimeStrip darf deine Kalender nicht lesen. Aktiviere es unter Systemeinstellungen → Datenschutz & Sicherheit → Kalender."]),
    ("Open Privacy Settings", ["打开隐私设置", "打開隱私權設定", "プライバシー設定を開く", "개인정보 보호 설정 열기", "Abrir ajustes de privacidad", "Ouvrir les réglages de confidentialité", "Datenschutzeinstellungen öffnen"]),
    ("No events", ["没有事件", "沒有事件", "予定なし", "일정 없음", "Sin eventos", "Aucun événement", "Keine Termine"]),
    ("All day", ["全天", "全天", "終日", "종일", "Todo el día", "Toute la journée", "Ganztägig"]),
    ("Settings…", ["设置…", "設定…", "設定…", "설정…", "Ajustes…", "Réglages…", "Einstellungen…"]),
    ("Open Calendar", ["打开日历", "打開日曆", "カレンダーを開く", "캘린더 열기", "Abrir Calendario", "Ouvrir Calendrier", "Kalender öffnen"]),
    ("Close", ["关闭", "關閉", "閉じる", "닫기", "Cerrar", "Fermer", "Schließen"]),
    ("Calendar events are only supported on macOS.", ["日历事件仅支持 macOS。", "日曆事件僅支援 macOS。", "カレンダーの予定は macOS のみ対応しています。", "캘린더 일정은 macOS에서만 지원됩니다.", "Los eventos solo funcionan en macOS.", "Les événements ne sont pris en charge que sur macOS.", "Termine werden nur unter macOS unterstützt."]),
    ("Calendar events need the TimeStrip.app bundle (see README → Build it yourself).", ["日历事件需要使用 TimeStrip.app（见 README → 自行构建）。", "日曆事件需要使用 TimeStrip.app（見 README → 自行建置）。", "カレンダーの予定には TimeStrip.app が必要です（README → Build it yourself を参照）。", "캘린더 일정에는 TimeStrip.app이 필요합니다(README → Build it yourself 참조).", "Los eventos requieren TimeStrip.app (ver README → Build it yourself).", "Les événements nécessitent TimeStrip.app (voir README → Build it yourself).", "Termine benötigen TimeStrip.app (siehe README → Build it yourself)."]),

    // ---- Settings: tabs & common ----
    ("TimeStrip Settings", ["TimeStrip 设置", "TimeStrip 設定", "TimeStrip 設定", "TimeStrip 설정", "Ajustes de TimeStrip", "Réglages de TimeStrip", "TimeStrip-Einstellungen"]),
    ("Regions", ["地区", "地區", "地域", "지역", "Regiones", "Régions", "Regionen"]),
    ("Appearance", ["外观", "外觀", "外観", "모양", "Apariencia", "Apparence", "Darstellung"]),
    ("Time & Date", ["时间和日期", "時間與日期", "時刻と日付", "시간 및 날짜", "Hora y fecha", "Heure et date", "Zeit & Datum"]),
    ("Calendar & Panel", ["日历和面板", "日曆與面板", "カレンダーとパネル", "캘린더 및 패널", "Calendario y panel", "Calendrier et panneau", "Kalender & Panel"]),
    ("Language", ["语言", "語言", "言語", "언어", "Idioma", "Langue", "Sprache"]),
    ("General", ["通用", "一般", "一般", "일반", "General", "Général", "Allgemein"]),
    ("Preview", ["预览", "預覽", "プレビュー", "미리보기", "Vista previa", "Aperçu", "Vorschau"]),
    ("The menu bar is about 22 pt tall; anything taller is scaled down to fit.", ["菜单栏约 22 pt 高，更高的内容会被缩小以适应。", "選單列約 22 pt 高，更高的內容會被縮小以容納。", "メニューバーの高さは約 22 pt です。それより大きいものは縮小されます。", "메뉴 막대 높이는 약 22pt이며 더 큰 내용은 축소됩니다.", "La barra de menús mide unos 22 pt; lo más alto se reduce.", "La barre des menus mesure environ 22 pt ; le reste est réduit.", "Die Menüleiste ist etwa 22 pt hoch; Größeres wird verkleinert."]),
    ("No regions are shown in the menu bar. Tick “Menu bar” on the Regions tab.", ["菜单栏中没有显示任何地区。请在“地区”标签中勾选“菜单栏”。", "選單列中沒有顯示任何地區。請在「地區」分頁勾選「選單列」。", "メニューバーに表示中の地域はありません。「地域」タブで「メニューバー」をオンにしてください。", "메뉴 막대에 표시되는 지역이 없습니다. 지역 탭에서 '메뉴 막대'를 체크하세요.", "No hay regiones en la barra de menús. Marca «Barra de menús» en Regiones.", "Aucune région dans la barre des menus. Cochez « Barre des menus » dans Régions.", "Keine Region in der Menüleiste. Aktiviere „Menüleiste“ im Tab Regionen."]),
    ("Couldn't save settings: {}", ["无法保存设置：{}", "無法儲存設定：{}", "設定を保存できませんでした：{}", "설정을 저장할 수 없습니다: {}", "No se pudieron guardar los ajustes: {}", "Impossible d'enregistrer les réglages : {}", "Einstellungen konnten nicht gespeichert werden: {}"]),
    ("Defaults for every region. Individual regions can override these on the Regions tab.", ["所有地区的默认值。可在“地区”标签中为单个地区单独设置。", "所有地區的預設值。可在「地區」分頁為個別地區單獨設定。", "すべての地域の既定値です。「地域」タブで地域ごとに変更できます。", "모든 지역의 기본값입니다. 지역 탭에서 지역별로 바꿀 수 있습니다.", "Valores por defecto para todas las regiones. Cada región puede cambiarlos en Regiones.", "Valeurs par défaut pour toutes les régions. Chaque région peut les modifier dans Régions.", "Standard für alle Regionen. Einzelne Regionen können sie im Tab Regionen ändern."]),

    // ---- Settings: Regions ----
    ("Each region can have its own label, size, font, color and time format. Anything not set here uses the Appearance and Time & Date tabs.", ["每个地区都可以有自己的标签、大小、字体、颜色和时间格式。未设置的项目使用“外观”和“时间和日期”中的设置。", "每個地區都可以有自己的標籤、大小、字體、顏色和時間格式。未設定的項目使用「外觀」和「時間與日期」中的設定。", "地域ごとにラベル、サイズ、フォント、色、時刻形式を設定できます。未設定の項目は「外観」と「時刻と日付」の設定を使います。", "지역마다 레이블, 크기, 글꼴, 색상, 시간 형식을 따로 정할 수 있습니다. 설정하지 않은 항목은 모양 및 시간 및 날짜 탭을 따릅니다.", "Cada región puede tener su etiqueta, tamaño, fuente, color y formato de hora. Lo que no se ajuste aquí usa Apariencia y Hora y fecha.", "Chaque région peut avoir son libellé, sa taille, sa police, sa couleur et son format d'heure. Le reste suit Apparence et Heure et date.", "Jede Region kann eigene Beschriftung, Größe, Schrift, Farbe und Zeitformat haben. Nicht Gesetztes folgt Darstellung und Zeit & Datum."]),
    ("Label", ["标签", "標籤", "ラベル", "레이블", "Etiqueta", "Libellé", "Beschriftung"]),
    ("Menu bar", ["菜单栏", "選單列", "メニューバー", "메뉴 막대", "Barra de menús", "Barre des menus", "Menüleiste"]),
    ("Remove", ["移除", "移除", "削除", "삭제", "Quitar", "Supprimer", "Entfernen"]),
    ("Style for this region (using defaults)", ["此地区的样式（使用默认值）", "此地區的樣式（使用預設值）", "この地域のスタイル（既定値）", "이 지역의 스타일(기본값 사용)", "Estilo de esta región (por defecto)", "Style de cette région (par défaut)", "Stil dieser Region (Standard)"]),
    ("Style for this region (customized)", ["此地区的样式（已自定义）", "此地區的樣式（已自訂）", "この地域のスタイル（カスタム）", "이 지역의 스타일(사용자 지정)", "Estilo de esta región (personalizado)", "Style de cette région (personnalisé)", "Stil dieser Region (angepasst)"]),
    ("Add a region", ["添加地区", "新增地區", "地域を追加", "지역 추가", "Añadir una región", "Ajouter une région", "Region hinzufügen"]),
    ("Search a city or region, e.g. Tokyo, Paris, Kolkata", ["搜索城市或地区，如 Tokyo、Shanghai、Paris", "搜尋城市或地區，如 Tokyo、Taipei、Paris", "都市や地域を検索（例: Tokyo、Paris）", "도시나 지역 검색(예: Seoul, Tokyo, Paris)", "Busca una ciudad o región, p. ej. Tokyo, Madrid", "Cherchez une ville ou région, ex. Tokyo, Paris", "Stadt oder Region suchen, z. B. Tokyo, Berlin"]),
    ("No matching time zones.", ["没有匹配的时区。", "沒有符合的時區。", "一致するタイムゾーンはありません。", "일치하는 시간대가 없습니다.", "No hay zonas horarias que coincidan.", "Aucun fuseau horaire correspondant.", "Keine passende Zeitzone."]),
    ("Tick a setting to override it for this region only.", ["勾选某项设置，仅对此地区生效。", "勾選某項設定，僅對此地區生效。", "チェックした設定はこの地域だけに適用されます。", "체크한 설정은 이 지역에만 적용됩니다.", "Marca un ajuste para cambiarlo solo en esta región.", "Cochez un réglage pour le modifier pour cette région seulement.", "Aktiviere eine Einstellung, um sie nur für diese Region zu ändern."]),
    ("Text size", ["文字大小", "文字大小", "文字サイズ", "글자 크기", "Tamaño del texto", "Taille du texte", "Textgröße"]),
    ("Font", ["字体", "字體", "フォント", "글꼴", "Fuente", "Police", "Schrift"]),
    ("Weight", ["粗细", "粗細", "太さ", "굵기", "Grosor", "Graisse", "Stärke"]),
    ("Color", ["颜色", "顏色", "色", "색상", "Color", "Couleur", "Farbe"]),
    ("show", ["显示", "顯示", "表示", "표시", "mostrar", "afficher", "zeigen"]),
    ("on", ["开", "開", "オン", "켬", "activado", "activé", "an"]),
    ("24-hour", ["24 小时制", "24 小時制", "24 時間表示", "24시간제", "24 horas", "24 heures", "24 Stunden"]),
    ("AM/PM", ["上午/下午", "上午/下午", "午前/午後", "오전/오후", "a. m./p. m.", "AM/PM", "AM/PM"]),
    ("Seconds", ["秒", "秒", "秒", "초", "Segundos", "Secondes", "Sekunden"]),
    ("Time format", ["时间格式", "時間格式", "時刻の形式", "시간 형식", "Formato de hora", "Format d'heure", "Zeitformat"]),
    ("Date", ["日期", "日期", "日付", "날짜", "Fecha", "Date", "Datum"]),
    ("never", ["从不", "從不", "表示しない", "안 함", "nunca", "jamais", "nie"]),
    ("when different", ["日期不同时", "日期不同時", "日付が違うとき", "날짜가 다를 때", "si es otro día", "si autre jour", "wenn anders"]),
    ("always", ["始终", "總是", "常に", "항상", "siempre", "toujours", "immer"]),
    ("Date format", ["日期格式", "日期格式", "日付の形式", "날짜 형식", "Formato de fecha", "Format de date", "Datumsformat"]),
    ("Clear all overrides", ["清除所有单独设置", "清除所有個別設定", "個別設定をすべて解除", "개별 설정 모두 지우기", "Quitar todos los cambios", "Effacer toutes les modifications", "Alle Anpassungen entfernen"]),

    // ---- Settings: Appearance ----
    ("Size", ["大小", "大小", "サイズ", "크기", "Tamaño", "Taille", "Größe"]),
    ("Text size (pt)", ["文字大小 (pt)", "文字大小 (pt)", "文字サイズ (pt)", "글자 크기(pt)", "Tamaño del texto (pt)", "Taille du texte (pt)", "Textgröße (pt)"]),
    ("Label size (× text size)", ["标签大小（× 文字大小）", "標籤大小（× 文字大小）", "ラベルのサイズ（× 文字サイズ）", "레이블 크기(× 글자 크기)", "Tamaño de etiqueta (× texto)", "Taille du libellé (× texte)", "Beschriftungsgröße (× Text)"]),
    ("Space between regions (pt)", ["地区间距 (pt)", "地區間距 (pt)", "地域の間隔 (pt)", "지역 간격(pt)", "Espacio entre regiones (pt)", "Espace entre régions (pt)", "Abstand zwischen Regionen (pt)"]),
    ("Letter spacing (pt)", ["字间距 (pt)", "字距 (pt)", "文字間隔 (pt)", "자간(pt)", "Espaciado entre letras (pt)", "Espacement des lettres (pt)", "Zeichenabstand (pt)"]),
    ("Vertical nudge (pt, + is up)", ["垂直微调（pt，+ 为向上）", "垂直微調（pt，+ 為向上）", "上下位置の微調整（pt、+ で上）", "세로 위치 조정(pt, +는 위로)", "Ajuste vertical (pt, + sube)", "Décalage vertical (pt, + vers le haut)", "Vertikal verschieben (pt, + nach oben)"]),
    ("Layout", ["布局", "版面", "レイアウト", "레이아웃", "Diseño", "Disposition", "Anordnung"]),
    ("Single line", ["单行", "單行", "1 行", "한 줄", "Una línea", "Une ligne", "Einzeilig"]),
    ("Stacked (label above time)", ["堆叠（标签在时间上方）", "堆疊（標籤在時間上方）", "2 段（ラベルを時刻の上に）", "2단(시간 위에 레이블)", "Apilado (etiqueta sobre la hora)", "Empilé (libellé au-dessus)", "Gestapelt (Beschriftung oben)"]),
    ("Filter:", ["筛选：", "篩選：", "絞り込み:", "필터:", "Filtrar:", "Filtrer :", "Filter:"]),
    ("Family", ["字体系列", "字體系列", "ファミリー", "글꼴 패밀리", "Familia", "Famille", "Familie"]),
    ("System font", ["系统字体", "系統字體", "システムフォント", "시스템 글꼴", "Fuente del sistema", "Police système", "Systemschrift"]),
    ("Fixed-width digits (stops the clock from jiggling)", ["等宽数字（防止时钟跳动）", "等寬數字（防止時鐘跳動）", "等幅数字（時計の揺れを防ぐ）", "고정폭 숫자(시계 흔들림 방지)", "Dígitos de ancho fijo (evita que el reloj baile)", "Chiffres à chasse fixe (l'horloge ne bouge plus)", "Ziffern mit fester Breite (kein Wackeln)"]),
    ("Style", ["样式", "樣式", "スタイル", "스타일", "Estilo", "Style", "Stil"]),
    ("Show labels", ["显示标签", "顯示標籤", "ラベルを表示", "레이블 표시", "Mostrar etiquetas", "Afficher les libellés", "Beschriftungen zeigen"]),
    ("Dim labels and dates", ["淡化标签和日期", "淡化標籤和日期", "ラベルと日付を薄く表示", "레이블과 날짜 흐리게", "Atenuar etiquetas y fechas", "Atténuer libellés et dates", "Beschriftungen und Daten abblenden"]),
    ("opacity", ["不透明度", "不透明度", "不透明度", "불투명도", "opacidad", "opacité", "Deckkraft"]),
    ("Separator between regions:", ["地区之间的分隔符：", "地區之間的分隔符：", "地域の区切り:", "지역 구분 기호:", "Separador entre regiones:", "Séparateur entre régions :", "Trennzeichen zwischen Regionen:"]),
    ("Custom color", ["自定义颜色", "自訂顏色", "カスタムカラー", "사용자 지정 색상", "Color personalizado", "Couleur personnalisée", "Eigene Farbe"]),
    ("(follows light/dark menu bar)", ["（跟随浅色/深色菜单栏）", "（跟隨淺色/深色選單列）", "（ライト/ダークに合わせる）", "(밝은/어두운 메뉴 막대에 맞춤)", "(sigue la barra clara/oscura)", "(suit la barre claire/sombre)", "(folgt heller/dunkler Menüleiste)"]),
    ("Reset appearance to defaults", ["恢复默认外观", "恢復預設外觀", "外観を既定に戻す", "모양을 기본값으로 재설정", "Restablecer la apariencia", "Réinitialiser l'apparence", "Darstellung zurücksetzen"]),
    ("Thin", ["极细", "極細", "極細", "가장 얇게", "Fina", "Fine", "Dünn"]),
    ("Extra Light", ["特细", "特細", "極細", "아주 얇게", "Extraligera", "Extra-légère", "Extraleicht"]),
    ("Light", ["细", "細", "細", "얇게", "Ligera", "Légère", "Leicht"]),
    ("Regular", ["常规", "標準", "標準", "보통", "Normal", "Normale", "Normal"]),
    ("Medium", ["中等", "中等", "中", "중간", "Media", "Moyenne", "Mittel"]),
    ("Semibold", ["中粗", "中粗", "中太", "약간 굵게", "Seminegrita", "Demi-grasse", "Halbfett"]),
    ("Bold", ["粗", "粗", "太", "굵게", "Negrita", "Grasse", "Fett"]),
    ("Extra Bold", ["特粗", "特粗", "極太", "아주 굵게", "Extranegrita", "Extra-grasse", "Extrafett"]),
    ("Black", ["最粗", "最粗", "超極太", "가장 굵게", "Negra", "Noire", "Schwarz"]),

    // ---- Settings: Time & Date ----
    ("Time", ["时间", "時間", "時刻", "시간", "Hora", "Heure", "Zeit"]),
    ("24-hour clock", ["24 小时制", "24 小時制", "24 時間表示", "24시간제", "Reloj de 24 horas", "Format 24 heures", "24-Stunden-Format"]),
    ("Show AM/PM", ["显示上午/下午", "顯示上午/下午", "午前/午後を表示", "오전/오후 표시", "Mostrar a. m./p. m.", "Afficher AM/PM", "AM/PM anzeigen"]),
    ("Lowercase am/pm", ["小写 am/pm", "小寫 am/pm", "am/pm を小文字に", "소문자 am/pm", "a. m./p. m. en minúsculas", "am/pm en minuscules", "am/pm kleingeschrieben"]),
    ("Show seconds", ["显示秒", "顯示秒", "秒を表示", "초 표시", "Mostrar segundos", "Afficher les secondes", "Sekunden anzeigen"]),
    ("Custom format:", ["自定义格式：", "自訂格式：", "カスタム形式:", "사용자 지정 형식:", "Formato personalizado:", "Format personnalisé :", "Eigenes Format:"]),
    ("Leave empty to use the options above. %H hour (24h), %-I hour (12h), %M minute, %S second, %p AM/PM.", ["留空则使用上方选项。%H 小时 (24h)、%-I 小时 (12h)、%M 分、%S 秒、%p 上午/下午。", "留空則使用上方選項。%H 小時 (24h)、%-I 小時 (12h)、%M 分、%S 秒、%p 上午/下午。", "空欄なら上の設定を使います。%H 時 (24h)、%-I 時 (12h)、%M 分、%S 秒、%p 午前/午後。", "비워 두면 위 옵션을 사용합니다. %H 시(24h), %-I 시(12h), %M 분, %S 초, %p 오전/오후.", "Déjalo vacío para usar las opciones de arriba. %H hora (24h), %-I hora (12h), %M minuto, %S segundo, %p a. m./p. m.", "Laissez vide pour utiliser les options ci-dessus. %H heure (24h), %-I heure (12h), %M minute, %S seconde, %p AM/PM.", "Leer lassen für die Optionen oben. %H Stunde (24h), %-I Stunde (12h), %M Minute, %S Sekunde, %p AM/PM."]),
    ("Date in the menu bar", ["菜单栏中的日期", "選單列中的日期", "メニューバーの日付", "메뉴 막대의 날짜", "Fecha en la barra de menús", "Date dans la barre des menus", "Datum in der Menüleiste"]),
    ("Never", ["从不", "從不", "表示しない", "안 함", "Nunca", "Jamais", "Nie"]),
    ("Only when that region is on a different day than you", ["仅当该地区与你不在同一天时", "僅當該地區與你不在同一天時", "その地域の日付が自分と違うときだけ", "그 지역 날짜가 나와 다를 때만", "Solo si esa región está en otro día", "Seulement si cette région est un autre jour", "Nur wenn dort ein anderer Tag ist"]),
    ("Always", ["始终", "總是", "常に", "항상", "Siempre", "Toujours", "Immer"]),
    ("or custom:", ["或自定义：", "或自訂：", "またはカスタム:", "또는 사용자 지정:", "o personalizado:", "ou personnalisé :", "oder eigenes:"]),
    ("%a weekday, %b month, %-d day, %-m month number, %Y year.", ["%a 星期，%b 月份，%-d 日，%-m 月份数字，%Y 年。", "%a 星期，%b 月份，%-d 日，%-m 月份數字，%Y 年。", "%a 曜日、%b 月、%-d 日、%-m 月（数字）、%Y 年。", "%a 요일, %b 월, %-d 일, %-m 월(숫자), %Y 연도.", "%a día de la semana, %b mes, %-d día, %-m mes (número), %Y año.", "%a jour, %b mois, %-d jour du mois, %-m mois (nombre), %Y année.", "%a Wochentag, %b Monat, %-d Tag, %-m Monat (Zahl), %Y Jahr."]),
    ("Weekday", ["星期", "星期", "曜日", "요일", "Día de la semana", "Jour", "Wochentag"]),
    ("Weekday + day", ["星期 + 日", "星期 + 日", "曜日 + 日", "요일 + 일", "Día + número", "Jour + date", "Wochentag + Tag"]),
    ("Month + day", ["月 + 日", "月 + 日", "月 + 日", "월 + 일", "Mes + día", "Mois + jour", "Monat + Tag"]),
    ("Weekday, month, day", ["星期、月、日", "星期、月、日", "曜日・月・日", "요일, 월, 일", "Día, mes y número", "Jour, mois, date", "Wochentag, Monat, Tag"]),
    ("Numeric (US)", ["数字（美式）", "數字（美式）", "数字（米国式）", "숫자(미국식)", "Numérico (EE. UU.)", "Numérique (US)", "Numerisch (US)"]),
    ("Numeric (intl)", ["数字（国际）", "數字（國際）", "数字（国際式）", "숫자(국제식)", "Numérico (intl.)", "Numérique (intl.)", "Numerisch (intl.)"]),
    ("Right-click menu", ["右键菜单", "右鍵選單", "右クリックメニュー", "오른쪽 클릭 메뉴", "Menú del clic derecho", "Menu du clic droit", "Rechtsklick-Menü"]),
    ("Show dates", ["显示日期", "顯示日期", "日付を表示", "날짜 표시", "Mostrar fechas", "Afficher les dates", "Daten anzeigen"]),
    ("Show difference from your time", ["显示与你的时差", "顯示與你的時差", "自分との時差を表示", "내 시간과의 차이 표시", "Mostrar diferencia con tu hora", "Afficher le décalage avec votre heure", "Zeitunterschied anzeigen"]),

    // ---- Settings: Calendar & Panel ----
    ("Panel", ["面板", "面板", "パネル", "패널", "Panel", "Panneau", "Panel"]),
    ("The panel opens when you click the clock in the menu bar.", ["点击菜单栏中的时钟即可打开面板。", "點按選單列中的時鐘即可打開面板。", "メニューバーの時計をクリックするとパネルが開きます。", "메뉴 막대의 시계를 클릭하면 패널이 열립니다.", "El panel se abre al hacer clic en el reloj.", "Le panneau s'ouvre quand vous cliquez sur l'horloge.", "Das Panel öffnet sich per Klick auf die Uhr."]),
    ("Clicking the clock opens:", ["点击时钟打开：", "點按時鐘打開：", "時計のクリックで開くもの:", "시계를 클릭하면 열기:", "Al hacer clic en el reloj se abre:", "Un clic sur l'horloge ouvre :", "Klick auf die Uhr öffnet:"]),
    ("Clocks & calendar panel", ["时钟和日历面板", "時鐘與日曆面板", "時計とカレンダーのパネル", "시계 및 캘린더 패널", "Panel de relojes y calendario", "Panneau horloges et calendrier", "Uhren- & Kalender-Panel"]),
    ("Menu", ["菜单", "選單", "メニュー", "메뉴", "Menú", "Menu", "Menü"]),
    ("Right-click always opens the menu (when the panel is chosen above).", ["右键单击始终打开菜单（上方选择面板时）。", "右鍵點按一律打開選單（上方選擇面板時）。", "右クリックでは常にメニューが開きます（上でパネルを選んだ場合）。", "오른쪽 클릭은 항상 메뉴를 엽니다(위에서 패널 선택 시).", "El clic derecho siempre abre el menú (si arriba eliges el panel).", "Le clic droit ouvre toujours le menu (si le panneau est choisi).", "Rechtsklick öffnet immer das Menü (wenn oben Panel gewählt ist)."]),
    ("Panel size (zoom)", ["面板大小（缩放）", "面板大小（縮放）", "パネルの大きさ（拡大）", "패널 크기(확대)", "Tamaño del panel (zoom)", "Taille du panneau (zoom)", "Panelgröße (Zoom)"]),
    ("Panel width (pt)", ["面板宽度 (pt)", "面板寬度 (pt)", "パネルの幅 (pt)", "패널 너비(pt)", "Ancho del panel (pt)", "Largeur du panneau (pt)", "Panelbreite (pt)"]),
    ("Show every region's time", ["显示每个地区的时间", "顯示每個地區的時間", "各地域の時刻を表示", "모든 지역 시간 표시", "Mostrar la hora de cada región", "Afficher l'heure de chaque région", "Zeit jeder Region anzeigen"]),
    ("Show the time slider (see other times across regions)", ["显示时间滑块（查看各地区的其他时间）", "顯示時間滑桿（查看各地區的其他時間）", "時刻スライダーを表示（各地域の別の時刻を確認）", "시간 슬라이더 표시(지역별 다른 시간 보기)", "Mostrar el control deslizante de hora", "Afficher le curseur d'heure", "Zeitregler anzeigen"]),
    ("Show the month calendar and events", ["显示月历和事件", "顯示月曆和事件", "月カレンダーと予定を表示", "월 캘린더 및 일정 표시", "Mostrar calendario mensual y eventos", "Afficher le calendrier mensuel et les événements", "Monatskalender und Termine anzeigen"]),
    ("Month view", ["月视图", "月檢視", "月表示", "월 보기", "Vista mensual", "Vue mensuelle", "Monatsansicht"]),
    ("Week starts on", ["每周开始于", "每週開始於", "週の始まり", "주 시작 요일", "La semana empieza el", "La semaine commence le", "Woche beginnt am"]),
    ("Calendar opens on", ["日历默认地区", "日曆預設地區", "カレンダーの初期地域", "캘린더 기본 지역", "El calendario abre en", "Le calendrier s'ouvre sur", "Kalender öffnet mit"]),
    ("Your time zone", ["你的时区", "你的時區", "自分のタイムゾーン", "내 시간대", "Tu zona horaria", "Votre fuseau horaire", "Deine Zeitzone"]),
    ("Show week numbers", ["显示周数", "顯示週數", "週番号を表示", "주 번호 표시", "Mostrar número de semana", "Afficher les numéros de semaine", "Kalenderwochen anzeigen"]),
    ("Dim weekends", ["淡化周末", "淡化週末", "週末を薄く表示", "주말 흐리게", "Atenuar fines de semana", "Atténuer les week-ends", "Wochenenden abblenden"]),
    ("Show days from the previous and next month", ["显示上月和下月的日期", "顯示上月和下月的日期", "前月と翌月の日付も表示", "이전 및 다음 달 날짜 표시", "Mostrar días del mes anterior y siguiente", "Afficher les jours des mois voisins", "Tage aus Vor- und Folgemonat zeigen"]),
    ("In the panel, click a region's clock (or its name above the calendar) to see that region's dates.", ["在面板中点击某个地区的时钟（或日历上方的名称），即可查看该地区的日期。", "在面板中點按某個地區的時鐘（或日曆上方的名稱），即可查看該地區的日期。", "パネルで地域の時計（またはカレンダー上の名前）をクリックすると、その地域の日付を表示します。", "패널에서 지역 시계(또는 캘린더 위 이름)를 클릭하면 그 지역 날짜를 봅니다.", "En el panel, haz clic en el reloj de una región para ver sus fechas.", "Dans le panneau, cliquez sur l'horloge d'une région pour voir ses dates.", "Klicke im Panel auf die Uhr einer Region, um ihre Daten zu sehen."]),
    ("Events", ["事件", "事件", "予定", "일정", "Eventos", "Événements", "Termine"]),
    ("Show events from the Calendar app", ["显示“日历”App 中的事件", "顯示「行事曆」App 中的事件", "カレンダー App の予定を表示", "캘린더 앱의 일정 표시", "Mostrar eventos de la app Calendario", "Afficher les événements de l'app Calendrier", "Termine aus der Kalender-App anzeigen"]),
    ("Show each event's time in:", ["事件时间显示在：", "事件時間顯示在：", "予定の時刻を表示する地域:", "일정 시간 표시 지역:", "Mostrar la hora de cada evento en:", "Afficher l'heure de chaque événement dans :", "Uhrzeit der Termine zeigen in:"]),
    ("Every region", ["所有地区", "所有地區", "すべての地域", "모든 지역", "Todas las regiones", "Toutes les régions", "Allen Regionen"]),
    ("Regions shown in the menu bar", ["菜单栏中显示的地区", "選單列中顯示的地區", "メニューバーに表示中の地域", "메뉴 막대에 표시된 지역", "Regiones de la barra de menús", "Régions de la barre des menus", "Regionen in der Menüleiste"]),
    ("Only the region the calendar is showing", ["仅日历当前显示的地区", "僅日曆目前顯示的地區", "カレンダーで表示中の地域のみ", "캘린더에 표시 중인 지역만", "Solo la región del calendario", "Seulement la région du calendrier", "Nur die Region des Kalenders"]),
    ("Upcoming days listed under the selected day", ["所选日期下方列出的后续天数", "所選日期下方列出的後續天數", "選択した日の下に表示する日数", "선택한 날 아래에 표시할 일수", "Días siguientes bajo el día elegido", "Jours suivants sous le jour choisi", "Folgetage unter dem gewählten Tag"]),
    ("Calendars to show:", ["要显示的日历：", "要顯示的日曆：", "表示するカレンダー:", "표시할 캘린더:", "Calendarios que mostrar:", "Calendriers à afficher :", "Anzuzeigende Kalender:"]),
    ("No calendars found.", ["未找到日历。", "找不到日曆。", "カレンダーが見つかりません。", "캘린더가 없습니다.", "No se encontraron calendarios.", "Aucun calendrier trouvé.", "Keine Kalender gefunden."]),
    ("Refresh list", ["刷新列表", "重新整理列表", "一覧を更新", "목록 새로 고침", "Actualizar lista", "Actualiser la liste", "Liste aktualisieren"]),
    ("TimeStrip needs permission to read your calendars.", ["TimeStrip 需要读取日历的权限。", "TimeStrip 需要讀取日曆的權限。", "TimeStrip にはカレンダーを読む許可が必要です。", "TimeStrip에 캘린더 읽기 권한이 필요합니다.", "TimeStrip necesita permiso para leer tus calendarios.", "TimeStrip a besoin d'accéder à vos calendriers.", "TimeStrip braucht Zugriff auf deine Kalender."]),
    ("Allow access to calendars…", ["允许访问日历…", "允許取用日曆…", "カレンダーへのアクセスを許可…", "캘린더 접근 허용…", "Permitir acceso a calendarios…", "Autoriser l'accès aux calendriers…", "Zugriff auf Kalender erlauben…"]),
    ("Calendar access is turned off. Turn it on in System Settings → Privacy & Security → Calendars.", ["日历访问已关闭。请在“系统设置 → 隐私与安全性 → 日历”中开启。", "日曆取用已關閉。請在「系統設定 → 隱私權與安全性 → 日曆」中開啟。", "カレンダーへのアクセスはオフです。「システム設定 → プライバシーとセキュリティ → カレンダー」でオンにしてください。", "캘린더 접근이 꺼져 있습니다. 시스템 설정 → 개인정보 보호 및 보안 → 캘린더에서 켜세요.", "El acceso está desactivado. Actívalo en Ajustes del Sistema → Privacidad y seguridad → Calendarios.", "L'accès est désactivé. Activez-le dans Réglages Système → Confidentialité et sécurité → Calendriers.", "Der Zugriff ist aus. Aktiviere ihn unter Systemeinstellungen → Datenschutz & Sicherheit → Kalender."]),
    ("Sunday", ["星期日", "星期日", "日曜日", "일요일", "Domingo", "Dimanche", "Sonntag"]),
    ("Monday", ["星期一", "星期一", "月曜日", "월요일", "Lunes", "Lundi", "Montag"]),
    ("Tuesday", ["星期二", "星期二", "火曜日", "화요일", "Martes", "Mardi", "Dienstag"]),
    ("Wednesday", ["星期三", "星期三", "水曜日", "수요일", "Miércoles", "Mercredi", "Mittwoch"]),
    ("Thursday", ["星期四", "星期四", "木曜日", "목요일", "Jueves", "Jeudi", "Donnerstag"]),
    ("Friday", ["星期五", "星期五", "金曜日", "금요일", "Viernes", "Vendredi", "Freitag"]),
    ("Saturday", ["星期六", "星期六", "土曜日", "토요일", "Sábado", "Samedi", "Samstag"]),

    // ---- Settings: Language ----
    ("Interface language", ["界面语言", "介面語言", "表示言語", "인터페이스 언어", "Idioma de la interfaz", "Langue de l'interface", "Sprache der Oberfläche"]),
    ("Same as system", ["跟随系统", "跟隨系統", "システムと同じ", "시스템과 동일", "Igual que el sistema", "Comme le système", "Wie das System"]),
    ("Language of weekday and month names", ["星期和月份名称的语言", "星期和月份名稱的語言", "曜日と月の名前の言語", "요일 및 월 이름 언어", "Idioma de días y meses", "Langue des jours et des mois", "Sprache für Wochentage und Monate"]),
    ("Same as interface", ["与界面相同", "與介面相同", "表示言語と同じ", "인터페이스와 동일", "Igual que la interfaz", "Comme l'interface", "Wie die Oberfläche"]),
    ("Example:", ["示例：", "範例：", "例:", "예:", "Ejemplo:", "Exemple :", "Beispiel:"]),
    ("Time zones are set per region on the Regions tab.", ["时区在“地区”标签中按地区设置。", "時區在「地區」分頁中按地區設定。", "タイムゾーンは「地域」タブで地域ごとに設定します。", "시간대는 지역 탭에서 지역별로 설정합니다.", "Las zonas horarias se ajustan por región en Regiones.", "Les fuseaux horaires se règlent par région dans Régions.", "Zeitzonen werden pro Region im Tab Regionen festgelegt."]),
    ("Your time zone (detected):", ["你的时区（自动检测）：", "你的時區（自動偵測）：", "自分のタイムゾーン（自動検出）:", "내 시간대(자동 감지):", "Tu zona horaria (detectada):", "Votre fuseau (détecté) :", "Deine Zeitzone (erkannt):"]),
    ("Use a different time zone for “Local”", ["为“本地”使用其他时区", "為「本地」使用其他時區", "「ローカル」に別のタイムゾーンを使う", "'현지'에 다른 시간대 사용", "Usar otra zona horaria para «Local»", "Utiliser un autre fuseau pour « Local »", "Andere Zeitzone für „Lokal“ verwenden"]),

    // ---- Settings: General ----
    ("Open TimeStrip at login", ["登录时打开 TimeStrip", "登入時開啟 TimeStrip", "ログイン時に TimeStrip を開く", "로그인 시 TimeStrip 열기", "Abrir TimeStrip al iniciar sesión", "Ouvrir TimeStrip à la connexion", "TimeStrip beim Anmelden öffnen"]),
    ("Couldn't change login setting: {}", ["无法更改登录设置：{}", "無法變更登入設定：{}", "ログイン設定を変更できませんでした：{}", "로그인 설정을 바꿀 수 없습니다: {}", "No se pudo cambiar el inicio: {}", "Impossible de modifier l'ouverture : {}", "Anmeldeeinstellung nicht geändert: {}"]),
    ("Settings are saved automatically to:", ["设置会自动保存到：", "設定會自動儲存到：", "設定は自動的に次の場所に保存されます:", "설정은 다음 위치에 자동 저장됩니다:", "Los ajustes se guardan automáticamente en:", "Les réglages sont enregistrés automatiquement dans :", "Einstellungen werden automatisch gespeichert in:"]),
    ("Show in Finder", ["在访达中显示", "在 Finder 中顯示", "Finder で表示", "Finder에서 보기", "Mostrar en Finder", "Afficher dans le Finder", "Im Finder zeigen"]),
    ("Tip: hold ⌘ and drag the clock in the menu bar to move it.", ["提示：按住 ⌘ 拖动菜单栏中的时钟可移动它。", "提示：按住 ⌘ 拖曳選單列中的時鐘可移動它。", "ヒント: ⌘ を押しながらドラッグすると時計を移動できます。", "팁: ⌘를 누른 채 시계를 드래그하면 이동할 수 있습니다.", "Consejo: mantén ⌘ y arrastra el reloj para moverlo.", "Astuce : maintenez ⌘ et faites glisser l'horloge pour la déplacer.", "Tipp: ⌘ gedrückt halten und die Uhr ziehen, um sie zu verschieben."]),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_map_to_languages() {
        assert_eq!(from_tag("zh-Hans-CN"), Lang::ZhHans);
        assert_eq!(from_tag("zh_TW"), Lang::ZhHant);
        assert_eq!(from_tag("ja-JP"), Lang::Ja);
        assert_eq!(from_tag("pt-BR"), Lang::En);
        assert_eq!(resolve("de"), Lang::De);
    }

    // These tests avoid `apply`, which changes process-wide state other tests rely on.
    #[test]
    fn translation_and_fallback() {
        assert_eq!(translate(Lang::ZhHans, "Today"), "今天");
        assert_eq!(translate(Lang::De, "Today"), "Heute");
        assert_eq!(translate(Lang::Ja, "Not in the table"), "Not in the table");
        assert_eq!(translate(Lang::En, "Today"), "Today");
        assert_eq!(t("Today"), "Today"); // default language is English
    }

    #[test]
    fn table_has_no_duplicate_keys_with_different_text() {
        let mut seen: HashMap<&str, &[&str; 7]> = HashMap::new();
        for (k, v) in TABLE {
            if let Some(prev) = seen.insert(k, v) {
                assert_eq!(prev, v, "conflicting translations for {k:?}");
            }
        }
    }
}
