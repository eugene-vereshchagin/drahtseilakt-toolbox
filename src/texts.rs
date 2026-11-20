// The window's texts in English, Russian, German, Simplified Chinese, French, Spanish and Italian, in one table, and the language chosen from
// the system's (LC_ALL, LC_MESSAGES, LANG; English when none is known), which the window can switch.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    En,
    Ru,
    De,
    Zh,
    Fr,
    Es,
    It,
}

pub const LANGS: [(Lang, &str); 7] = [
    (Lang::En, "English"),
    (Lang::Ru, "Русский"),
    (Lang::De, "Deutsch"),
    (Lang::Zh, "简体中文"),
    (Lang::Fr, "Français"),
    (Lang::Es, "Español"),
    (Lang::It, "Italiano"),
];

/// The language the system asks for, from its locale variables.
pub fn system() -> Lang {
    ["LC_ALL", "LC_MESSAGES", "LANG", "LANGUAGE"]
        .iter()
        .filter_map(|k| std::env::var(k).ok())
        .find(|v| !v.is_empty())
        .map_or(Lang::En, |v| of(&v))
}

fn of(locale: &str) -> Lang {
    let l = locale.to_ascii_lowercase();
    if l.starts_with("ru") {
        Lang::Ru
    } else if l.starts_with("de") {
        Lang::De
    } else if l.starts_with("zh") {
        Lang::Zh
    } else if l.starts_with("fr") {
        Lang::Fr
    } else if l.starts_with("es") {
        Lang::Es
    } else if l.starts_with("it") {
        Lang::It
    } else {
        Lang::En
    }
}

/// One text in the seven languages.
pub struct T {
    en: &'static str,
    ru: &'static str,
    de: &'static str,
    zh: &'static str,
    fr: &'static str,
    es: &'static str,
    it: &'static str,
}

impl T {
    pub fn get(&self, lang: Lang) -> &'static str {
        match lang {
            Lang::En => self.en,
            Lang::Ru => self.ru,
            Lang::De => self.de,
            Lang::Zh => self.zh,
            Lang::Fr => self.fr,
            Lang::Es => self.es,
            Lang::It => self.it,
        }
    }
}

macro_rules! text {
    ($name:ident, $en:expr, $ru:expr, $de:expr, $zh:expr, $fr:expr, $es:expr, $it:expr) => {
        pub const $name: T = T {
            en: $en,
            ru: $ru,
            de: $de,
            zh: $zh,
            fr: $fr,
            es: $es,
            it: $it,
        };
    };
}

