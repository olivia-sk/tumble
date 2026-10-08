//! The right-click menu on Linux, for the file managers of the common
//! desktops. Every entry runs `tumble convert --to <format> -- <files>`,
//! with the same short lists as Windows (`menu_targets`).
//!
//! ```text
//! Dolphin (KDE)        ~/.local/share/kio/servicemenus/tumble-<format>.desktop
//!                      "Convert to" submenu, matched by MIME type
//! Nemo (Cinnamon)      ~/.local/share/nemo/actions/tumble-<format>-<target>.nemo_action
//!                      "Convert to PNG", matched by extension
//! Thunar (Xfce)        <action>s with a tumble- unique-id in ~/.config/Thunar/uca.xml
//!                      "Convert to PNG", matched by extension
//! Files (GNOME)        ~/.local/share/nautilus-python/extensions/tumble.py
//!                      "Convert to" submenu (needs nautilus-python), or else
//!                      ~/.local/share/nautilus/scripts/Tumble/Convert to PNG
//! install record       ~/.local/share/tumble/menu.txt
//! ```
//!
//! `install` only writes for the file managers that are installed;
//! `uninstall` removes every Tumble entry for all of them, and nothing else.

use crate::FormatMenu;
use crate::unix::{shell_quote, which};
use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use tumble_core::format::FORMATS;
use tumble_core::{Format, FormatId, brand};

/// Marks files Tumble wrote, so uninstall never removes anything else.
const MARK: &str = "Tumble menu entry";
/// Prefix of Tumble's Thunar unique-ids and file names.
const PREFIX: &str = "tumble-";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Manager {
    Dolphin,
    Nemo,
    Thunar,
    /// GNOME Files with nautilus-python: a real submenu.
    Nautilus,
    /// GNOME Files without nautilus-python: the Scripts menu.
    NautilusScripts,
}

impl Manager {
    pub fn name(self) -> &'static str {
        match self {
            Manager::Dolphin => "Dolphin (KDE)",
            Manager::Nemo => "Nemo (Cinnamon)",
            Manager::Thunar => "Thunar (Xfce)",
            Manager::Nautilus => "Files (GNOME)",
            Manager::NautilusScripts => "Files (GNOME), in the Scripts menu",
        }
    }

    fn key(self) -> &'static str {
        match self {
            Manager::Dolphin => "dolphin",
            Manager::Nemo => "nemo",
            Manager::Thunar => "thunar",
            Manager::Nautilus => "nautilus-python",
            Manager::NautilusScripts => "nautilus",
        }
    }

    fn from_key(key: &str) -> Option<Manager> {
        [
            Manager::Dolphin,
            Manager::Nemo,
            Manager::Thunar,
            Manager::Nautilus,
            Manager::NautilusScripts,
        ]
        .into_iter()
        .find(|m| m.key() == key)
    }
}

/// Whether nautilus-python is installed (its plugin sits in Nautilus's
/// extension folder, which differs between distributions).
fn has_nautilus_python() -> bool {
    let roots =
        ["/usr/lib", "/usr/lib64", "/usr/lib/x86_64-linux-gnu", "/usr/lib/aarch64-linux-gnu"];
    roots.iter().any(|root| {
        ["extensions-4", "extensions-3.0"].iter().any(|ext| {
            Path::new(root).join("nautilus").join(ext).join("libnautilus-python.so").is_file()
        })
    })
}

/// The file managers installed here. `TUMBLE_FILE_MANAGERS` (a comma
/// separated list of dolphin, nemo, thunar, nautilus, nautilus-python)
/// overrides the check; tests use it.
pub fn detect() -> Vec<Manager> {
    if let Some(list) = std::env::var_os(brand::env_var("FILE_MANAGERS")) {
        return list
            .to_string_lossy()
            .split(',')
            .filter_map(|k| Manager::from_key(k.trim()))
            .collect();
    }
    let mut found = Vec::new();
    if which("dolphin").is_some() {
        found.push(Manager::Dolphin);
    }
    if which("nemo").is_some() {
        found.push(Manager::Nemo);
    }
    if which("thunar").is_some() {
        found.push(Manager::Thunar);
    }
    if which("nautilus").is_some() {
        found.push(if has_nautilus_python() {
            Manager::Nautilus
        } else {
            Manager::NautilusScripts
        });
    }
    found
}

