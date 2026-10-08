//! `tumble menu install | uninstall | status`: the Explorer right-click
//! menu, under `HKCU\Software\Classes` only (no admin rights).

use crate::exit;

#[cfg(not(windows))]
pub fn run(_action: &crate::args::MenuAction) -> u8 {
    eprintln!("the right-click menu is only available on Windows");
    exit::BAD_ARGS
}

#[cfg(windows)]
pub fn run(action: &crate::args::MenuAction) -> u8 {
    use crate::args::MenuAction;
    let result = match action {
        MenuAction::Install => install(),
        MenuAction::Uninstall => uninstall(),
        MenuAction::Status => status(),
    };
    match result {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e}");
            exit::SOME_FAILED
        }
    }
}

#[cfg(windows)]
fn install() -> std::io::Result<u8> {
    use tumble_core::brand;
    use tumble_core::format::FORMATS;
    use tumble_core::menu::menu_targets;
    use tumble_shell::menu::{self, FormatMenu};

    let exe = std::env::current_exe()?;
    let tumblew = exe.with_file_name(format!("{}.exe", brand::GUI_BIN));
    if !tumblew.is_file() {
        eprintln!("{} not found next to {}", tumblew.display(), exe.display());
        return Ok(exit::BAD_ARGS);
    }
    let registry = tumble_engines::default_registry();
    let menus: Vec<FormatMenu> = FORMATS
        .iter()
        .filter(|f| registry.reads(f.id))
        .map(|f| FormatMenu { format: f, targets: menu_targets(&registry, f) })
        .collect();
    let icon = tumble_shell::write_icon();
    let done = menu::install(menu::CLASSES, &tumblew, icon.as_deref(), &menus)?;
    let formats = menus.iter().filter(|m| !m.targets.is_empty()).count();
    println!("Right-click menu installed for {} extensions ({formats} formats).", done.extensions);
    println!("Runs: {}", tumblew.display());
    println!("On Windows 11 it is under \"Show more options\" (or Shift+F10).");

    let missing: Vec<String> = tumble_engines::engine_report()
        .into_iter()
        .filter(|e| !e.available)
        .map(|e| format!("  {}: {}", e.name, e.detail))
        .collect();
    if !missing.is_empty() {
        println!("\nNot included, because these engines are missing:");
        for m in missing {
            println!("{m}");
        }
        println!("Run `tumble menu install` again after installing them.");
    }
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
