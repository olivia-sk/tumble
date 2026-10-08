//! The COM objects Explorer talks to: a "Convert to" command whose
//! sub-commands are the targets for the current selection.
//!
//! Explorer calls `GetState` on the top command with the selection before
//! it asks for sub-commands, and `EnumSubCommands` gets no selection, so
//! the top command remembers the last selection it saw.
//!
//! Every method catches panics: this code runs inside explorer.exe.

use crate::menus::{self, Item, Source};
use crate::registry::Registry;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::sync::Mutex;
use tumble_core::brand;
use windows::Win32::Foundation::{E_FAIL, E_NOTIMPL, S_FALSE, S_OK};
use windows::Win32::System::Com::{CoTaskMemFree, IBindCtx};
use windows::Win32::UI::Shell::{
    ECF_DEFAULT, ECF_HASSUBCOMMANDS, ECS_ENABLED, ECS_HIDDEN, IEnumExplorerCommand,
    IEnumExplorerCommand_Impl, IExplorerCommand, IExplorerCommand_Impl, IShellItemArray, SHStrDupW,
    SIGDN_FILESYSPATH,
};
use windows::core::{BOOL, GUID, HRESULT, HSTRING, PWSTR, Ref, Result};
use windows_core::implement;

/// Runs `f`, turning a panic into E_FAIL.
fn guard<T>(f: impl FnOnce() -> Result<T>) -> Result<T> {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or_else(|_| Err(E_FAIL.into()))
}

fn co_string(s: &str) -> Result<PWSTR> {
    // SAFETY: SHStrDupW copies into CoTaskMem memory the caller frees.
    unsafe { SHStrDupW(&HSTRING::from(s)) }
}

/// File-system paths of the selected items.
pub fn selected_paths(items: Option<&IShellItemArray>) -> Vec<PathBuf> {
    let Some(items) = items else { return Vec::new() };
    let mut out = Vec::new();
    // SAFETY: standard IShellItemArray walk; each name is freed after copying.
    unsafe {
        let Ok(count) = items.GetCount() else { return out };
        for i in 0..count {
            let Ok(item) = items.GetItemAt(i) else { continue };
            let Ok(name) = item.GetDisplayName(SIGDN_FILESYSPATH) else { continue };
            if let Ok(s) = name.to_string() {
                out.push(PathBuf::from(s));
            }
            CoTaskMemFree(Some(name.0 as _));
        }
    }
    out
}

/// What a click does: run tumblew.exe, or, in tests, write the command
/// lines to the file named by `TUMBLE_EXPLORER_DRY_RUN`.
fn launch(target: &str, paths: &[PathBuf]) -> Result<()> {
    let src = Registry::from_env();
    let tumblew = src.tumblew().ok_or(windows::core::Error::from(E_FAIL))?;
    for args in menus::launches(target, paths) {
        if let Ok(log) = std::env::var(brand::env_var("EXPLORER_DRY_RUN")) {
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(log)
                .map_err(|_| E_FAIL)?;
            let line: Vec<String> = args.iter().map(|a| a.to_string_lossy().into_owned()).collect();
            let _ = writeln!(f, "{}\t{}", tumblew.display(), line.join("\t"));
        } else {
            std::process::Command::new(&tumblew).args(&args).spawn().map_err(|_| E_FAIL)?;
        }
    }
    Ok(())
}

#[implement(IExplorerCommand)]
pub struct ConvertTo {
    selection: Mutex<Vec<PathBuf>>,
}

impl ConvertTo {
    pub fn new() -> ConvertTo {
        ConvertTo { selection: Mutex::new(Vec::new()) }
    }

    fn remember(&self, items: Ref<IShellItemArray>) -> Vec<PathBuf> {
        let paths = selected_paths(items.as_ref());
        if !paths.is_empty()
            && let Ok(mut s) = self.selection.lock()
        {
            *s = paths.clone();
        }
        paths
    }
}

impl Default for ConvertTo {
    fn default() -> Self {
        ConvertTo::new()
    }
}

impl IExplorerCommand_Impl for ConvertTo_Impl {
    fn GetTitle(&self, _items: Ref<IShellItemArray>) -> Result<PWSTR> {
        guard(|| co_string("Convert to"))
    }

    fn GetIcon(&self, _items: Ref<IShellItemArray>) -> Result<PWSTR> {
        guard(|| match Registry::from_env().tumblew() {
            Some(exe) => co_string(&format!("{},0", exe.display())),
            None => Err(E_NOTIMPL.into()),
        })
    }