/// The XDG base folders the menu files go in.
pub struct Places {
    /// `$XDG_DATA_HOME`, or `~/.local/share`.
    pub data: PathBuf,
    /// `$XDG_CONFIG_HOME`, or `~/.config`.
    pub config: PathBuf,
}

impl Places {
    pub fn from_env() -> Option<Places> {
        let var =
            |name: &str| std::env::var_os(name).map(PathBuf::from).filter(|p| p.is_absolute());
        let home = var("HOME");
        let data =
            var("XDG_DATA_HOME").or_else(|| home.as_ref().map(|h| h.join(".local/share")))?;
        let config = var("XDG_CONFIG_HOME").or_else(|| home.as_ref().map(|h| h.join(".config")))?;
        Some(Places { data, config })
    }

    fn dolphin(&self) -> PathBuf {
        self.data.join("kio/servicemenus")
    }
    fn nemo(&self) -> PathBuf {
        self.data.join("nemo/actions")
    }
    fn thunar(&self) -> PathBuf {
        self.config.join("Thunar/uca.xml")
    }
    fn nautilus_extension(&self) -> PathBuf {
        self.data.join("nautilus-python/extensions/tumble.py")
    }
    fn nautilus_scripts(&self) -> PathBuf {
        self.data.join("nautilus/scripts").join(brand::APP_NAME)
    }
    fn record(&self) -> PathBuf {
        self.data.join(brand::UNIX_DIR).join("menu.txt")
    }
}

/// One argument of a desktop entry's `Exec` line: always quoted, with the
/// characters the spec reserves escaped, then escaped again for the key
/// file's own string syntax.
fn exec_arg(s: &str) -> String {
    let mut quoted = String::from("\"");
    for c in s.chars() {
        if matches!(c, '"' | '`' | '$' | '\\') {
            quoted.push('\\');
        }
        quoted.push(c);
    }
    quoted.push('"');
    quoted.replace('\\', "\\\\").replace('%', "%%")
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// MIME types (shared-mime-info names) for each format, for Dolphin.
fn mime_types(id: FormatId) -> &'static [&'static str] {
    match id.as_str() {
        "jpeg" => &["image/jpeg"],
        "png" => &["image/png"],
        "webp" => &["image/webp"],
        "heic" => &["image/heif", "image/heic", "image/heif-sequence", "image/heic-sequence"],
        "avif" => &["image/avif"],
        "gif" => &["image/gif"],
        "tiff" => &["image/tiff"],
        "bmp" => &["image/bmp", "image/x-bmp", "image/x-ms-bmp"],
        "ico" => &["image/vnd.microsoft.icon", "image/x-icon"],
        "tga" => &["image/x-tga"],
        "ppm" => &[
            "image/x-portable-pixmap",
            "image/x-portable-graymap",
            "image/x-portable-bitmap",
            "image/x-portable-anymap",
        ],
        "qoi" => &["image/qoi"],
        "exr" => &["image/x-exr"],
        "svg" => &["image/svg+xml"],
        "mp4" => &["video/mp4", "video/x-m4v"],
        "mov" => &["video/quicktime"],
        "webm" => &["video/webm"],
        "mkv" => &["video/x-matroska"],
        "avi" => &["video/vnd.avi", "video/x-msvideo", "video/avi"],
        "mp3" => &["audio/mpeg"],
        "wav" => &["audio/vnd.wave", "audio/x-wav", "audio/wav"],
        "flac" => &["audio/flac", "audio/x-flac"],
        "aac" => &["audio/aac", "audio/x-aac"],
        "m4a" => &["audio/mp4", "audio/x-m4a"],
        "ogg" => &["audio/ogg", "audio/x-vorbis+ogg", "audio/vorbis"],
        "opus" => &["audio/x-opus+ogg", "audio/opus"],
        "pdf" => &["application/pdf"],
        "docx" => &["application/vnd.openxmlformats-officedocument.wordprocessingml.document"],
        "doc" => &["application/msword"],
        "odt" => &["application/vnd.oasis.opendocument.text"],
        "rtf" => &["application/rtf", "text/rtf"],
        "txt" => &["text/plain"],
        "html" => &["text/html"],
        "md" => &["text/markdown", "text/x-markdown"],
        "pptx" => &["application/vnd.openxmlformats-officedocument.presentationml.presentation"],
        "ppt" => &["application/vnd.ms-powerpoint"],
        "odp" => &["application/vnd.oasis.opendocument.presentation"],
        "xlsx" => &["application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"],
        "xls" => &["application/vnd.ms-excel"],
        "ods" => &["application/vnd.oasis.opendocument.spreadsheet"],
        "csv" => &["text/csv"],
        _ => &[],
    }
}

