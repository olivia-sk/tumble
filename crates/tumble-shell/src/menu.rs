//! The Explorer right-click menu (PRD section 11), registered under
//! `HKCU\Software\Classes` only, so no admin rights are needed.
//!
//! ```text
//! SystemFileAssociations\.jpg\shell\Tumble        "Convert to", icon,
//!     ExtendedSubCommandsKey = Tumble\menus\jpeg, MultiSelectModel = Player
//! Tumble\menus\jpeg\shell\01-png                  "PNG"
//! Tumble\menus\jpeg\shell\01-png\command          "...\tumblew.exe" convert --to png "%1"
//! Tumble                                          install record
//! AppUserModelId\Tumble.Converter                 name and icon for toasts
//! ```
//!
//! Submenus are shared per input format (`.jpg`, `.jpeg` and `.jfif` all
//! point at `menus\jpeg`), so a JPEG's menu never offers JPEG. `install`
//! records which parent keys it had to create, and `uninstall` removes
//! exactly those plus Tumble's own keys.

use crate::registry::{self, Key};
use std::io;
use std::path::{Path, PathBuf};
use tumble_core::format::FORMATS;
use tumble_core::{FormatId, brand};

/// The real classes root. Tests pass a sandbox instead.
pub const CLASSES: &str = r"Software\Classes";

pub use crate::FormatMenu;

