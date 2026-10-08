//! Running a program so that it and everything it starts can be killed
//! together. LibreOffice's `soffice.exe` starts `soffice.bin`; killing
//! only the first would leave the second running.
//!
//! On Windows the child starts suspended, joins a Job Object, then resumes,
//! so even its earliest children are in the job. Dropping the tree (or
//! calling `kill`) ends the whole job.

use std::io;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

pub struct Tree {
    child: Child,
    #[cfg(windows)]
    job: win::Job,
}

pub enum Waited {
    /// Ended on its own. Callers check outputs rather than the exit code,
    /// which LibreOffice does not set reliably.
    Exited,
    TimedOut,
    Cancelled,
}

impl Tree {
    pub fn spawn(mut command: Command) -> io::Result<Tree> {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_SUSPENDED: u32 = 0x0000_0004;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_SUSPENDED | CREATE_NO_WINDOW);
            let mut child = command.spawn()?;
            let job = match win::Job::new().and_then(|job| {
                job.assign(&child)?;
                win::resume(child.id())?;
                Ok(job)
            }) {
                Ok(job) => job,
                Err(e) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(e);
                }
            };
            Ok(Tree { child, job })
        }
        #[cfg(not(windows))]
        {
            Ok(Tree { child: command.spawn()? })
        }
    }

    /// Waits for the program, polling `cancelled` and giving up after
    /// `timeout`. The tree is killed on timeout or cancel.
    pub fn wait(mut self, timeout: Duration, cancelled: impl Fn() -> bool) -> io::Result<Waited> {
        let start = Instant::now();
        loop {
            if let Some(status) = self.child.try_wait()? {
                let _ = status;
                return Ok(Waited::Exited);
            }
            if cancelled() {
                self.kill();
                return Ok(Waited::Cancelled);
            }
            if start.elapsed() >= timeout {
                self.kill();
                return Ok(Waited::TimedOut);
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    pub fn kill(&mut self) {
        #[cfg(windows)]
        self.job.terminate();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        if let Ok(None) = self.child.try_wait() {
            self.kill();
        }
    }
}

#[cfg(windows)]
mod win {
    use std::io;
    use std::os::windows::io::AsRawHandle;
    use std::process::Child;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
    };
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject, TerminateJobObject,
    };
    use windows_sys::Win32::System::Threading::{OpenThread, ResumeThread, THREAD_SUSPEND_RESUME};

    /// A Job Object that kills its processes when closed.
    pub struct Job(HANDLE);

    // SAFETY: a job handle may be used from any thread.
    unsafe impl Send for Job {}

    impl Job {
        pub fn new() -> io::Result<Job> {
            // SAFETY: plain Win32 calls; the handle is closed in Drop.
            unsafe {
                let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if handle.is_null() {
                    return Err(io::Error::last_os_error());
                }
                let job = Job(handle);
                let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                let ok = SetInformationJobObject(
                    job.0,
                    JobObjectExtendedLimitInformation,
                    (&info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                    size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                );
                if ok == 0 {
                    return Err(io::Error::last_os_error());
                }
                Ok(job)
            }
        }

        pub fn assign(&self, child: &Child) -> io::Result<()> {
            // SAFETY: both handles are valid for the call.
            let ok = unsafe { AssignProcessToJobObject(self.0, child.as_raw_handle() as HANDLE) };
            if ok == 0 { Err(io::Error::last_os_error()) } else { Ok(()) }
        }

        pub fn terminate(&self) {
            // SAFETY: valid job handle.
            unsafe { TerminateJobObject(self.0, 1) };
        }
    }

    impl Drop for Job {
        fn drop(&mut self) {
            // SAFETY: closing kills any processes still in the job.
            unsafe { CloseHandle(self.0) };
        }
    }

    /// Resumes every thread of a process started with CREATE_SUSPENDED.
    pub fn resume(pid: u32) -> io::Result<()> {
        // SAFETY: Toolhelp snapshot walk; every handle opened is closed.
        unsafe {
            let snap = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
            if snap == INVALID_HANDLE_VALUE {
                return Err(io::Error::last_os_error());
            }
            let mut entry: THREADENTRY32 = std::mem::zeroed();
            entry.dwSize = size_of::<THREADENTRY32>() as u32;
            let mut resumed = 0;
            let mut more = Thread32First(snap, &mut entry) != 0;
            while more {
                if entry.th32OwnerProcessID == pid {
                    let thread = OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID);
                    if !thread.is_null() {
                        ResumeThread(thread);
                        CloseHandle(thread);
                        resumed += 1;
                    }
                }
                more = Thread32Next(snap, &mut entry) != 0;
            }
            CloseHandle(snap);
            if resumed == 0 {
                return Err(io::Error::other("could not resume the started process"));
            }
            Ok(())
        }
    }
}
