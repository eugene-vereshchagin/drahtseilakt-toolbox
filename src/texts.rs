// The window's texts in English, Russian, German and Simplified Chinese, in one table, and the language chosen from
// the system's (LC_ALL, LC_MESSAGES, LANG; English when none is known), which the window can switch.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    En,
    Ru,
    De,
    Zh,
}

pub const LANGS: [(Lang, &str); 4] = [
    (Lang::En, "English"),
    (Lang::Ru, "Русский"),
    (Lang::De, "Deutsch"),
    (Lang::Zh, "简体中文"),
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
    } else {
        Lang::En
    }
}

/// One text in the four languages.
pub struct T {
    en: &'static str,
    ru: &'static str,
    de: &'static str,
    zh: &'static str,
}

impl T {
    pub fn get(&self, lang: Lang) -> &'static str {
        match lang {
            Lang::En => self.en,
            Lang::Ru => self.ru,
            Lang::De => self.de,
            Lang::Zh => self.zh,
        }
    }
}

macro_rules! text {
    ($name:ident, $en:expr, $ru:expr, $de:expr, $zh:expr) => {
        pub const $name: T = T { en: $en, ru: $ru, de: $de, zh: $zh };
    };
}

text!(TITLE, "Drahtseilakt toolbox", "Drahtseilakt: подготовка уровней", "Drahtseilakt-Toolbox", "Drahtseilakt 工具箱");
text!(
    INTRO,
    "Makes remastered levels from your own copy of Ballance: rounder shapes and larger textures.",
    "Делает ремастер-уровни из вашей копии Ballance: более круглые формы и увеличенные текстуры.",
    "Macht Remaster-Level aus deiner Kopie von Ballance: rundere Formen und größere Texturen.",
    "用你自己的 Ballance 拷贝制作重制关卡：更圆的形状和更大的纹理。"
);
text!(
    REMASTER_TITLE,
    "Remaster the original levels",
    "Ремастер оригинальных уровней",
    "Remaster der Original-Level",
    "重制原版关卡"
);
text!(MAPS_TITLE, "Import community maps", "Импорт любительских карт", "Community-Karten importieren", "导入自制地图");
text!(ADVANCED, "Advanced", "Дополнительно", "Erweitert", "高级");
text!(LANGUAGE, "Language", "Язык", "Sprache", "语言");
text!(COPY, "Your copy of the game", "Ваша копия игры", "Deine Kopie des Spiels", "你的游戏拷贝");
text!(
    NO_COPY,
    "Not found: give the Ballance folder (or import it with the game first).",
    "Не найдена: укажите папку Ballance (или сначала импортируйте её игрой).",
    "Nicht gefunden: gib den Ballance-Ordner an (oder importiere ihn zuerst mit dem Spiel).",
    "未找到：请填写 Ballance 文件夹（或先用游戏导入）。"
);
text!(BROWSE, "Browse…", "Обзор…", "Durchsuchen…", "浏览…");
text!(IMAGE, "Image…", "Образ…", "Abbild…", "镜像…");
text!(PICK_COPY, "Choose the game's folder", "Выберите папку с игрой", "Spielordner wählen", "选择游戏文件夹");
text!(PICK_IMAGE, "Choose the CD image (.cue, .bin, .iso)", "Выберите образ диска (.cue, .bin, .iso)", "CD-Abbild wählen (.cue, .bin, .iso)", "选择光盘镜像（.cue、.bin、.iso）");
text!(PICK_MAPS, "Choose the folder of downloaded maps", "Выберите папку со скачанными картами", "Ordner mit heruntergeladenen Karten wählen", "选择已下载地图的文件夹");
text!(USE, "Use", "Выбрать", "Übernehmen", "使用");
text!(OUTPUT, "Levels are made into", "Уровни будут в папке", "Die Level entstehen in", "关卡将生成到");
text!(SCALE, "Textures larger by", "Увеличить текстуры в", "Texturen größer um", "纹理放大倍数");
text!(MODEL, "Upscaler", "Чем увеличивать", "Vergrößern mit", "放大方式");
text!(
    LANCZOS,
    "Lanczos (Real-ESRGAN not found: larger, no new detail)",
    "Lanczos (Real-ESRGAN не найден: крупнее, без новых деталей)",
    "Lanczos (Real-ESRGAN nicht gefunden: größer, ohne neue Details)",
    "Lanczos（未找到 Real-ESRGAN：只放大，不补细节）"
);
text!(RELIEF, "Relief strength", "Сила рельефа", "Reliefstärke", "浮雕强度");
text!(
    RELIEF_WARN,
    "Guessed from the picture, relief deepens every crack and gap. 0 is off; keep it at 1–3.",
    "Рельеф угадывается по картинке и углубляет каждую трещину и щель. 0 — выключен; держите 1–3.",
    "Aus dem Bild geraten, vertieft das Relief jeden Riss und jede Fuge. 0 ist aus; bleib bei 1–3.",
    "浮雕是从图像推测的，会加深每一条裂缝和缝隙。0 为关闭；建议 1–3。"
);
text!(MAKE, "Make", "Сделать", "Erstellen", "开始制作");
text!(CANCEL, "Cancel", "Отменить", "Abbrechen", "取消");
text!(CANCELLING, "Stopping after this texture…", "Останавливаюсь после текущей текстуры…", "Halte nach dieser Textur an…", "将在当前纹理后停止…");
text!(WORKING, "Working on", "Сейчас", "In Arbeit", "正在处理");
text!(DONE, "Done.", "Готово.", "Fertig.", "完成。");
text!(FAILED, "Failed:", "Ошибка:", "Fehler:", "失败：");
text!(RESULT, "Result", "Результат", "Ergebnis", "结果");
text!(PREVIEW, "Last texture: before and after", "Последняя текстура: до и после", "Letzte Textur: vorher und nachher", "最近的纹理：处理前后");
text!(
    PLAY,
    "Play them in the game: Start → Remastered (extended mode).",
    "Играть в игре: Start → Remastered (расширенный режим).",
    "Spielen im Spiel: Start → Remastered (erweiterter Modus).",
    "在游戏中游玩：Start → Remastered（扩展模式）。"
);
text!(
    MAPS,
    "Community maps: an unpacked download, sorted into levels/user-levels, one folder a level",
    "Любительские карты: распакованная загрузка — по папкам в levels/user-levels, папка на уровень",
    "Community-Karten: ein entpackter Download, in levels/user-levels einsortiert, ein Ordner je Level",
    "自制地图：把下载的合集（已解压）整理到游戏的 levels/user-levels，每个关卡一个文件夹"
);
text!(IMPORT, "Import", "Импортировать", "Importieren", "导入");
text!(
    PLAY_MAPS,
    "Play them in the game: Start → Community maps (extended mode).",
    "Играть в игре: Start → Community maps (расширенный режим).",
    "Spielen im Spiel: Start → Community maps (erweiterter Modus).",
    "在游戏中游玩：Start → Community maps（扩展模式）。"
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_locale_picks_the_language() {
        assert_eq!(of("ru_RU.UTF-8"), Lang::Ru);
        assert_eq!(of("de_AT"), Lang::De);
        assert_eq!(of("zh_CN.UTF-8"), Lang::Zh);
        assert_eq!(of("en_US.UTF-8"), Lang::En);
        assert_eq!(of("C"), Lang::En);
        assert_eq!(MAKE.get(Lang::Ru), "Сделать");
    }
}
