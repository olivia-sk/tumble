//! The right-click menu on macOS: one Finder Quick Action, "Convert with
//! Tumble", in `~/Library/Services/Convert with Tumble.workflow`. It runs
//! `tumble pick -- <files>`, which asks which format to convert to (listing
//! only the formats every selected file can become) and then converts like
//! the Windows menu does. Finder cannot nest a submenu of formats under a
//! Quick Action, hence the list.
//!
//! The workflow is an Automator document with one "Run Shell Script" step.
//! While it runs, Finder shows a gear in the menu bar whose stop button
//! ends the job.

use crate::unix::shell_quote;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use tumble_core::brand;

/// The Quick Action's name in Finder.
pub const ACTION: &str = "Convert with Tumble";
/// Marks the workflow as Tumble's, so uninstall never removes anything else.
const MARK: &str = "Tumble menu entry";

pub struct Places {
    /// `~/Library/Services`.
    pub services: PathBuf,
    /// Where the install record goes (`~/Library/Application Support/Tumble`).
    pub data: PathBuf,
}

impl Places {
    pub fn from_env() -> Option<Places> {
        let home = std::env::var_os("HOME").map(PathBuf::from).filter(|p| p.is_absolute())?;
        Some(Places {
            services: home.join("Library/Services"),
            data: home.join("Library/Application Support").join(brand::DATA_DIR),
        })
    }

    pub fn workflow(&self) -> PathBuf {
        self.services.join(format!("{ACTION}.workflow"))
    }

    fn record(&self) -> PathBuf {
        self.data.join("menu.txt")
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// The shell script the Quick Action runs, with the selected files as its
/// arguments. `exec` makes the menu bar's stop button reach tumble itself.
pub fn script(tumble: &Path) -> String {
    format!(
        "# {MARK}, removed by `tumble menu uninstall`.\nexec {} pick -- \"$@\"\n",
        shell_quote(&tumble.to_string_lossy())
    )
}

/// `Contents/Info.plist`: offers the action in Finder for any file.
fn info_plist() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>NSServices</key>
	<array>
		<dict>
			<key>NSBackgroundColorName</key>
			<string>background</string>
			<key>NSIconName</key>
			<string>NSActionTemplate</string>
			<key>NSMenuItem</key>
			<dict>
				<key>default</key>
				<string>{}</string>
			</dict>
			<key>NSMessage</key>
			<string>runWorkflowAsService</string>
			<key>NSRequiredContext</key>
			<dict>
				<key>NSApplicationIdentifier</key>
				<string>com.apple.finder</string>
			</dict>
			<key>NSSendFileTypes</key>
			<array>
				<string>public.item</string>
			</array>
		</dict>
	</array>
</dict>
</plist>
"#,
        xml_escape(ACTION)
    )
}

/// One entry of the Run Shell Script action's `arguments` table.
fn argument(index: u32, name: &str, default: &str, kind: &str) -> String {
    format!(
        "\t\t\t\t\t<key>{index}</key>\n\t\t\t\t\t<dict>\n\t\t\t\t\t\t<key>default value</key>\n\t\t\t\t\t\t{default}\n\
         \t\t\t\t\t\t<key>name</key>\n\t\t\t\t\t\t<string>{name}</string>\n\t\t\t\t\t\t<key>required</key>\n\
         \t\t\t\t\t\t<string>0</string>\n\t\t\t\t\t\t<key>type</key>\n\t\t\t\t\t\t<string>{kind}</string>\n\
         \t\t\t\t\t\t<key>uuid</key>\n\t\t\t\t\t\t<string>{index}</string>\n\t\t\t\t\t</dict>\n"
    )
}

