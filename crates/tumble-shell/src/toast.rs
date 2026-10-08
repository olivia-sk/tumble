//! One Windows toast when a right-click job ends. Clicking it opens the
//! output folder (protocol activation of a `file:` URL, so no COM server or
//! running process is needed afterwards).
//!
//! Toasts are shown under the AppUserModelID that `menu install` registers
//! (`HKCU\Software\Classes\AppUserModelId\Tumble.Converter`).

use std::path::Path;
use tumble_core::brand;
use windows::Data::Xml::Dom::XmlDocument;
use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};
use windows::core::HSTRING;

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// `file:///C:/Users/Jo%20Doe/Pictures/`, for opening a folder.
fn folder_url(dir: &Path) -> String {
    let mut url = String::from("file:///");
    for b in dir.to_string_lossy().replace('\\', "/").bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b':' | b'-' | b'_' | b'.' | b'~' => {
                url.push(b as char)
            }
            _ => url.push_str(&format!("%{b:02X}")),
        }
    }
    if !url.ends_with('/') {
        url.push('/');
    }
    url
}

/// The toast XML: a title, a detail line, and a click that opens `folder`.
pub fn xml(title: &str, detail: &str, folder: Option<&Path>) -> String {
    let launch = folder
        .map(|f| format!(r#" activationType="protocol" launch="{}""#, escape(&folder_url(f))))
        .unwrap_or_default();
    format!(
        r#"<toast{launch}><visual><binding template="ToastGeneric"><text>{}</text><text>{}</text></binding></visual></toast>"#,
        escape(title),
        escape(detail)
    )
}

/// Shows the toast. Errors (toasts disabled, AUMID not registered) are
/// returned for logging; they never fail the job.
pub fn show(title: &str, detail: &str, folder: Option<&Path>) -> windows::core::Result<()> {
    // SAFETY: WinRT needs COM on this thread; an already-initialised
    // apartment is fine.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
    let doc = XmlDocument::new()?;
    doc.LoadXml(&HSTRING::from(xml(title, detail, folder)))?;
    let toast = ToastNotification::CreateToastNotification(&doc)?;
    ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(brand::AUMID))?.Show(&toast)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toast_xml_escapes_and_links_the_folder() {
        let x =
            xml("12 files converted", "to PNG in Tom & Jo's", Some(Path::new(r"C:\Tom & Jo\Pics")));
        assert!(x.contains("Tom &amp; Jo&apos;s"));
        assert!(x.contains(r#"launch="file:///C:/Tom%20%26%20Jo/Pics/""#), "{x}");
        assert!(!xml("a", "b", None).contains("launch"));
    }
}
