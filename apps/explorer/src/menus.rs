//! What the menu offers for a selection, read from the keys `tumble menu
//! install` writes (see crates/tumble-shell/src/menu.rs), so the Windows 11
//! menu and the classic one always agree. Nothing here decides formats.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    /// Target format id, e.g. `png`.
    pub id: String,
    /// Label, e.g. `PNG`.
    pub label: String,
}

/// Where menu data comes from: the registry in Explorer, a fake in tests.
pub trait Source {
    /// The shared submenu key for an extension (lower case, no dot).
    fn submenu_for(&self, ext: &str) -> Option<String>;
    /// The items under a submenu, in menu order.
    fn items(&self, submenu: &str) -> Vec<Item>;
    /// The tumblew.exe the classic menu runs.
    fn tumblew(&self) -> Option<PathBuf>;
}

fn ext(path: &Path) -> Option<String> {
    path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase())
}

/// Targets every selected file can reach, in the first file's menu order.
/// Empty when any file has no menu (the command then hides itself).
pub fn targets_for(src: &dyn Source, paths: &[PathBuf]) -> Vec<Item> {
    let mut lists = Vec::new();
    for p in paths {
        let Some(sub) = ext(p).and_then(|e| src.submenu_for(&e)) else { return Vec::new() };
        lists.push(src.items(&sub));
    }
    let Some((first, rest)) = lists.split_first() else { return Vec::new() };
    first.iter().filter(|i| rest.iter().all(|l| l.iter().any(|x| x.id == i.id))).cloned().collect()
}

/// Command-line budget per launch; Windows allows 32 767 characters.
const MAX_ARGS_CHARS: usize = 30_000;

/// The `tumblew.exe` launches for converting `paths` to `target`: as few as
/// possible, each under the command-line limit. tumblew's single-instance
/// batching joins them into one job anyway.
pub fn launches(target: &str, paths: &[PathBuf]) -> Vec<Vec<OsString>> {
    let head = || {
        vec![
            OsString::from("convert"),
            OsString::from("--to"),
            OsString::from(target),
            OsString::from("--"),
        ]
    };
    let mut out = Vec::new();
    let mut args = head();
    let mut len = 0;
    for p in paths {
        let n = p.as_os_str().len() + 3;
        if len + n > MAX_ARGS_CHARS && args.len() > 4 {
            out.push(std::mem::replace(&mut args, head()));
            len = 0;
        }
        args.push(p.as_os_str().to_os_string());
        len += n;
    }
    if args.len() > 4 {
        out.push(args);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct Fake(HashMap<&'static str, &'static str>, HashMap<&'static str, Vec<Item>>);

    impl Source for Fake {
        fn submenu_for(&self, ext: &str) -> Option<String> {
            self.0.get(ext).map(|s| s.to_string())
        }
        fn items(&self, submenu: &str) -> Vec<Item> {
            self.1.get(submenu).cloned().unwrap_or_default()
        }
        fn tumblew(&self) -> Option<PathBuf> {
            None
        }
    }

    fn item(id: &str) -> Item {
        Item { id: id.into(), label: id.to_uppercase() }
    }

    fn fake() -> Fake {
        Fake(
            HashMap::from([
                ("jpg", "m/jpeg"),
                ("jpeg", "m/jpeg"),
                ("png", "m/png"),
                ("mp4", "m/mp4"),
            ]),
            HashMap::from([
                ("m/jpeg", vec![item("png"), item("webp"), item("avif")]),
                ("m/png", vec![item("jpeg"), item("webp"), item("avif")]),
                ("m/mp4", vec![item("webm"), item("mp3")]),
            ]),
        )
    }

    #[test]
    fn one_type_gets_its_own_menu() {
        let got = targets_for(&fake(), &[PathBuf::from(r"C:\a\Photo.JPG")]);
        assert_eq!(got, [item("png"), item("webp"), item("avif")]);
    }

    #[test]
    fn mixed_selection_gets_the_common_targets() {
        let paths = [PathBuf::from("a.jpg"), PathBuf::from("b.png")];
        assert_eq!(targets_for(&fake(), &paths), [item("webp"), item("avif")]);
        let paths = [PathBuf::from("a.jpg"), PathBuf::from("c.mp4")];
        assert!(targets_for(&fake(), &paths).is_empty(), "nothing in common");
        let paths = [PathBuf::from("a.jpg"), PathBuf::from("notes.xyz")];
        assert!(targets_for(&fake(), &paths).is_empty(), "an unknown type hides the menu");
        assert!(targets_for(&fake(), &[]).is_empty());
    }

    #[test]
    fn launches_split_long_selections() {
        let few = launches("png", &[PathBuf::from("-a.jpg"), PathBuf::from("b c.jpg")]);
        assert_eq!(few.len(), 1);
        assert_eq!(
            few[0],
            ["convert", "--to", "png", "--", "-a.jpg", "b c.jpg"].map(OsString::from)
        );
        let many: Vec<PathBuf> = (0..2000)
            .map(|i| PathBuf::from(format!(r"C:\Photos\holiday\picture-{i:04}.jpg")))
            .collect();
        let split = launches("webp", &many);
        assert!(split.len() > 1);
        assert_eq!(split.iter().map(|a| a.len() - 4).sum::<usize>(), 2000, "every file once");
        assert!(split.iter().all(|a| a.iter().map(|s| s.len() + 3).sum::<usize>() < 32_767));
    }
}