text!(
    TITLE,
    "Drahtseilakt toolbox",
    "Drahtseilakt: подготовка уровней",
    "Drahtseilakt-Toolbox",
    "Drahtseilakt 工具箱",
    "Boîte à outils Drahtseilakt",
    "Caja de herramientas de Drahtseilakt",
    "Toolbox di Drahtseilakt"
);
text!(
    INTRO,
    "Makes remastered levels from your own copy of Ballance: rounder shapes and larger textures.",
    "Делает ремастер-уровни из вашей копии Ballance: более круглые формы и увеличенные текстуры.",
    "Macht Remaster-Level aus deiner Kopie von Ballance: rundere Formen und größere Texturen.",
    "用你自己的 Ballance 拷贝制作重制关卡：更圆的形状和更大的纹理。",
    "Crée des niveaux remasterisés à partir de votre propre copie de Ballance : des formes plus rondes et de plus grandes textures.",
    "Crea niveles remasterizados a partir de tu propia copia de Ballance: formas más redondeadas y texturas más grandes.",
    "Crea livelli rimasterizzati dalla tua copia di Ballance: forme più arrotondate e texture più grandi."
);
text!(
    REMASTER_TITLE,
    "Remaster the original levels",
    "Ремастер оригинальных уровней",
    "Remaster der Original-Level",
    "重制原版关卡",
    "Remaster des niveaux originaux",
    "Remasterizar los niveles originales",
    "Rimasterizza i livelli originali"
);
text!(
    MAPS_TITLE,
    "Import community maps",
    "Импорт любительских карт",
    "Community-Karten importieren",
    "导入自制地图",
    "Importer des cartes de la communauté",
    "Importar mapas de la comunidad",
    "Importa mappe della comunità"
);
text!(
    ADVANCED,
    "Advanced",
    "Дополнительно",
    "Erweitert",
    "高级",
    "Avancé",
    "Avanzado",
    "Avanzate"
);
text!(
    LANGUAGE, "Language", "Язык", "Sprache", "语言", "Langue", "Idioma", "Lingua"
);
text!(
    COPY,
    "Your copy of the game",
    "Ваша копия игры",
    "Deine Kopie des Spiels",
    "你的游戏拷贝",
    "Votre copie du jeu",
    "Tu copia del juego",
    "La tua copia del gioco"
);
text!(
    NO_COPY,
    "Not found: give the Ballance folder (or import it with the game first).",
    "Не найдена: укажите папку Ballance (или сначала импортируйте её игрой).",
    "Nicht gefunden: gib den Ballance-Ordner an (oder importiere ihn zuerst mit dem Spiel).",
    "未找到：请填写 Ballance 文件夹（或先用游戏导入）。",
    "Introuvable : indiquez le dossier Ballance (ou importez-le d'abord avec le jeu).",
    "No encontrada: indica la carpeta de Ballance (o impórtala antes con el juego).",
    "Non trovata: indica la cartella di Ballance (o importala prima con il gioco)."
);
text!(
    BROWSE,
    "Folder…",
    "Папка…",
    "Ordner…",
    "文件夹…",
    "Dossier…",
    "Carpeta…",
    "Cartella…"
);
text!(
    IMAGE,
    "Disc image…",
    "Образ диска…",
    "CD-Abbild…",
    "光盘镜像…",
    "Image disque…",
    "Imagen de disco…",
    "Immagine disco…"
);
text!(
    PICK_COPY,
    "Choose the game's folder",
    "Выберите папку с игрой",
    "Spielordner wählen",
    "选择游戏文件夹",
    "Choisir le dossier du jeu",
    "Elige la carpeta del juego",
    "Scegli la cartella del gioco"
);
text!(
    PICK_IMAGE,
    "Choose the CD image (.cue, .bin, .iso)",
    "Выберите образ диска (.cue, .bin, .iso)",
    "CD-Abbild wählen (.cue, .bin, .iso)",
    "选择光盘镜像（.cue、.bin、.iso）",
    "Choisir l'image du CD (.cue, .bin, .iso)",
    "Elige la imagen del CD (.cue, .bin, .iso)",
    "Scegli l'immagine del CD (.cue, .bin, .iso)"
);
text!(
    PICK_MAPS,
    "Choose the folder of downloaded maps",
    "Выберите папку со скачанными картами",
    "Ordner mit heruntergeladenen Karten wählen",
    "选择已下载地图的文件夹",
    "Choisir le dossier des cartes téléchargées",
    "Elige la carpeta de los mapas descargados",
    "Scegli la cartella delle mappe scaricate"
);
text!(
    USE,
    "Use",
    "Выбрать",
    "Übernehmen",
    "使用",
    "Utiliser",
    "Usar",
    "Usa"
);
text!(
    OUTPUT,
    "Levels are made into",
    "Уровни будут в папке",
    "Die Level entstehen in",
    "关卡将生成到",
    "Les niveaux sont créés dans",
    "Los niveles se crean en",
    "I livelli vengono creati in"
);
text!(
    SCALE,
    "Textures larger by",
    "Увеличить текстуры в",
    "Texturen größer um",
    "纹理放大倍数",
    "Textures agrandies de",
    "Texturas más grandes en",
    "Texture ingrandite di"
);
text!(
    MODEL,
    "Upscaler",
    "Чем увеличивать",
    "Vergrößern mit",
    "放大方式",
    "Agrandir avec",
    "Ampliar con",
    "Ingrandisci con"
);
text!(
    LANCZOS,
    "Lanczos (Real-ESRGAN not found: larger, no new detail)",
    "Lanczos (Real-ESRGAN не найден: крупнее, без новых деталей)",
    "Lanczos (Real-ESRGAN nicht gefunden: größer, ohne neue Details)",
    "Lanczos（未找到 Real-ESRGAN：只放大，不补细节）",
    "Lanczos (Real-ESRGAN introuvable : plus grand, sans nouveaux détails)",
    "Lanczos (Real-ESRGAN no encontrado: más grande, sin detalles nuevos)",
    "Lanczos (Real-ESRGAN non trovato: più grande, senza nuovi dettagli)"
);
text!(
    RELIEF,
    "Relief strength",
    "Сила рельефа",
    "Reliefstärke",
    "浮雕强度",
    "Force du relief",
    "Intensidad del relieve",
    "Intensità del rilievo"
);
text!(
    RELIEF_WARN,
    "Guessed from the picture, relief deepens every crack and gap. 0 is off; keep it at 1–3.",
    "Рельеф угадывается по картинке и углубляет каждую трещину и щель. 0 — выключен; держите 1–3.",
    "Aus dem Bild geraten, vertieft das Relief jeden Riss und jede Fuge. 0 ist aus; bleib bei 1–3.",
    "浮雕是从图像推测的，会加深每一条裂缝和缝隙。0 为关闭；建议 1–3。",
    "Deviné d'après l'image, le relief creuse chaque fissure et chaque joint. 0 : désactivé ; restez entre 1 et 3.",
    "Se adivina a partir de la imagen y profundiza cada grieta y cada junta. 0 es desactivado; mantenlo entre 1 y 3.",
    "Dedotto dall'immagine, il rilievo approfondisce ogni crepa e ogni fessura. 0 è disattivato; resta tra 1 e 3."
);
text!(
    MAKE,
    "Make",
    "Сделать",
    "Erstellen",
    "开始制作",
    "Créer",
    "Crear",
    "Crea"
);
text!(
    CANCEL,
    "Cancel",
    "Отменить",
    "Abbrechen",
    "取消",
    "Annuler",
    "Cancelar",
    "Annulla"
);
text!(
    CANCELLING,
    "Stopping after this texture…",
    "Останавливаюсь после текущей текстуры…",
    "Halte nach dieser Textur an…",
    "将在当前纹理后停止…",
    "Arrêt après cette texture…",
    "Se detendrá tras esta textura…",
    "Mi fermo dopo questa texture…"
);
text!(
    WORKING,
    "Working on",
    "Сейчас",
    "In Arbeit",
    "正在处理",
    "En cours",
    "En curso",
    "In corso"
);
text!(
    DONE,
    "Done.",
    "Готово.",
    "Fertig.",
    "完成。",
    "Terminé.",
    "Hecho.",
    "Fatto."
);
text!(
    FAILED,
    "Failed:",
    "Ошибка:",
    "Fehler:",
    "失败：",
    "Échec :",
    "Error:",
    "Errore:"
);
text!(
    RESULT,
    "Result",
    "Результат",
    "Ergebnis",
    "结果",
    "Résultat",
    "Resultado",
    "Risultato"
);
text!(
    PREVIEW,
    "Last texture: before and after",
    "Последняя текстура: до и после",
    "Letzte Textur: vorher und nachher",
    "最近的纹理：处理前后",
    "Dernière texture : avant et après",
    "Última textura: antes y después",
    "Ultima texture: prima e dopo"
);
text!(
    PLAY,
    "Play them in the game: Start → Remastered (extended mode).",
    "Играть в игре: Start → Remastered (расширенный режим).",
    "Spielen im Spiel: Start → Remastered (erweiterter Modus).",
    "在游戏中游玩：Start → Remastered（扩展模式）。",
    "À jouer dans le jeu : Start → Remastered (mode étendu).",
    "Para jugar en el juego: Start → Remastered (modo ampliado).",
    "Da giocare nel gioco: Start → Remastered (modalità estesa)."
);
text!(
    MAPS,
    "Community maps: an unpacked download, sorted into levels/user-levels, one folder a level",
    "Любительские карты: распакованная загрузка — по папкам в levels/user-levels, папка на уровень",
    "Community-Karten: ein entpackter Download, in levels/user-levels einsortiert, ein Ordner je Level",
    "自制地图：把下载的合集（已解压）整理到游戏的 levels/user-levels，每个关卡一个文件夹",
    "Cartes de la communauté : un téléchargement décompressé, classé dans levels/user-levels, un dossier par niveau",
    "Mapas de la comunidad: una descarga descomprimida, ordenada en levels/user-levels, una carpeta por nivel",
    "Mappe della comunità: un download decompresso, ordinato in levels/user-levels, una cartella per livello"
);
text!(
    IMPORT,
    "Import",
    "Импортировать",
    "Importieren",
    "导入",
    "Importer",
    "Importar",
    "Importa"
);
text!(
    PLAY_MAPS,
    "Play them in the game: Start → Community maps (extended mode).",
    "Играть в игре: Start → Community maps (расширенный режим).",
    "Spielen im Spiel: Start → Community maps (erweiterter Modus).",
    "在游戏中游玩：Start → Community maps（扩展模式）。",
    "À jouer dans le jeu : Start → Community maps (mode étendu).",
    "Para jugar en el juego: Start → Community maps (modo ampliado).",
    "Da giocare nel gioco: Start → Community maps (modalità estesa)."
);