/// `Contents/document.wflow`: a Quick Action that receives files in Finder
/// and passes them to a shell script as arguments (`inputMethod` 1).
fn document(tumble: &Path) -> String {
    let arguments = [
        argument(0, "inputMethod", "<integer>0</integer>", "0"),
        argument(1, "source", "<string></string>", "0"),
        argument(2, "CheckedForUserDefaultShell", "<false/>", "0"),
        argument(3, "COMMAND_STRING", "<string></string>", "0"),
        argument(4, "shell", "<string></string>", "0"),
    ]
    .concat();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>AMApplicationBuild</key>
	<string>523</string>
	<key>AMApplicationVersion</key>
	<string>2.10</string>
	<key>AMDocumentVersion</key>
	<string>2</string>
	<key>actions</key>
	<array>
		<dict>
			<key>action</key>
			<dict>
				<key>AMAccepts</key>
				<dict>
					<key>Container</key>
					<string>List</string>
					<key>Optional</key>
					<true/>
					<key>Types</key>
					<array>
						<string>com.apple.cocoa.string</string>
					</array>
				</dict>
				<key>AMActionVersion</key>
				<string>2.0.3</string>
				<key>AMApplication</key>
				<array>
					<string>Automator</string>
				</array>
				<key>AMParameterProperties</key>
				<dict>
					<key>COMMAND_STRING</key>
					<dict/>
					<key>CheckedForUserDefaultShell</key>
					<dict/>
					<key>inputMethod</key>
					<dict/>
					<key>shell</key>
					<dict/>
					<key>source</key>
					<dict/>
				</dict>
				<key>AMProvides</key>
				<dict>
					<key>Container</key>
					<string>List</string>
					<key>Types</key>
					<array>
						<string>com.apple.cocoa.string</string>
					</array>
				</dict>
				<key>ActionBundlePath</key>
				<string>/System/Library/Automator/Run Shell Script.action</string>
				<key>ActionName</key>
				<string>Run Shell Script</string>
				<key>ActionParameters</key>
				<dict>
					<key>COMMAND_STRING</key>
					<string>{command}</string>
					<key>CheckedForUserDefaultShell</key>
					<true/>
					<key>inputMethod</key>
					<integer>1</integer>
					<key>shell</key>
					<string>/bin/sh</string>
					<key>source</key>
					<string></string>
				</dict>
				<key>BundleIdentifier</key>
				<string>com.apple.RunShellScript</string>
				<key>CFBundleVersion</key>
				<string>2.0.3</string>
				<key>CanShowSelectedItemsWhenRun</key>
				<false/>
				<key>CanShowWhenRun</key>
				<true/>
				<key>Category</key>
				<array>
					<string>AMCategoryUtilities</string>
				</array>
				<key>Class Name</key>
				<string>RunShellScriptAction</string>
				<key>InputUUID</key>
				<string>6B1A9C3E-2F4D-4E8A-9B7C-1D2E3F4A5B60</string>
				<key>Keywords</key>
				<array>
					<string>Shell</string>
					<string>Script</string>
					<string>Command</string>
					<string>Run</string>
					<string>Unix</string>
				</array>
				<key>OutputUUID</key>
				<string>6B1A9C3E-2F4D-4E8A-9B7C-1D2E3F4A5B61</string>
				<key>UUID</key>
				<string>6B1A9C3E-2F4D-4E8A-9B7C-1D2E3F4A5B62</string>
				<key>UnlocalizedApplications</key>
				<array>
					<string>Automator</string>
				</array>
				<key>arguments</key>
				<dict>
{arguments}				</dict>
				<key>isViewVisible</key>
				<integer>1</integer>
				<key>location</key>
				<string>309.000000:253.000000</string>
				<key>nibPath</key>
				<string>/System/Library/Automator/Run Shell Script.action/Contents/Resources/Base.lproj/main.nib</string>
			</dict>
			<key>isViewVisible</key>
			<integer>1</integer>
		</dict>
	</array>
	<key>connectors</key>
	<dict/>
	<key>workflowMetaData</key>
	<dict>
		<key>applicationBundleID</key>
		<string>com.apple.finder</string>
		<key>applicationBundleIDsByPath</key>
		<dict>
			<key>/System/Library/CoreServices/Finder.app</key>
			<string>com.apple.finder</string>
		</dict>
		<key>applicationPath</key>
		<string>/System/Library/CoreServices/Finder.app</string>
		<key>applicationPaths</key>
		<array>
			<string>/System/Library/CoreServices/Finder.app</string>
		</array>
		<key>inputTypeIdentifier</key>
		<string>com.apple.Automator.fileSystemObject</string>
		<key>outputTypeIdentifier</key>
		<string>com.apple.Automator.nothing</string>
		<key>presentationMode</key>
		<integer>15</integer>
		<key>processesInput</key>
		<integer>0</integer>
		<key>serviceApplicationBundleID</key>
		<string>com.apple.finder</string>
		<key>serviceApplicationPath</key>
		<string>/System/Library/CoreServices/Finder.app</string>
		<key>serviceInputTypeIdentifier</key>
		<string>com.apple.Automator.fileSystemObject</string>
		<key>serviceOutputTypeIdentifier</key>
		<string>com.apple.Automator.nothing</string>
		<key>serviceProcessesInput</key>
		<integer>0</integer>
		<key>systemImageName</key>
		<string>NSActionTemplate</string>
		<key>useAutomaticInputType</key>
		<integer>0</integer>
		<key>workflowTypeIdentifier</key>
		<string>com.apple.Automator.servicesMenu</string>
	</dict>
</dict>
</plist>
"#,
        command = xml_escape(&script(tumble)),
    )
}

