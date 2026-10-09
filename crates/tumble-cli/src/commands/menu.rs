//! `tumble menu install | uninstall | status`: the right-click menu, for
//! the current user only (no admin rights). Explorer's under
//! `HKCU\Software\Classes` on Windows, a Finder Quick Action on macOS, and
//! the menus of Dolphin, Nemo, Thunar and GNOME Files on Linux.

use crate::exit;
use tumble_core::format::FORMATS;
use tumble_core::menu::menu_targets;
use tumble_shell::FormatMenu;

pub fn run(action: &crate::args::MenuAction) -> u8 {
    use crate::args::MenuAction;
    let result = match action {
        MenuAction::Install => install(),
        MenuAction::Uninstall => uninstall(),
        MenuAction::Status => status(),
    };
    #[cfg(unix)]
    if let (Ok(exit::OK), Some(note)) = (&result, removed_note()) {
        match action {
            MenuAction::Install => {
                let _ = std::fs::remove_file(note);
            }
            MenuAction::Uninstall => {
                let _ = note.parent().map(std::fs::create_dir_all);
                let _ = std::fs::write(note, "The right-click menu was removed on purpose.\n");
            }
            MenuAction::Status => {}
        }
    }
    match result {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e}");
            exit::SOME_FAILED
        }
    }
}

/// Every readable format and its short list of targets on this machine.
#[cfg_attr(target_os = "macos", allow(dead_code))]
fn menus() -> Vec<FormatMenu> {
    let registry = tumble_engines::default_registry();
    FORMATS
        .iter()
        .filter(|f| registry.reads(f.id))
        .map(|f| FormatMenu { format: f, targets: menu_targets(&registry, f) })
        .collect()
}

/// Says which engines are missing, so their formats are not in the menu.
fn print_missing_engines() {
    let missing: Vec<String> = tumble_engines::engine_report()
        .into_iter()
        .filter(|e| !e.available)
        .map(|e| format!("  {}: {}", e.name, e.detail))
        .collect();
    if !missing.is_empty() {
        println!(
            "
Not included, because these engines are missing:"
        );
        for m in missing {
            println!("{m}");
        }
        println!("Run `tumble menu install` again after installing them.");
    }
}

/// Left by `menu uninstall` on macOS and Linux, so the desktop window,
/// which adds the menu when it first opens, doesn't add it back.
#[cfg(unix)]
fn removed_note() -> Option<std::path::PathBuf> {
    Some(tumble_core::config::data_dir()?.join("menu-removed"))
}

/// This program, with links resolved, which is what the menu runs.
#[cfg(unix)]
fn this_tumble() -> std::io::Result<std::path::PathBuf> {
    let exe = std::env::current_exe()?;
    Ok(std::fs::canonicalize(&exe).unwrap_or(exe))
}

/// Prints where the menu runs from, and whether that file still exists.
#[cfg(unix)]
fn print_runs(tumble: Option<(std::path::PathBuf, bool)>) {
    match tumble {
        Some((path, true)) => println!("Runs: {}", path.display()),
        Some((path, false)) => {
            println!("Runs: {} (MISSING: run `tumble menu install` again)", path.display())
        }
        None => {}
    }
}

#[cfg(target_os = "linux")]
fn places() -> std::io::Result<tumble_shell::menu::Places> {
    tumble_shell::menu::Places::from_env().ok_or_else(|| {
        std::io::Error::other("HOME is not set, so there is nowhere to install the menu")
    })
}

#[cfg(target_os = "linux")]
fn install() -> std::io::Result<u8> {
    use tumble_shell::menu::{self, Manager};
    let managers = menu::detect();
    if managers.is_empty() {
        eprintln!(
            "No supported file manager found (Dolphin, Nemo, Thunar or GNOME Files).              Tumble still works from the command line."
        );
        return Ok(exit::NO_ROUTE);
    }
    let tumble = this_tumble()?;
    let icon = tumble_shell::write_icon();
    let done = menu::install(&places()?, &tumble, icon.as_deref(), &menus(), &managers)?;
    println!("Right-click menu installed for {} formats in:", done.formats);
    for m in &done.managers {
        println!("  {}", m.name());
    }
    println!("Runs: {}", tumble.display());
    if done.managers.contains(&Manager::NautilusScripts) {
        println!(
            "
In GNOME Files it is under Scripts > {app}. For a \"Convert to\" submenu instead,              install nautilus-python (python3-nautilus or nautilus-python) and run              `tumble menu install` again.",
            app = tumble_core::brand::APP_NAME
        );
    }
    if done.managers.iter().any(|m| matches!(m, Manager::Nautilus | Manager::NautilusScripts)) {
        println!("Restart GNOME Files (nautilus -q) if the menu doesn't show yet.");
    }
    print_missing_engines();
    Ok(exit::OK)
}