text!(
    TAKE_PROGRESS,
    "Take over my progress (unlocked levels and highscores from the copy's Database.tdb)",
    "Перенести мой прогресс (открытые уровни и рекорды из Database.tdb копии)",
    "Meinen Fortschritt übernehmen (freigeschaltete Level und Highscores aus der Database.tdb der Kopie)",
    "沿用我的进度（从副本的 Database.tdb 读取已解锁关卡和最高分）",
    "Reprendre ma progression (niveaux débloqués et meilleurs scores de la Database.tdb de la copie)",
    "Traspasar mi progreso (niveles desbloqueados y récords de la Database.tdb de la copia)",
    "Riporta i miei progressi (livelli sbloccati e record dal Database.tdb della copia)"
);
text!(
    TAKE_PROGRESS_HINT,
    "Only adds: nothing you have in Drahtseilakt is lowered or removed.",
    "Только добавляет: ничего из того, что у вас уже есть в Drahtseilakt, не уменьшается и не удаляется.",
    "Fügt nur hinzu: nichts, was du in Drahtseilakt schon hast, wird gesenkt oder entfernt.",
    "只增不减：你在 Drahtseilakt 中已有的内容不会被降低或删除。",
    "N'ajoute que : rien de ce que vous avez déjà dans Drahtseilakt n'est réduit ni supprimé.",
    "Solo añade: nada de lo que ya tienes en Drahtseilakt se reduce ni se borra.",
    "Aggiunge soltanto: nulla di ciò che hai già in Drahtseilakt viene ridotto o rimosso."
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_locale_picks_the_language() {
        assert_eq!(of("ru_RU.UTF-8"), Lang::Ru);
        assert_eq!(of("de_AT"), Lang::De);
        assert_eq!(of("zh_CN.UTF-8"), Lang::Zh);
        assert_eq!(of("fr_FR.UTF-8"), Lang::Fr);
        assert_eq!(of("es_MX"), Lang::Es);
        assert_eq!(of("it_IT.UTF-8"), Lang::It);
        assert_eq!(of("en_US.UTF-8"), Lang::En);
        assert_eq!(of("C"), Lang::En);
        assert_eq!(MAKE.get(Lang::Ru), "Сделать");
    }
}