/// Dolphin matches a MIME type's subtypes too (Markdown, CSV and HTML are
/// all text/plain), so the text menu leaves out the ones with their own.
fn excluded_mime_types(id: FormatId) -> Vec<&'static str> {
    if id.as_str() != "txt" {
        return Vec::new();
    }
    FORMATS
        .iter()
        .filter(|f| f.id != id)
        .flat_map(|f| mime_types(f.id).iter().copied())
        .filter(|m| m.starts_with("text/"))
        .collect()
}

fn dolphin_entry(tumble: &Path, icon: Option<&Path>, menu: &FormatMenu) -> String {
    let id = |t: &FormatId| format!("{PREFIX}{t}");
    let mut s =
        format!("# {MARK}, removed by `tumble menu uninstall`.\n[Desktop Entry]\nType=Service\n");
    s.push_str("X-KDE-ServiceTypes=KonqPopupMenu/Plugin\n");
    s.push_str(&format!("MimeType={};\n", mime_types(menu.format.id).join(";")));
    let excluded = excluded_mime_types(menu.format.id);
    if !excluded.is_empty() {
        s.push_str(&format!("ExcludeServiceTypes={};\n", excluded.join(";")));
    }
    let actions: Vec<String> = menu.targets.iter().map(id).collect();
    s.push_str(&format!("Actions={};\n", actions.join(";")));
    s.push_str("X-KDE-Submenu=Convert to\n");
    if let Some(icon) = icon {
        s.push_str(&format!("Icon={}\n", icon.display()));
    }
    for target in &menu.targets {
        s.push_str(&format!("\n[Desktop Action {}]\nName={}\n", id(target), target.format().name));
        if let Some(icon) = icon {
            s.push_str(&format!("Icon={}\n", icon.display()));
        }
        s.push_str(&format!(
            "Exec={} convert --to {target} -- %F\n",
            exec_arg(&tumble.to_string_lossy())
        ));
    }
    s
}

fn nemo_action(tumble: &Path, format: &Format, target: FormatId) -> String {
    let exts: Vec<&str> = format.extensions.to_vec();
    format!(
        "# {MARK}, removed by `tumble menu uninstall`.\n[Nemo Action]\nName=Convert to {}\n\
         Comment=Convert with {}\nExec={} convert --to {target} -- %F\nSelection=notnone\n\
         Extensions={};\nQuote=double\n",
        target.format().name,
        brand::APP_NAME,
        exec_arg(&tumble.to_string_lossy()).replace("%%", "%"),
        exts.join(";")
    )
}

/// Thunar matches patterns case-sensitively, so both cases are listed.
fn thunar_patterns(format: &Format) -> String {
    let mut patterns = Vec::new();
    for ext in format.extensions {
        patterns.push(format!("*.{ext}"));
        patterns.push(format!("*.{}", ext.to_uppercase()));
    }
    patterns.join(";")
}