#[cfg(target_os = "linux")]
fn uninstall() -> std::io::Result<u8> {
    tumble_shell::menu::uninstall(&places()?)?;
    println!("Right-click menu removed.");
    Ok(exit::OK)
}

#[cfg(target_os = "linux")]
fn status() -> std::io::Result<u8> {
    let st = tumble_shell::menu::status(&places()?)?;
    if st.managers.is_empty() {
        println!("The right-click menu is not installed. Run `tumble menu install`.");
        return Ok(exit::OK);
    }
    println!("Installed in:");
    for m in &st.managers {
        println!("  {}", m.name());
    }
    print_runs(st.tumble);
    Ok(exit::OK)
}

#[cfg(target_os = "macos")]
fn places() -> std::io::Result<tumble_shell::menu::Places> {
    tumble_shell::menu::Places::from_env().ok_or_else(|| {
        std::io::Error::other("HOME is not set, so there is nowhere to install the menu")
    })
}

#[cfg(target_os = "macos")]
fn install() -> std::io::Result<u8> {
    let tumble = this_tumble()?;
    tumble_shell::menu::install(&places()?, &tumble)?;
    println!(
        "Right-click menu installed: select files in Finder, then right-click > Quick Actions > {}.",
        tumble_shell::menu::ACTION
    );
    println!("Runs: {}", tumble.display());
    print_missing_engines();
    Ok(exit::OK)
}

#[cfg(target_os = "macos")]
fn uninstall() -> std::io::Result<u8> {
    tumble_shell::menu::uninstall(&places()?)?;
    println!("Right-click menu removed.");
    Ok(exit::OK)
}

#[cfg(target_os = "macos")]
fn status() -> std::io::Result<u8> {
    let st = tumble_shell::menu::status(&places()?)?;
    if !st.installed {
        println!("The right-click menu is not installed. Run `tumble menu install`.");
        return Ok(exit::OK);
    }
    println!("Installed: Finder > right-click > Quick Actions > {}", tumble_shell::menu::ACTION);
    print_runs(st.tumble);
    Ok(exit::OK)
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
fn install() -> std::io::Result<u8> {
    eprintln!("the right-click menu is not available on this system");
    Ok(exit::BAD_ARGS)
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
fn uninstall() -> std::io::Result<u8> {
    install()
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
fn status() -> std::io::Result<u8> {
    install()
}

#[cfg(windows)]
fn install() -> std::io::Result<u8> {
    use tumble_core::brand;
    use tumble_shell::menu;

    let exe = std::env::current_exe()?;
    let tumblew = exe.with_file_name(format!("{}.exe", brand::GUI_BIN));
    if !tumblew.is_file() {
        eprintln!("{} not found next to {}", tumblew.display(), exe.display());
        return Ok(exit::BAD_ARGS);
    }
    let menus = menus();
    let icon = tumble_shell::write_icon();
    let done = menu::install(menu::CLASSES, &tumblew, icon.as_deref(), &menus)?;
    let formats = menus.iter().filter(|m| !m.targets.is_empty()).count();
    println!("Right-click menu installed for {} extensions ({formats} formats).", done.extensions);
    println!("Runs: {}", tumblew.display());
    println!("On Windows 11 it is under \"Show more options\" (or Shift+F10).");
    print_missing_engines();
    Ok(exit::OK)
}

#[cfg(windows)]
fn uninstall() -> std::io::Result<u8> {
    tumble_shell::menu::uninstall(tumble_shell::menu::CLASSES)?;
    if let Some(icon) = tumble_shell::local_data_dir().map(|d| d.join("tumble.ico")) {
        let _ = std::fs::remove_file(icon);
    }
    println!("Right-click menu removed.");
    Ok(exit::OK)
}

#[cfg(windows)]
fn status() -> std::io::Result<u8> {
    let st = tumble_shell::menu::status(tumble_shell::menu::CLASSES)?;
    if st.registered.is_empty() {
        println!("The right-click menu is not installed. Run `tumble menu install`.");
        return Ok(exit::OK);
    }
    // Group extensions by the submenu they share.
    let mut groups: Vec<(String, Vec<String>)> = Vec::new();
    for r in st.registered {
        match groups.iter_mut().find(|(s, _)| *s == r.submenu) {
            Some((_, exts)) => exts.push(format!(".{}", r.extension)),
            None => groups.push((r.submenu, vec![format!(".{}", r.extension)])),
        }
    }
    let count: usize = groups.iter().map(|(_, e)| e.len()).sum();
    println!("Installed for {count} extensions:");
    for (submenu, exts) in &groups {
        println!("  {:<28} {}", exts.join(" "), submenu);
    }
    match st.tumblew {
        Some((path, true)) => println!("Runs: {}", path.display()),
        Some((path, false)) => {
            println!("Runs: {} (MISSING: run `tumble menu install` again)", path.display())
        }
        None => {}
    }
    Ok(exit::OK)
}
