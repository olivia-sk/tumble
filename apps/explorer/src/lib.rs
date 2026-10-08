//! Tumble's Windows 11 top-level context menu (PRD phase 7): a COM in-proc
//! server exposing one `IExplorerCommand`, registered through a sparse MSIX
//! package (see `package/` and `scripts/explorer-menu.ps1`). The classic
//! "Show more options" entries from `tumble menu install` stay as they are,
//! and this command reads its menus from them.
//!
//! - `menus.rs`     selection to targets, and tumblew.exe command lines
//! - `registry.rs`  reading the classic menu's keys
//! - `command.rs`   the COM objects

#![cfg(windows)]

pub mod command;
pub mod menus;
mod registry;

use std::ffi::c_void;
use windows::Win32::Foundation::{
    CLASS_E_CLASSNOTAVAILABLE, CLASS_E_NOAGGREGATION, E_POINTER, S_FALSE,
};
use windows::Win32::System::Com::{IClassFactory, IClassFactory_Impl};
use windows::core::{BOOL, GUID, HRESULT, IUnknown, Interface, Ref, Result};
use windows_core::implement;

/// The class Explorer creates; must match `package/AppxManifest.xml`.
pub const CLSID: GUID = GUID::from_u128(0x7c1e2b9a_4f0d_4c55_9a63_2d8e5b1f0a47);

#[implement(IClassFactory)]
struct Factory;

impl IClassFactory_Impl for Factory_Impl {
    fn CreateInstance(
        &self,
        outer: Ref<IUnknown>,
        iid: *const GUID,
        object: *mut *mut c_void,
    ) -> Result<()> {
        if object.is_null() || iid.is_null() {
            return Err(E_POINTER.into());
        }
        // SAFETY: checked for null above.
        unsafe { *object = std::ptr::null_mut() };
        if outer.is_some() {
            return Err(CLASS_E_NOAGGREGATION.into());
        }
        let command: windows::Win32::UI::Shell::IExplorerCommand = command::ConvertTo::new().into();
        // SAFETY: COM QueryInterface with valid pointers.
        unsafe { command.query(iid, object).ok() }
    }

    fn LockServer(&self, _lock: BOOL) -> Result<()> {
        Ok(())
    }
}

/// COM entry point: hands out the class factory for `CLSID`.
///
/// # Safety
/// Called by COM with valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllGetClassObject(
    clsid: *const GUID,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    if clsid.is_null() || iid.is_null() || out.is_null() {
        return E_POINTER;
    }
    // SAFETY: checked for null above.
    unsafe {
        *out = std::ptr::null_mut();
        if *clsid != CLSID {
            return CLASS_E_CLASSNOTAVAILABLE;
        }
        let factory: IClassFactory = Factory.into();
        factory.query(iid, out)
    }
}

/// The DLL stays loaded; Explorer creates the command often.
#[unsafe(no_mangle)]
pub extern "system" fn DllCanUnloadNow() -> HRESULT {
    S_FALSE
}