fn thunar_action(tumble: &Path, icon: Option<&Path>, format: &Format, target: FormatId) -> String {
    let icon = icon.map(|i| xml_escape(&i.to_string_lossy())).unwrap_or_default();
    let command = format!("{} convert --to {target} -- %F", shell_quote(&tumble.to_string_lossy()));
    format!(
        "<action>\n\t<icon>{icon}</icon>\n\t<name>Convert to {}</name>\n\t<submenu></submenu>\n\
         \t<unique-id>{PREFIX}{}-{target}</unique-id>\n\t<command>{}</command>\n\
         \t<description>Convert with {}</description>\n\t<range></range>\n\
         \t<patterns>{}</patterns>\n\t<audio-files/>\n\t<image-files/>\n\t<other-files/>\n\
         \t<text-files/>\n\t<video-files/>\n</action>\n",
        xml_escape(target.format().name),
        format.id,
        xml_escape(&command),
        brand::APP_NAME,
        xml_escape(&thunar_patterns(format)),
    )
}

/// `uca.xml` without Tumble's actions: every `<action>` block whose
/// unique-id starts with `tumble-` is cut out, everything else is kept as
/// it was.
fn thunar_without_tumble(xml: &str) -> String {
    let mut out = String::with_capacity(xml.len());
    let mut rest = xml;
    while let Some(start) = rest.find("<action>") {
        let Some(len) = rest[start..].find("</action>") else { break };
        let end = start + len + "</action>".len();
        let block = &rest[start..end];
        out.push_str(&rest[..start]);
        if block.contains(&format!("<unique-id>{PREFIX}")) {
            // Drop the line break after the block too.
            let after = &rest[end..];
            rest = after.strip_prefix('\n').unwrap_or(after);
            // And the indentation before it.
            let trimmed = out.trim_end_matches([' ', '\t']).len();
            out.truncate(trimmed);
        } else {
            out.push_str(block);
            rest = &rest[end..];
        }
    }
    out.push_str(rest);
    out
}

/// The system's default `uca.xml`, which Thunar copies on first start; a
/// new user file starts from it so its default actions are kept.
fn thunar_default() -> String {
    let dirs = std::env::var("XDG_CONFIG_DIRS").unwrap_or_default();
    dirs.split(':')
        .filter(|d| d.starts_with('/'))
        .chain(["/etc/xdg"])
        .find_map(|d| fs::read_to_string(Path::new(d).join("Thunar/uca.xml")).ok())
        .filter(|x| x.contains("</actions>"))
        .unwrap_or_else(|| {
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<actions>\n</actions>\n".into()
        })
}

fn thunar_install(path: &Path, actions: &str) -> io::Result<()> {
    let current = match fs::read_to_string(path) {
        Ok(xml) => xml,
        Err(e) if e.kind() == io::ErrorKind::NotFound => thunar_default(),
        Err(e) => return Err(e),
    };
    let mut xml = thunar_without_tumble(&current);
    let Some(at) = xml.rfind("</actions>") else {
        return Err(io::Error::other(format!("{} has no </actions>; not changed", path.display())));
    };
    xml.insert_str(at, actions);
    write(path, &xml, false)
}

