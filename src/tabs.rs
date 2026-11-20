use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub const MIN_WIDTH: f32 = 112.0;
pub const MAX_WIDTH: f32 = 200.0;
pub const HEIGHT: f32 = 30.0;
pub const LAST_SLOT: usize = 8;

pub fn key(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

pub fn label(path: &Path) -> Arc<str> {
    path.file_name().map_or_else(|| path.to_string_lossy(), |f| f.to_string_lossy()).into()
}

pub fn after_close(active: usize, closed: usize, remaining: usize) -> usize {
    match remaining {
        0 => 0,
        _ if active > closed => active - 1,
        _ => active.min(remaining - 1),
    }
}

pub fn step(active: usize, len: usize, forward: bool) -> usize {
    match len {
        0 => 0,
        _ if forward => (active + 1) % len,
        _ => (active + len - 1) % len,
    }
}

pub fn slot(n: usize, len: usize) -> Option<usize> {
    match len {
        0 => None,
        _ if n >= LAST_SLOT => Some(len - 1),
        _ => (n < len).then_some(n),
    }
}

pub fn width(available: f32, count: usize, gap: f32) -> f32 {
    (available / count.max(1) as f32 - gap).clamp(MIN_WIDTH, MAX_WIDTH)
}

pub fn quit_label(picked: usize, total: usize) -> Cow<'static, str> {
    match picked {
        0 => "Exit without saving".into(),
        _ if picked < total => format!("Save {picked} and exit").into(),
        _ if total == 1 => "Save and exit".into(),
        _ => "Save all and exit".into(),
    }
}

pub fn slot_at(x: f32, origin: f32, width: f32, gap: f32, count: usize) -> usize {
    (((x - origin) / (width + gap)).max(0.0) as usize).min(count.saturating_sub(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dragging_picks_the_slot_under_the_pointer() {
        assert_eq!(slot_at(5.0, 0.0, 100.0, 2.0, 4), 0);
        assert_eq!(slot_at(150.0, 0.0, 100.0, 2.0, 4), 1);
        assert_eq!(slot_at(-40.0, 0.0, 100.0, 2.0, 4), 0);
        assert_eq!(slot_at(9000.0, 0.0, 100.0, 2.0, 4), 3);
    }

    #[test]
    fn the_quit_button_says_what_it_will_save() {
        assert_eq!(quit_label(0, 3), "Exit without saving");
        assert_eq!(quit_label(2, 3), "Save 2 and exit");
        assert_eq!(quit_label(3, 3), "Save all and exit");
        assert_eq!(quit_label(1, 1), "Save and exit");
        assert!(matches!(quit_label(3, 3), Cow::Borrowed(_)));
    }

    #[test]
    fn closing_moves_the_active_tab_like_a_browser() {
        assert_eq!(after_close(2, 2, 4), 2);
        assert_eq!(after_close(3, 3, 3), 2);
        assert_eq!(after_close(3, 1, 4), 2);
        assert_eq!(after_close(1, 3, 4), 1);
        assert_eq!(after_close(0, 0, 0), 0);
    }

    #[test]
    fn stepping_wraps_both_ways() {
        assert_eq!(step(0, 3, true), 1);
        assert_eq!(step(2, 3, true), 0);
        assert_eq!(step(0, 3, false), 2);
        assert_eq!(step(0, 0, true), 0);
    }

    #[test]
    fn slots_jump_and_the_last_slot_means_last_tab() {
        assert_eq!(slot(0, 3), Some(0));
        assert_eq!(slot(2, 3), Some(2));
        assert_eq!(slot(4, 3), None);
        assert_eq!(slot(LAST_SLOT, 3), Some(2));
        assert_eq!(slot(LAST_SLOT, 12), Some(11));
        assert_eq!(slot(0, 0), None);
    }

    #[test]
    fn tabs_shrink_to_fit_then_stop_at_the_minimum() {
        assert_eq!(width(1000.0, 2, 2.0), MAX_WIDTH);
        assert_eq!(width(700.0, 5, 2.0), 138.0);
        assert_eq!(width(500.0, 20, 2.0), MIN_WIDTH);
        assert_eq!(width(500.0, 0, 2.0), MAX_WIDTH);
    }

    #[test]
    fn the_same_file_reached_two_ways_gets_one_key() {
        let dir = std::env::temp_dir().join("dmn_viewer_tab_key");
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        let file = dir.join("t.dmn");
        std::fs::write(&file, "x").unwrap();
        assert_eq!(key(&file), key(&dir.join("sub").join("..").join("t.dmn")));
        assert_ne!(key(&file), key(&dir.join("missing.dmn")));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn labels_are_the_file_name() {
        assert_eq!(&*label(Path::new("a/b/c.dmn")), "c.dmn");
        assert_eq!(&*label(Path::new("c.dmn")), "c.dmn");
    }
}