/// One key to create (relative to the classes root) and its string values;
/// a `None` name is the key's default value.
#[derive(Debug, PartialEq)]
pub struct KeyPlan {
    pub path: String,
    pub values: Vec<(Option<&'static str>, String)>,
}

fn verb_key(ext: &str) -> String {
    format!(r"SystemFileAssociations\.{ext}\shell\{}", brand::MENU_VERB)
}

fn submenu_key(format: FormatId) -> String {
    format!(r"{}\menus\{}", brand::REGISTRY_KEY, format)
}

fn aumid_key() -> String {
    format!(r"AppUserModelId\{}", brand::AUMID)
}

/// Every key the menu needs, in creation order.
pub fn plan(tumblew: &Path, icon_file: Option<&Path>, menus: &[FormatMenu]) -> Vec<KeyPlan> {
    let exe = tumblew.display().to_string();
    let mut keys = Vec::new();
    for menu in menus.iter().filter(|m| !m.targets.is_empty()) {
        let sub = submenu_key(menu.format.id);
        for (i, target) in menu.targets.iter().enumerate() {
            let item = format!(r"{sub}\shell\{:02}-{}", i + 1, target);
            keys.push(KeyPlan {
                path: item.clone(),
                values: vec![(Some("MUIVerb"), target.format().name.to_string())],
            });
            keys.push(KeyPlan {
                path: format!(r"{item}\command"),
                values: vec![(None, format!("\"{exe}\" convert --to {target} \"%1\""))],
            });
        }
        for ext in menu.format.extensions {
            keys.push(KeyPlan {
                path: verb_key(ext),
                values: vec![
                    (Some("MUIVerb"), "Convert to".to_string()),
                    (Some("Icon"), format!("\"{exe}\",0")),
                    (Some("ExtendedSubCommandsKey"), sub.clone()),
                    (Some("MultiSelectModel"), "Player".to_string()),
                ],
            });
        }
    }
    let mut aumid = vec![(Some("DisplayName"), brand::APP_NAME.to_string())];
    if let Some(icon) = icon_file {
        aumid.push((Some("IconUri"), icon.display().to_string()));
    }
    keys.push(KeyPlan { path: aumid_key(), values: aumid });
    keys
}

/// Ancestors of `path` (shallowest first), e.g. `a`, `a\b` for `a\b\c`.
fn ancestors(path: &str) -> Vec<String> {
    let parts: Vec<&str> = path.split('\\').collect();
    (1..parts.len()).map(|n| parts[..n].join("\\")).collect()
}

/// What `install` did.
#[derive(Debug)]
pub struct Installed {
    pub extensions: usize,
    pub keys: usize,
}

/// Writes the menu under `root` (normally `CLASSES`). Removes any earlier
/// install first, so running it again is safe.
pub fn install(
    root: &str,
    tumblew: &Path,
    icon_file: Option<&Path>,
    menus: &[FormatMenu],
) -> io::Result<Installed> {
    uninstall(root)?;
    let keys = plan(tumblew, icon_file, menus);
    let full = |p: &str| format!(r"{root}\{p}");

    // Parent keys outside Tumble's own tree that do not exist yet; uninstall
    // removes them again if they are still empty.
    let mut created_parents: Vec<String> = Vec::new();
    for k in &keys {
        if k.path.starts_with(brand::REGISTRY_KEY) {
            continue;
        }
        for a in ancestors(&k.path) {
            if !registry::exists(&full(&a)) && !created_parents.contains(&a) {
                created_parents.push(a);
            }
        }
    }

    for k in &keys {
        let (key, _) = Key::create(&full(&k.path))?;
        for (name, value) in &k.values {
            key.set(*name, value)?;
        }
    }
    let (record, _) = Key::create(&full(brand::REGISTRY_KEY))?;
    record.set(Some("Installed"), env!("CARGO_PKG_VERSION"))?;
    record.set(Some("Tumblew"), &tumblew.display().to_string())?;
    record.set_multi("CreatedKeys", &created_parents)?;

    notify_explorer();
    let extensions = keys.iter().filter(|k| k.path.starts_with("SystemFileAssociations")).count();
    Ok(Installed { extensions, keys: keys.len() + 1 })
}

/// Removes every key Tumble created under `root`, and nothing else.
pub fn uninstall(root: &str) -> io::Result<()> {
    let full = |p: &str| format!(r"{root}\{p}");
    let created = Key::open(&full(brand::REGISTRY_KEY))?
        .map(|k| k.get(Some("CreatedKeys")))
        .transpose()?
        .flatten()
        .unwrap_or_default();

    // Every extension Tumble knows, not just the ones installed last time,
    // so a stale install from an older build is cleaned up too.
    for f in FORMATS {
        for ext in f.extensions {
            registry::delete_tree(&full(&verb_key(ext)))?;
        }
    }
    registry::delete_tree(&full(brand::REGISTRY_KEY))?;
    registry::delete_tree(&full(&aumid_key()))?;

    let mut created = created;
    created.sort_by_key(|p| std::cmp::Reverse(p.matches('\\').count()));
    for p in created {
        registry::delete_empty(&full(&p))?;
    }
    notify_explorer();
    Ok(())
}

/// One registered extension, for `tumble menu status`.
pub struct Registered {
    pub extension: String,
    pub submenu: String,
}

pub struct Status {
    pub registered: Vec<Registered>,
    /// The tumblew.exe the menu runs, and whether it still exists.
    pub tumblew: Option<(PathBuf, bool)>,
}

pub fn status(root: &str) -> io::Result<Status> {
    let full = |p: &str| format!(r"{root}\{p}");
    let mut registered = Vec::new();
    for f in FORMATS {
        for ext in f.extensions {
            if let Some(k) = Key::open(&full(&verb_key(ext)))? {
                let submenu =
                    k.get(Some("ExtendedSubCommandsKey"))?.and_then(|v| v.into_iter().next());
                registered.push(Registered {
                    extension: ext.to_string(),
                    submenu: submenu.unwrap_or_default(),
                });
            }
        }
    }
    let tumblew = Key::open(&full(brand::REGISTRY_KEY))?
        .map(|k| k.get(Some("Tumblew")))
        .transpose()?
        .flatten()
        .and_then(|v| v.into_iter().next())
        .map(|p| {
            let path = PathBuf::from(p);
            let exists = path.is_file();
            (path, exists)
        });
    Ok(Status { registered, tumblew })
}

/// Tells Explorer file associations changed, so menus update at once.
fn notify_explorer() {
    use windows::Win32::UI::Shell::{SHCNE_ASSOCCHANGED, SHCNF_IDLIST, SHChangeNotify};
    // SAFETY: documented no-argument notification.
    unsafe { SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None) };
}

#[cfg(test)]
mod tests {
    use super::*;
    use tumble_core::Format;

    fn menus() -> Vec<FormatMenu> {
        vec![
            FormatMenu {
                format: Format::by_id("jpeg").unwrap(),
                targets: vec![FormatId("png"), FormatId("webp")],
            },
            FormatMenu { format: Format::by_id("qoi").unwrap(), targets: vec![] },
        ]
    }