fn thunar_uninstall(path: &Path) -> io::Result<()> {
    match fs::read_to_string(path) {
        Ok(xml) => {
            let cleaned = thunar_without_tumble(&xml);
            if cleaned != xml { write(path, &cleaned, false) } else { Ok(()) }
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// Writes through a temporary file and a rename, so a file manager never
/// reads half a file.
fn write(path: &Path, text: &str, executable: bool) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_file_name(format!(
        ".{}.{}.tmp",
        path.file_name().unwrap_or_default().to_string_lossy(),
        std::process::id()
    ));
    fs::write(&tmp, text)?;
    let mode = if executable { 0o755 } else { 0o644 };
    fs::set_permissions(&tmp, fs::Permissions::from_mode(mode))?;
    fs::rename(&tmp, path)
}

/// Python string literal for a path.
fn python_str(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n"))
}

fn nautilus_script(tumble: &Path, target: FormatId) -> String {
    format!(
        "#!/bin/sh\n# {MARK}, removed by `tumble menu uninstall`.\nexec {} convert --to {target} -- \"$@\"\n",
        shell_quote(&tumble.to_string_lossy())
    )
}

/// Every output in the menus, in table order, for the Scripts menu (which
/// cannot be limited to certain file types).
fn all_targets(menus: &[&FormatMenu]) -> Vec<FormatId> {
    FORMATS.iter().map(|f| f.id).filter(|id| menus.iter().any(|m| m.targets.contains(id))).collect()
}

/// Files in `dir` named `<prefix>*<suffix>` that carry `MARK`.
fn marked_files(dir: &Path, suffix: &str) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else { return Vec::new() };
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            let name = p.file_name().unwrap_or_default().to_string_lossy();
            name.starts_with(PREFIX) && name.ends_with(suffix)
        })
        .filter(|p| fs::read_to_string(p).is_ok_and(|t| t.contains(MARK)))
        .collect();
    files.sort();
    files
}

/// What `install` did.
#[derive(Debug)]
pub struct Installed {
    pub managers: Vec<Manager>,
    /// Input formats that got a menu.
    pub formats: usize,
}

/// Writes the menu for `managers`. Removes any earlier install first, so
/// running it again is safe.
pub fn install(
    places: &Places,
    tumble: &Path,
    icon: Option<&Path>,
    menus: &[FormatMenu],
    managers: &[Manager],
) -> io::Result<Installed> {
    uninstall(places)?;
    let menus: Vec<&FormatMenu> = menus.iter().filter(|m| !m.targets.is_empty()).collect();
    for manager in managers {
        match manager {
            Manager::Dolphin => {
                for m in &menus {
                    let path = places.dolphin().join(format!("{PREFIX}{}.desktop", m.format.id));
                    // Dolphin only runs service menus that are executable.
                    write(&path, &dolphin_entry(tumble, icon, m), true)?;
                }
            }
            Manager::Nemo => {
                for m in &menus {
                    for t in &m.targets {
                        let name = format!("{PREFIX}{}-{t}.nemo_action", m.format.id);
                        write(
                            &places.nemo().join(name),
                            &nemo_action(tumble, m.format, *t),
                            false,
                        )?;
                    }
                }
            }
            Manager::Thunar => {
                let actions: String = menus
                    .iter()
                    .flat_map(|m| {
                        m.targets.iter().map(|t| thunar_action(tumble, icon, m.format, *t))
                    })
                    .collect();
                thunar_install(&places.thunar(), &actions)?;
            }
            Manager::Nautilus => {
                let source = include_str!("nautilus.py")
                    .replace("@TUMBLE@", &python_str(&tumble.to_string_lossy()));
                write(&places.nautilus_extension(), &source, false)?;
            }
            Manager::NautilusScripts => {
                for t in all_targets(&menus) {
                    let path =
                        places.nautilus_scripts().join(format!("Convert to {}", t.format().name));
                    write(&path, &nautilus_script(tumble, t), true)?;
                }
            }
        }
    }
    let keys: Vec<&str> = managers.iter().map(|m| m.key()).collect();
    write(
        &places.record(),
        &format!("tumble={}\nmanagers={}\n", tumble.display(), keys.join(",")),
        false,
    )?;
    refresh(managers);
    Ok(Installed { managers: managers.to_vec(), formats: menus.len() })
}

/// Asks KDE to pick up new service menus at once. Never fails the install.
fn refresh(managers: &[Manager]) {
    if managers.contains(&Manager::Dolphin)
        && std::env::var_os(brand::env_var("FILE_MANAGERS")).is_none()
        && let Some(kbuild) = ["kbuildsycoca6", "kbuildsycoca5"].into_iter().find_map(which)
    {
        let _ = std::process::Command::new(kbuild)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    }
}

