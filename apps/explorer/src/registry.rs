//! Reads the classic menu's keys under `HKCU\<root>` (normally
//! `Software\Classes`). Read-only: this DLL never writes the registry.

use crate::menus::{Item, Source};
use std::path::PathBuf;
use tumble_core::brand;
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_READ, RRF_RT_REG_SZ, RegCloseKey, RegEnumKeyExW, RegGetValueW,
    RegOpenKeyExW,
};
use windows::core::{HSTRING, PCWSTR, PWSTR};

pub struct Registry {
    pub root: String,
}

impl Registry {
    /// `TUMBLE_CLASSES_ROOT` lets tests point at a sandbox; Explorer never
    /// sets it, so the real menu always reads `Software\Classes`.
    pub fn from_env() -> Registry {
        let root = std::env::var(brand::env_var("CLASSES_ROOT"))
            .unwrap_or_else(|_| r"Software\Classes".into());
        Registry { root }
    }

    fn string(&self, key: &str, name: Option<&str>) -> Option<String> {
        let key = HSTRING::from(format!(r"{}\{key}", self.root));
        let name = name.map(HSTRING::from);
        let pname = name.as_ref().map_or(PCWSTR::null(), |n| PCWSTR(n.as_ptr()));
        let mut buf = vec![0u16; 1024];
        let mut bytes = (buf.len() * 2) as u32;
        // SAFETY: `buf` holds `bytes` bytes; RRF_RT_REG_SZ NUL-terminates.
        let r = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                &key,
                pname,
                RRF_RT_REG_SZ,
                None,
                Some(buf.as_mut_ptr().cast()),
                Some(&mut bytes),
            )
        };
        if r != ERROR_SUCCESS {
            return None;
        }
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        Some(String::from_utf16_lossy(&buf[..len]))
    }

    fn subkeys(&self, key: &str) -> Vec<String> {
        let path = HSTRING::from(format!(r"{}\{key}", self.root));
        let mut hkey = HKEY::default();
        // SAFETY: out-pointer is a valid local; closed below.
        if unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, &path, None, KEY_READ, &mut hkey) }
            != ERROR_SUCCESS
        {
            return Vec::new();
        }
        let mut names = Vec::new();
        for i in 0.. {
            let mut buf = [0u16; 256];
            let mut len = buf.len() as u32;
            // SAFETY: `buf` holds `len` characters.
            let r = unsafe {
                RegEnumKeyExW(
                    hkey,
                    i,
                    Some(PWSTR(buf.as_mut_ptr())),
                    &mut len,
                    None,
                    None,
                    None,
                    None,
                )
            };
            if r != ERROR_SUCCESS {
                break;
            }
            names.push(String::from_utf16_lossy(&buf[..len as usize]));
        }
        // SAFETY: opened above.
        unsafe {
            let _ = RegCloseKey(hkey);
        }
        names.sort();
        names
    }
}

impl Source for Registry {
    fn submenu_for(&self, ext: &str) -> Option<String> {
        let verb = format!(r"SystemFileAssociations\.{ext}\shell\{}", brand::MENU_VERB);
        self.string(&verb, Some("ExtendedSubCommandsKey"))
    }

    fn items(&self, submenu: &str) -> Vec<Item> {
        // Item keys are named `NN-<format id>`, so their order is the menu order.
        self.subkeys(&format!(r"{submenu}\shell"))
            .into_iter()
            .filter_map(|name| {
                let id = name.split_once('-')?.1.to_string();
                let label = self
                    .string(&format!(r"{submenu}\shell\{name}"), Some("MUIVerb"))
                    .unwrap_or_else(|| id.clone());
                Some(Item { id, label })
            })
            .collect()
    }

    fn tumblew(&self) -> Option<PathBuf> {
        self.string(brand::REGISTRY_KEY, Some("Tumblew")).map(PathBuf::from).filter(|p| p.is_file())
    }
}