    #[test]
    fn plan_shares_one_submenu_per_format() {
        let keys = plan(Path::new(r"C:\Tools\Tumble\tumblew.exe"), None, &menus());
        let paths: Vec<&str> = keys.iter().map(|k| k.path.as_str()).collect();
        assert_eq!(
            paths,
            [
                r"Tumble\menus\jpeg\shell\01-png",
                r"Tumble\menus\jpeg\shell\01-png\command",
                r"Tumble\menus\jpeg\shell\02-webp",
                r"Tumble\menus\jpeg\shell\02-webp\command",
                r"SystemFileAssociations\.jpg\shell\Tumble",
                r"SystemFileAssociations\.jpeg\shell\Tumble",
                r"SystemFileAssociations\.jfif\shell\Tumble",
                r"AppUserModelId\Tumble.Converter",
            ],
            "a format with no targets gets no menu"
        );
        assert_eq!(
            keys[1].values,
            [(None, r#""C:\Tools\Tumble\tumblew.exe" convert --to png "%1""#.to_string())]
        );
        assert!(
            keys[4].values.contains(&(Some("ExtendedSubCommandsKey"), r"Tumble\menus\jpeg".into()))
        );
        assert!(keys[4].values.contains(&(Some("MultiSelectModel"), "Player".into())));
    }

    /// Writes only under HKCU\Software\TumbleTest, which is removed again.
    #[test]
    fn sandbox_install_uninstall_round_trip() {
        const SANDBOX: &str = r"Software\TumbleTest";
        let root = format!(r"{SANDBOX}\Classes-{}", std::process::id());
        let full = |p: &str| format!(r"{root}\{p}");
        let value = |p: &str, name: Option<&str>| {
            Key::open(&full(p))
                .unwrap()
                .and_then(|k| k.get(name).unwrap())
                .and_then(|v| v.into_iter().next())
        };

        // Something the user already has for .jpg: it must survive.
        let (theirs, _) = Key::create(&full(r"SystemFileAssociations\.jpg\shell\edit")).unwrap();
        theirs.set(None, "Edit with Paint").unwrap();
        drop(theirs);

        let exe = Path::new(r"C:\Tumble\tumblew.exe");
        let done = install(&root, exe, Some(Path::new(r"C:\Tumble\tumble.ico")), &menus()).unwrap();
        assert_eq!(done.extensions, 3);
        // Again: idempotent.
        install(&root, exe, None, &menus()).unwrap();

        assert_eq!(
            value(r"SystemFileAssociations\.jfif\shell\Tumble", Some("MUIVerb")).as_deref(),
            Some("Convert to")
        );
        assert_eq!(
            value(r"Tumble\menus\jpeg\shell\02-webp\command", None).as_deref(),
            Some(r#""C:\Tumble\tumblew.exe" convert --to webp "%1""#)
        );
        assert_eq!(
            value(r"AppUserModelId\Tumble.Converter", Some("DisplayName")).as_deref(),
            Some("Tumble")
        );
        assert!(!registry::exists(&full(r"SystemFileAssociations\.qoi")), "no targets, no keys");

        let st = status(&root).unwrap();
        let exts: Vec<&str> = st.registered.iter().map(|r| r.extension.as_str()).collect();
        assert_eq!(exts, ["jpg", "jpeg", "jfif"]);
        assert_eq!(st.tumblew.unwrap().0, exe);

        uninstall(&root).unwrap();
        assert!(
            registry::exists(&full(r"SystemFileAssociations\.jpg\shell\edit")),
            "user's key kept"
        );
        assert!(!registry::exists(&full(r"SystemFileAssociations\.jpg\shell\Tumble")));
        assert!(
            !registry::exists(&full(r"SystemFileAssociations\.jpeg")),
            "parents we created are gone"
        );
        assert!(!registry::exists(&full("Tumble")));
        assert!(!registry::exists(&full("AppUserModelId")));
        assert!(status(&root).unwrap().registered.is_empty());

        registry::delete_tree(SANDBOX).unwrap();
        assert!(!registry::exists(SANDBOX));
    }

    #[test]
    fn ancestors_are_shallowest_first() {
        assert_eq!(ancestors(r"a\b\c"), ["a", r"a\b"]);
    }
}