/// Removes every Tumble menu entry under `places`, and nothing else.
pub fn uninstall(places: &Places) -> io::Result<()> {
    let remove = |p: &Path| match fs::remove_file(p) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    };
    for p in marked_files(&places.dolphin(), ".desktop") {
        remove(&p)?;
    }
    for p in marked_files(&places.nemo(), ".nemo_action") {
        remove(&p)?;
    }
    thunar_uninstall(&places.thunar())?;
    let extension = places.nautilus_extension();
    if fs::read_to_string(&extension).is_ok_and(|t| t.contains("tumble menu install")) {
        remove(&extension)?;
    }
    let scripts = places.nautilus_scripts();
    if let Ok(entries) = fs::read_dir(&scripts) {
        for p in entries.flatten().map(|e| e.path()) {
            if fs::read_to_string(&p).is_ok_and(|t| t.contains(MARK)) {
                remove(&p)?;
            }
        }
        let _ = fs::remove_dir(&scripts); // only if now empty
    }
    remove(&places.record())
}

pub struct Status {
    /// File managers that have Tumble's entries.
    pub managers: Vec<Manager>,
    /// The tumble the menu runs, and whether it still exists.
    pub tumble: Option<(PathBuf, bool)>,
}

pub fn status(places: &Places) -> io::Result<Status> {
    let mut managers = Vec::new();
    if !marked_files(&places.dolphin(), ".desktop").is_empty() {
        managers.push(Manager::Dolphin);
    }
    if !marked_files(&places.nemo(), ".nemo_action").is_empty() {
        managers.push(Manager::Nemo);
    }
    if fs::read_to_string(places.thunar())
        .is_ok_and(|x| x.contains(&format!("<unique-id>{PREFIX}")))
    {
        managers.push(Manager::Thunar);
    }
    if places.nautilus_extension().is_file() {
        managers.push(Manager::Nautilus);
    }
    if fs::read_dir(places.nautilus_scripts()).is_ok_and(|mut d| d.next().is_some()) {
        managers.push(Manager::NautilusScripts);
    }
    let tumble = fs::read_to_string(places.record()).ok().and_then(|text| {
        text.lines().find_map(|l| l.strip_prefix("tumble=")).map(|p| {
            let path = PathBuf::from(p);
            let exists = path.is_file();
            (path, exists)
        })
    });
    Ok(Status { managers, tumble })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn menus() -> Vec<FormatMenu> {
        vec![
            FormatMenu {
                format: Format::by_id("jpeg").unwrap(),
                targets: vec![FormatId("png"), FormatId("webp")],
            },
            FormatMenu { format: Format::by_id("txt").unwrap(), targets: vec![FormatId("pdf")] },
            FormatMenu { format: Format::by_id("qoi").unwrap(), targets: vec![] },
        ]
    }

    const ALL: &[Manager] = &[
        Manager::Dolphin,
        Manager::Nemo,
        Manager::Thunar,
        Manager::Nautilus,
        Manager::NautilusScripts,
    ];

    #[test]
    fn exec_arguments_are_quoted_and_escaped() {
        assert_eq!(exec_arg("/opt/tumble/tumble"), "\"/opt/tumble/tumble\"");
        assert_eq!(exec_arg("/a b/$x\"y"), r#""/a b/\\$x\\"y""#);
        assert_eq!(exec_arg("/100%/t"), "\"/100%%/t\"");
    }

    #[test]
    fn every_format_has_a_mime_type() {
        for f in FORMATS {
            assert!(!mime_types(f.id).is_empty(), "{} has no MIME type", f.id);
        }
        let excluded = excluded_mime_types(FormatId("txt"));
        assert!(excluded.contains(&"text/csv") && excluded.contains(&"text/markdown"));
        assert!(!excluded.contains(&"text/plain"));
    }

    #[test]
    fn thunar_keeps_the_users_own_actions() {
        let users = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<actions>\n<action>\n\t<name>Open Terminal Here</name>\n\t<unique-id>1-1</unique-id>\n</action>\n</actions>\n";
        let ours = thunar_action(
            Path::new("/t/tumble"),
            None,
            Format::by_id("png").unwrap(),
            FormatId("jpeg"),
        );
        let mut xml = users.to_string();
        xml.insert_str(xml.rfind("</actions>").unwrap(), &ours);
        assert!(xml.contains("tumble-png-jpeg"));
        assert_eq!(thunar_without_tumble(&xml), users);
        assert_eq!(thunar_without_tumble(users), users);
    }

    #[test]
    fn install_uninstall_round_trip() {
        let home = tempfile_dir();
        let places = Places { data: home.join("data"), config: home.join("config") };
        // Something the user already has: it must survive.
        let theirs = places.dolphin().join("their-menu.desktop");
        write(&theirs, "[Desktop Entry]\n", true).unwrap();
        let uca = places.thunar();
        write(
            &uca,
            "<actions>\n<action>\n\t<unique-id>1-1</unique-id>\n</action>\n</actions>\n",
            false,
        )
        .unwrap();

        let tumble = Path::new("/opt/Tumble app/tumble");
        let done = install(&places, tumble, None, &menus(), ALL).unwrap();
        assert_eq!(done.formats, 2);
        install(&places, tumble, None, &menus(), ALL).unwrap(); // again: idempotent

        let dolphin = fs::read_to_string(places.dolphin().join("tumble-jpeg.desktop")).unwrap();
        assert!(dolphin.contains("MimeType=image/jpeg;"), "{dolphin}");
        assert!(dolphin.contains("X-KDE-Submenu=Convert to"));
        assert!(
            dolphin.contains("Exec=\"/opt/Tumble app/tumble\" convert --to webp -- %F"),
            "{dolphin}"
        );
        let mode = fs::metadata(places.dolphin().join("tumble-jpeg.desktop"))
            .unwrap()
            .permissions()
            .mode();
        assert!(mode & 0o111 != 0, "Dolphin needs it executable");
        assert!(!places.dolphin().join("tumble-qoi.desktop").exists(), "no targets, no menu");
        let txt = fs::read_to_string(places.dolphin().join("tumble-txt.desktop")).unwrap();
        assert!(txt.contains("ExcludeServiceTypes="), "{txt}");

        let nemo = fs::read_to_string(places.nemo().join("tumble-jpeg-png.nemo_action")).unwrap();
        assert!(
            nemo.contains("Name=Convert to PNG") && nemo.contains("Extensions=jpg;jpeg;jfif;"),
            "{nemo}"
        );

        let xml = fs::read_to_string(&uca).unwrap();
        assert!(xml.contains("<unique-id>1-1</unique-id>"), "user's action kept");
        assert_eq!(xml.matches("<unique-id>tumble-").count(), 3, "{xml}");
        assert!(
            xml.contains("<patterns>*.jpg;*.JPG;*.jpeg;*.JPEG;*.jfif;*.JFIF</patterns>"),
            "{xml}"
        );

        let py = fs::read_to_string(places.nautilus_extension()).unwrap();
        assert!(py.contains("TUMBLE = \"/opt/Tumble app/tumble\""), "{py}");
        let script = places.nautilus_scripts().join("Convert to WebP");
        assert!(
            fs::read_to_string(&script)
                .unwrap()
                .contains("exec '/opt/Tumble app/tumble' convert --to webp")
        );

        let st = status(&places).unwrap();
        assert_eq!(st.managers, ALL);
        assert_eq!(st.tumble.unwrap().0, tumble);

        uninstall(&places).unwrap();
        assert!(theirs.is_file(), "user's menu kept");
        assert!(marked_files(&places.dolphin(), ".desktop").is_empty());
        assert!(marked_files(&places.nemo(), ".nemo_action").is_empty());
        assert_eq!(
            fs::read_to_string(&uca).unwrap(),
            "<actions>\n<action>\n\t<unique-id>1-1</unique-id>\n</action>\n</actions>\n"
        );
        assert!(!places.nautilus_extension().exists());
        assert!(!places.nautilus_scripts().exists());
        assert!(status(&places).unwrap().managers.is_empty());
        fs::remove_dir_all(&home).unwrap();
    }

    fn tempfile_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tumble-menu-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }
}
