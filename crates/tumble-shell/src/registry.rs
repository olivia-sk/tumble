//! A small, safe wrapper over the parts of the Win32 registry API Tumble
//! uses. Every path is relative to `HKEY_CURRENT_USER`; Tumble never
//! writes anywhere else.

use std::io;
use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, WIN32_ERROR};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_ALL_ACCESS, KEY_READ, REG_CREATED_NEW_KEY, REG_MULTI_SZ,
    REG_OPTION_NON_VOLATILE, REG_SZ, REG_VALUE_TYPE, RegCloseKey, RegCreateKeyExW, RegDeleteTreeW,
    RegOpenKeyExW, RegQueryInfoKeyW, RegQueryValueExW, RegSetValueExW,
};
use windows::core::{HSTRING, PCWSTR};

fn check(e: WIN32_ERROR) -> io::Result<()> {
    if e.is_ok() { Ok(()) } else { Err(io::Error::from_raw_os_error(e.0 as i32)) }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// An open key under HKCU, closed on drop.
pub struct Key(HKEY);

impl Drop for Key {
    fn drop(&mut self) {
        // SAFETY: the handle came from RegCreateKeyExW/RegOpenKeyExW.
        unsafe {
            let _ = RegCloseKey(self.0);
        }
    }
}

impl Key {
    /// Opens or creates `path`. Returns whether it was newly created.
    pub fn create(path: &str) -> io::Result<(Key, bool)> {
        let mut key = HKEY::default();
        let mut disposition = Default::default();
        // SAFETY: out-pointers are valid locals.
        check(unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                &HSTRING::from(path),
                None,
                None,
                REG_OPTION_NON_VOLATILE,
                KEY_ALL_ACCESS,
                None,
                &mut key,
                Some(&mut disposition),
            )
        })?;
        Ok((Key(key), disposition == REG_CREATED_NEW_KEY))
    }

    /// Opens `path` for reading, or `None` if it does not exist.
    pub fn open(path: &str) -> io::Result<Option<Key>> {
        let mut key = HKEY::default();
        // SAFETY: out-pointer is a valid local.
        let e = unsafe {
            RegOpenKeyExW(HKEY_CURRENT_USER, &HSTRING::from(path), None, KEY_READ, &mut key)
        };
        if e == ERROR_FILE_NOT_FOUND {
            return Ok(None);
        }
        check(e)?;
        Ok(Some(Key(key)))
    }

    /// Sets a string value; `None` is the key's default value.
    pub fn set(&self, name: Option<&str>, value: &str) -> io::Result<()> {
        let data = wide(value);
        self.set_raw(name, REG_SZ, &data)
    }

    pub fn set_multi(&self, name: &str, values: &[String]) -> io::Result<()> {
        let mut data: Vec<u16> = Vec::new();
        for v in values {
            data.extend(v.encode_utf16());
            data.push(0);
        }
        data.push(0);
        self.set_raw(Some(name), REG_MULTI_SZ, &data)
    }

    fn set_raw(&self, name: Option<&str>, kind: REG_VALUE_TYPE, data: &[u16]) -> io::Result<()> {
        let name = name.map(HSTRING::from);
        let pname = name.as_ref().map_or(PCWSTR::null(), |n| PCWSTR(n.as_ptr()));
        // SAFETY: `data` is a valid UTF-16 buffer viewed as bytes.
        let bytes =
            unsafe { std::slice::from_raw_parts(data.as_ptr().cast::<u8>(), data.len() * 2) };
        check(unsafe { RegSetValueExW(self.0, pname, None, kind, Some(bytes)) })
    }

    /// Reads a string (or multi-string) value.
    pub fn get(&self, name: Option<&str>) -> io::Result<Option<Vec<String>>> {
        let name = name.map(HSTRING::from);
        let pname = name.as_ref().map_or(PCWSTR::null(), |n| PCWSTR(n.as_ptr()));
        let mut size = 0u32;
        // SAFETY: size query with no buffer.
        let e = unsafe { RegQueryValueExW(self.0, pname, None, None, None, Some(&mut size)) };
        if e == ERROR_FILE_NOT_FOUND {
            return Ok(None);
        }
        check(e)?;
        let mut buf = vec![0u16; (size as usize).div_ceil(2) + 1];
        let mut bytes = (buf.len() * 2) as u32;
        // SAFETY: `buf` holds `bytes` bytes.
        check(unsafe {
            RegQueryValueExW(
                self.0,
                pname,
                None,
                None,
                Some(buf.as_mut_ptr().cast()),
                Some(&mut bytes),
            )
        })?;
        buf.truncate(bytes as usize / 2);
        let text = String::from_utf16_lossy(&buf);
        Ok(Some(text.split('\0').filter(|s| !s.is_empty()).map(str::to_string).collect()))
    }

    /// Whether the key has neither subkeys nor values.
    pub fn is_empty(&self) -> io::Result<bool> {
        let (mut subkeys, mut values) = (0u32, 0u32);
        // SAFETY: out-pointers are valid locals.
        check(unsafe {
            RegQueryInfoKeyW(
                self.0,
                None,
                None,
                None,
                Some(&mut subkeys),
                None,
                None,
                Some(&mut values),
                None,
                None,
                None,
                None,
            )
        })?;
        Ok(subkeys == 0 && values == 0)
    }
}

pub fn exists(path: &str) -> bool {
    matches!(Key::open(path), Ok(Some(_)))
}

/// Deletes `path` and everything under it. Missing keys are fine.
pub fn delete_tree(path: &str) -> io::Result<()> {
    let w = wide(path);
    // SAFETY: NUL-terminated path.
    let e = unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, PCWSTR(w.as_ptr())) };
    if e == ERROR_FILE_NOT_FOUND {
        return Ok(());
    }
    check(e)?;
    // RegDeleteTreeW empties the key but leaves it; remove it too.
    delete_empty(path)
}

/// Deletes `path` only if it exists and is empty.
pub fn delete_empty(path: &str) -> io::Result<()> {
    match Key::open(path)? {
        Some(k) if k.is_empty()? => {
            drop(k);
            let w = wide(path);
            // SAFETY: NUL-terminated path; the key has no subkeys.
            let e = unsafe {
                windows::Win32::System::Registry::RegDeleteKeyW(
                    HKEY_CURRENT_USER,
                    PCWSTR(w.as_ptr()),
                )
            };
            if e == ERROR_FILE_NOT_FOUND { Ok(()) } else { check(e) }
        }
        _ => Ok(()),
    }
}
