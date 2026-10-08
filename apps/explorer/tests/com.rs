//! The command as Explorer uses it: through DllGetClassObject and the class
//! factory, with real shell-item selections, against a sandbox copy of the
//! classic menu under HKCU\Software\TumbleTest (removed afterwards). Clicks
//! are recorded through TUMBLE_EXPLORER_DRY_RUN instead of launching.

use std::path::{Path, PathBuf};
use tumble_core::{Format, FormatId};
use tumble_explorer::{CLSID, DllGetClassObject};
use tumble_shell::menu::{FormatMenu, install};
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, IClassFactory};
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::UI::Shell::{
    ECS_ENABLED, ECS_HIDDEN, IExplorerCommand, ILCreateFromPathW, ILFree, IShellItemArray,
    SHCreateShellItemArrayFromIDLists,
};
use windows::core::{HSTRING, Interface};

const SANDBOX: &str = r"Software\TumbleTest";

fn selection(paths: &[PathBuf]) -> IShellItemArray {
    // SAFETY: PIDLs are created, used for the array, then freed.
    unsafe {
        let pidls: Vec<*const ITEMIDLIST> = paths
            .iter()
            .map(|p| ILCreateFromPathW(&HSTRING::from(p.as_os_str())) as *const _)
            .collect();
        assert!(pidls.iter().all(|p| !p.is_null()), "paths must exist");
        let array = SHCreateShellItemArrayFromIDLists(&pidls).unwrap();
        for p in pidls {
            ILFree(Some(p));
        }
        array
    }
}

fn command() -> IExplorerCommand {
    // SAFETY: the same calls COM makes when Explorer creates the command.
    unsafe {
        let mut factory = std::ptr::null_mut();
        DllGetClassObject(&CLSID, &IClassFactory::IID, &mut factory).ok().unwrap();
        let factory = IClassFactory::from_raw(factory);
        factory.CreateInstance::<_, IExplorerCommand>(None).unwrap()
    }
}

fn title(cmd: &IExplorerCommand, items: Option<&IShellItemArray>) -> String {
    // SAFETY: the returned string is CoTaskMem; copied, then leaked in a test.
    unsafe { cmd.GetTitle(items).unwrap().to_string().unwrap() }
}

fn sub_commands(cmd: &IExplorerCommand) -> Vec<IExplorerCommand> {
    let mut out = Vec::new();
    // SAFETY: standard enumerator walk.
    unsafe {
        let e = cmd.EnumSubCommands().unwrap();
        loop {
            let mut item = [None];
            let mut n = 0;
            let _ = e.Next(&mut item, Some(&mut n));
            if n == 0 {
                break;
            }
            out.push(item[0].take().unwrap());
        }
    }
    out
}

fn file(dir: &Path, name: &str) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, b"x").unwrap();
    p
}

#[test]
fn explorer_command_end_to_end() {
    // SAFETY: COM for this test thread.
    let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    let dir = tempfile::tempdir().unwrap();
    let tumblew = file(dir.path(), "tumblew.exe");
    let root = format!(r"{SANDBOX}\Explorer-{}", std::process::id());
    let menus = [
        FormatMenu {
            format: Format::by_id("jpeg").unwrap(),
            targets: vec![FormatId("png"), FormatId("webp")],
        },
        FormatMenu {
            format: Format::by_id("png").unwrap(),
            targets: vec![FormatId("jpeg"), FormatId("webp")],
        },
    ];
    install(&root, &tumblew, None, &menus).unwrap();
    let log = dir.path().join("launches.txt");
    // SAFETY: set once, before any other thread in this test binary reads them.
    unsafe {
        std::env::set_var("TUMBLE_CLASSES_ROOT", &root);
        std::env::set_var("TUMBLE_EXPLORER_DRY_RUN", &log);
    }

    let result = std::panic::catch_unwind(|| {
        let cmd = command();
        assert_eq!(title(&cmd, None), "Convert to");

        // A JPEG: its own menu, in order, without JPEG.
        let jpg = file(dir.path(), "-holiday photo.jpg");
        let one = selection(std::slice::from_ref(&jpg));
        // SAFETY: plain COM calls.
        let state = unsafe { cmd.GetState(&one, true).unwrap() };
        assert_eq!(state, ECS_ENABLED.0 as u32);
        let subs = sub_commands(&cmd);
        let labels: Vec<String> = subs.iter().map(|s| title(s, None)).collect();
        assert_eq!(labels, ["PNG", "WebP"]);

        // A JPEG and a PNG together: only what both can become.
        let png = file(dir.path(), "scan.png");
        let both = selection(&[jpg.clone(), png.clone()]);
        // SAFETY: plain COM calls.
        unsafe { cmd.GetState(&both, true).unwrap() };
        let subs = sub_commands(&cmd);
        assert_eq!(subs.iter().map(|s| title(s, None)).collect::<Vec<_>>(), ["WebP"]);

        // Clicking it launches tumblew once for the whole selection.
        // SAFETY: plain COM call.
        unsafe { subs[0].Invoke(&both, None).unwrap() };
        let launched = std::fs::read_to_string(&log).unwrap();
        let fields: Vec<&str> = launched.trim_end().split('\t').collect();
        assert_eq!(fields[0], tumblew.display().to_string());
        assert_eq!(&fields[1..5], ["convert", "--to", "webp", "--"]);
        // Explorer hands over long names even when TEMP uses a short
        // (8.3) one, as on CI machines, so compare the files themselves.
        let real = |p: &std::path::Path| std::fs::canonicalize(p).unwrap();
        let passed: Vec<_> = fields[5..].iter().map(|f| real(std::path::Path::new(f))).collect();
        assert_eq!(passed, [real(&jpg), real(&png)]);

        // An unknown type hides the command.
        let odd = selection(&[file(dir.path(), "notes.xyz")]);
        // SAFETY: plain COM call.
        let state = unsafe { cmd.GetState(&odd, true).unwrap() };
        assert_eq!(state, ECS_HIDDEN.0 as u32);
    });

    let _ = std::process::Command::new("reg")
        .args(["delete", &format!(r"HKCU\{SANDBOX}"), "/f"])
        .output();
    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}
