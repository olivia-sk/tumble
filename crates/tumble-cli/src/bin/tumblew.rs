//! The right-click menu's entry point. Built for the Windows GUI subsystem so
//! Explorer never flashes a console window, and kept tiny: it starts
//! `tumble.exe` from the same folder with no window, passes the arguments
//! through unchanged, and returns its exit code. All conversion code lives
//! in `tumble.exe`, so it is shipped once (PRD section 13 size budget).
#![windows_subsystem = "windows"]

use std::process::{Command, ExitCode, Stdio};
use tumble_core::brand;

fn main() -> ExitCode {
    let Ok(me) = std::env::current_exe() else {
        return ExitCode::from(2);
    };
    let cli = me.with_file_name(format!("{}{}", brand::CLI_BIN, std::env::consts::EXE_SUFFIX));
    let mut command = Command::new(cli);
    command
        .args(std::env::args_os().skip(1))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    match command.status() {
        Ok(status) => ExitCode::from(status.code().and_then(|c| u8::try_from(c).ok()).unwrap_or(1)),
        Err(_) => ExitCode::from(3),
    }
}
