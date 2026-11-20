// "Take over my progress": the unlocked levels and highscores of the original game's Database.tdb (in the Ballance
// folder the player picked) are added to the game's data folder. The reading, the merge and the files are
// gamedata::progress (shared with the game, which reads the same files); this is the summary line.

use std::path::Path;

/// Takes the progress of the Database.tdb in `game` over into the data folder `dir`; the line saying what came over.
pub fn take(game: &Path, dir: &Path) -> Result<String, String> {
    let (found, t) = gamedata::progress::import_from_game(game, dir)?;
    let lists = found.scores.iter().filter(|l| !l.is_empty()).count();
    Ok(format!(
        "progress: {} of {} unlocked levels new, {} new highscore entries on {} of {} lists (nothing lowered)",
        t.unlocked,
        found.unlocked.len(),
        t.scores,
        t.levels_with_scores,
        lists
    ))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn the_sample_copies_come_over_and_a_second_time_adds_nothing() {
        let refs = Path::new(env!("CARGO_MANIFEST_DIR")).join("../ref");
        let dir = std::env::temp_dir().join(format!("toolbox-progress-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut ran = 0;
        for (copy, new_unlocked) in [("Ballance", 2), ("Ballance-13", 10)] {
            let game = refs.join(copy);
            if !game.join("Database.tdb").is_file() {
                continue;
            }
            ran += 1;
            let line = take(&game, &dir).unwrap();
            assert!(
                line.starts_with(&format!("progress: {new_unlocked} of ")),
                "{line}"
            );
        }
        if ran == 2 {
            // 1 and 2 from the first copy, the other ten from the second
            assert_eq!(gamedata::progress::read_unlocked(&dir).len(), 12);
            let line = take(&refs.join("Ballance-13"), &dir).unwrap();
            assert!(
                line.contains("0 of 12 unlocked levels new, 0 new highscore"),
                "{line}"
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
        assert!(take(&dir, &dir).is_err(), "a missing folder is an error");
    }
}