/// Tells macOS the Services menu changed. Never fails the install.
fn refresh() {
    let pbs = Path::new("/System/Library/CoreServices/pbs");
    if pbs.is_file() {
        let _ = std::process::Command::new(pbs)
            .arg("-update")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }
}

/// Writes the Quick Action. Removes any earlier install first, so running
/// it again is safe.
pub fn install(places: &Places, tumble: &Path) -> io::Result<()> {
    uninstall(places)?;
    let contents = places.workflow().join("Contents");
    fs::create_dir_all(&contents)?;
    fs::write(contents.join("Info.plist"), info_plist())?;
    fs::write(contents.join("document.wflow"), document(tumble))?;
    fs::create_dir_all(&places.data)?;
    fs::write(places.record(), format!("tumble={}\n", tumble.display()))?;
    refresh();
    Ok(())
}

/// Removes Tumble's Quick Action, and nothing else.
pub fn uninstall(places: &Places) -> io::Result<()> {
    let workflow = places.workflow();
    let ours = fs::read_to_string(workflow.join("Contents/document.wflow"))
        .is_ok_and(|t| t.contains(MARK));
    if ours {
        fs::remove_dir_all(&workflow)?;
        refresh();
    }
    match fs::remove_file(places.record()) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

pub struct Status {
    pub installed: bool,
    /// The tumble the menu runs, and whether it still exists.
    pub tumble: Option<(PathBuf, bool)>,
}

pub fn status(places: &Places) -> io::Result<Status> {
    let installed = fs::read_to_string(places.workflow().join("Contents/document.wflow"))
        .is_ok_and(|t| t.contains(MARK));
    let tumble = fs::read_to_string(places.record()).ok().and_then(|text| {
        text.lines().find_map(|l| l.strip_prefix("tumble=")).map(|p| {
            let path = PathBuf::from(p);
            let exists = path.is_file();
            (path, exists)
        })
    });
    Ok(Status { installed, tumble })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workflow_runs_tumble_pick_with_the_files() {
        let doc =
            document(Path::new("/Users/jo/Applications/Tumble & Co.app/Contents/MacOS/tumble"));
        assert!(doc.contains(
            "exec '/Users/jo/Applications/Tumble &amp; Co.app/Contents/MacOS/tumble' pick -- &quot;$@&quot;"
        ), "{doc}");
        assert!(doc.contains("<string>com.apple.Automator.servicesMenu</string>"));
        assert!(info_plist().contains("<string>Convert with Tumble</string>"));
    }

    #[test]
    fn install_uninstall_round_trip() {
        let home = std::env::temp_dir().join(format!("tumble-qa-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&home);
        let places = Places { services: home.join("Services"), data: home.join("Tumble") };
        let theirs = places.services.join("Mine.workflow/Contents/document.wflow");
        fs::create_dir_all(theirs.parent().unwrap()).unwrap();
        fs::write(&theirs, "x").unwrap();

        let tumble = Path::new("/Applications/Tumble.app/Contents/MacOS/tumble");
        install(&places, tumble).unwrap();
        install(&places, tumble).unwrap();
        assert!(places.workflow().join("Contents/Info.plist").is_file());
        let st = status(&places).unwrap();
        assert!(st.installed);
        assert_eq!(st.tumble.unwrap().0, tumble);

        uninstall(&places).unwrap();
        assert!(!places.workflow().exists());
        assert!(theirs.is_file(), "user's workflow kept");
        assert!(!status(&places).unwrap().installed);
        fs::remove_dir_all(&home).unwrap();
    }
}