    fn GetToolTip(&self, _items: Ref<IShellItemArray>) -> Result<PWSTR> {
        Err(E_NOTIMPL.into())
    }

    fn GetCanonicalName(&self) -> Result<GUID> {
        Ok(crate::CLSID)
    }

    fn GetState(&self, items: Ref<IShellItemArray>, _ok_to_be_slow: BOOL) -> Result<u32> {
        guard(|| {
            let paths = self.remember(items);
            let shown = !menus::targets_for(&Registry::from_env(), &paths).is_empty();
            Ok(if shown { ECS_ENABLED.0 as u32 } else { ECS_HIDDEN.0 as u32 })
        })
    }

    fn Invoke(&self, _items: Ref<IShellItemArray>, _bind: Ref<IBindCtx>) -> Result<()> {
        Err(E_NOTIMPL.into())
    }

    fn GetFlags(&self) -> Result<u32> {
        Ok(ECF_HASSUBCOMMANDS.0 as u32)
    }

    fn EnumSubCommands(&self) -> Result<IEnumExplorerCommand> {
        guard(|| {
            let paths = self.selection.lock().map(|s| s.clone()).unwrap_or_default();
            let items: Vec<IExplorerCommand> = menus::targets_for(&Registry::from_env(), &paths)
                .into_iter()
                .map(|item| Target { item }.into())
                .collect();
            Ok(Commands { items, next: Mutex::new(0) }.into())
        })
    }
}

/// One target, e.g. "PNG".
#[implement(IExplorerCommand)]
pub struct Target {
    pub item: Item,
}

impl IExplorerCommand_Impl for Target_Impl {
    fn GetTitle(&self, _items: Ref<IShellItemArray>) -> Result<PWSTR> {
        guard(|| co_string(&self.item.label))
    }

    fn GetIcon(&self, _items: Ref<IShellItemArray>) -> Result<PWSTR> {
        Err(E_NOTIMPL.into())
    }

    fn GetToolTip(&self, _items: Ref<IShellItemArray>) -> Result<PWSTR> {
        Err(E_NOTIMPL.into())
    }

    fn GetCanonicalName(&self) -> Result<GUID> {
        Err(E_NOTIMPL.into())
    }

    fn GetState(&self, _items: Ref<IShellItemArray>, _ok_to_be_slow: BOOL) -> Result<u32> {
        Ok(ECS_ENABLED.0 as u32)
    }

    fn Invoke(&self, items: Ref<IShellItemArray>, _bind: Ref<IBindCtx>) -> Result<()> {
        guard(|| {
            let paths = selected_paths(items.as_ref());
            if paths.is_empty() {
                return Ok(());
            }
            launch(&self.item.id, &paths)
        })
    }

    fn GetFlags(&self) -> Result<u32> {
        Ok(ECF_DEFAULT.0 as u32)
    }

    fn EnumSubCommands(&self) -> Result<IEnumExplorerCommand> {
        Err(E_NOTIMPL.into())
    }
}

#[implement(IEnumExplorerCommand)]
pub struct Commands {
    items: Vec<IExplorerCommand>,
    next: Mutex<usize>,
}

impl IEnumExplorerCommand_Impl for Commands_Impl {
    // The raw pointers are the COM signature of IEnumExplorerCommand::Next;
    // the method cannot be marked unsafe.
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    fn Next(&self, count: u32, out: *mut Option<IExplorerCommand>, fetched: *mut u32) -> HRESULT {
        let Ok(mut next) = self.next.lock() else { return E_FAIL };
        let mut n = 0u32;
        while n < count && *next < self.items.len() {
            // SAFETY: Explorer passes room for `count` interfaces.
            unsafe { out.add(n as usize).write(Some(self.items[*next].clone())) };
            *next += 1;
            n += 1;
        }
        if !fetched.is_null() {
            // SAFETY: optional out-pointer, checked for null.
            unsafe { fetched.write(n) };
        }
        if n == count { S_OK } else { S_FALSE }
    }

    fn Skip(&self, count: u32) -> Result<()> {
        let mut next = self.next.lock().map_err(|_| E_FAIL)?;
        *next = (*next + count as usize).min(self.items.len());
        Ok(())
    }

    fn Reset(&self) -> Result<()> {
        *self.next.lock().map_err(|_| E_FAIL)? = 0;
        Ok(())
    }

    fn Clone(&self) -> Result<IEnumExplorerCommand> {
        let next = *self.next.lock().map_err(|_| E_FAIL)?;
        Ok(Commands { items: self.items.clone(), next: Mutex::new(next) }.into())
    }
}
