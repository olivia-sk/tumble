//! The LibreOffice engine: Word, Excel and PowerPoint formats, their open
//! equivalents, text and HTML, among themselves and to PDF. Documents reach
//! images through PDF and PDFium (two routed steps), one image per page.
//!
//! Runs the user's own `soffice.exe` headless, each job with a throwaway
//! profile (`profile.rs`), and kills the whole LibreOffice process tree if
//! a file takes longer than the timeout (120 s, or `TUMBLE_SOFFICE_TIMEOUT`
//! seconds).

pub mod filters;
mod profile;

use crate::process_tree::{Tree, Waited};
use crate::tools::Tool;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;
use tumble_core::job::ScratchDir;
use tumble_core::{
    CancelToken, ConvertOptions, Engine, EngineError, FormatId, Progress, Step, brand,
};

pub const INSTALL_HINT: &str = "winget install TheDocumentFoundation.LibreOffice";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);

const SOFFICE: Tool = Tool { exe: "soffice.exe", env: "SOFFICE" };

fn known_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![
        PathBuf::from(r"C:\Program Files\LibreOffice\program"),
        PathBuf::from(r"C:\Program Files (x86)\LibreOffice\program"),
    ];
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        dirs.push(PathBuf::from(local).join(r"Programs\LibreOffice\program"));
    }
    dirs
}

pub fn soffice() -> Option<&'static PathBuf> {
    static FOUND: OnceLock<Option<PathBuf>> = OnceLock::new();
    FOUND
        .get_or_init(|| {
            SOFFICE.find(tumble_core::config::current().tools.soffice.as_deref(), &known_dirs())
        })
        .as_ref()
}

fn timeout() -> Duration {
    std::env::var(brand::env_var("SOFFICE_TIMEOUT"))
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|s| *s > 0.0)
        .map_or(DEFAULT_TIMEOUT, Duration::from_secs_f64)
}

pub struct OfficeEngine;

impl OfficeEngine {
    pub fn describe() -> (bool, String) {
        match soffice() {
            None => (false, format!("soffice.exe not found; install with: {INSTALL_HINT}")),
            Some(path) => (true, format!("LibreOffice ({})", path.display())),
        }
    }
}

impl Engine for OfficeEngine {
    fn name(&self) -> &'static str {
        "libreoffice"
    }

    fn available(&self) -> bool {
        soffice().is_some()
    }

    fn steps(&self) -> Vec<Step> {
        filters::pairs().into_iter().map(|(from, to)| Step::new(from, to)).collect()
    }

    fn convert(
        &self,
        step: Step,
        input: &Path,
        output: &Path,
        _options: &ConvertOptions,
        progress: &dyn Progress,
        cancel: &CancelToken,
    ) -> Result<Vec<PathBuf>, EngineError> {
        let soffice = soffice().ok_or_else(|| {
            EngineError::failed(format!("LibreOffice is not installed ({INSTALL_HINT})"))
        })?;
        let (from, to) = (step.from.as_str(), step.to.as_str());

        // LibreOffice names its output after the input and reads any
        // argument starting with '-' as an option, so it gets a copy with a
        // plain name in a scratch folder.
        let scratch = ScratchDir::new_in(&std::env::temp_dir(), "tumble-office-")?;
        let work = scratch.path().join(format!("in.{}", ext(step.from)));
        std::fs::copy(input, &work)?;
        let out_dir = scratch.path().join("out");
        let profile = profile::prepare(soffice, &scratch.path().join("profile"));
        progress.update(0.1);

        let mut command = std::process::Command::new(soffice);
        command.arg(format!("-env:UserInstallation={profile}")).args([
            "--headless",
            "--invisible",
            "--norestore",
            "--nologo",
            "--nodefault",
            "--nolockcheck",
        ]);
        if let Some(filter) = filters::import(from) {
            command.arg(format!("--infilter={filter}"));
        }
        command
            .args(["--convert-to", filters::export(to), "--outdir"])
            .arg(&out_dir)
            .arg(&work)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());

        let limit = timeout();
        let waited = Tree::spawn(command)
            .map_err(|e| EngineError::failed(format!("cannot start LibreOffice: {e}")))?
            .wait(limit, || cancel.is_cancelled())?;
        match waited {
            Waited::Cancelled => return Err(EngineError::Cancelled),
            Waited::TimedOut => {
                return Err(EngineError::failed(format!(
                    "LibreOffice took longer than {} s and was stopped",
                    limit.as_secs()
                )));
            }
            Waited::Exited => {}
        }
        let produced = out_dir.join(format!("in.{}", ext(step.to)));
        if !produced.is_file() {
            return Err(EngineError::failed(format!(
                "LibreOffice could not convert this file to {}",
                step.to.format().name
            )));
        }
        // The scratch folder may be on another drive: copy, not rename.
        std::fs::copy(&produced, output)?;
        progress.update(1.0);
        Ok(vec![output.to_path_buf()])
    }
}

fn ext(id: FormatId) -> &'static str {
    id.format().primary_extension()
}
