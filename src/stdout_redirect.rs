use std::io::{self, IsTerminal};

use anyhow::anyhow;

#[cfg(unix)]
use rustix::{
    io::dup,
    stdio::{dup2_stdout, stdout},
};
#[cfg(unix)]
use std::fs::File;
#[cfg(unix)]
use std::os::fd::OwnedFd;
#[cfg(windows)]
use std::{
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    ptr,
};
#[cfg(windows)]
use windows_sys::Win32::{
    Foundation::{DuplicateHandle, GENERIC_READ, GENERIC_WRITE, INVALID_HANDLE_VALUE},
    Storage::FileSystem::{
        CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    },
    System::{
        Console::{GetStdHandle, SetStdHandle, STD_OUTPUT_HANDLE},
        Threading::GetCurrentProcess,
    },
};

/// Redirects `stdout` to the controlling TTY while the TUI is running.
///
/// This is needed when `--write-to-stdout` is used with piped stdout, e.g.
/// `cat data.json | jnv --write-to-stdout | pbcopy`.
/// During interactive rendering, cursor controls and screen output must go to
/// a terminal, not to the downstream pipe.
///
/// Unix flow:
/// 1. Save current `fd=1` with `dup` (`saved_stdout`).
/// 2. Replace `fd=1` with `/dev/tty` via `dup2_stdout` for TUI rendering.
/// 3. Restore the original `fd=1` on exit.
///
/// After restore, writing to `io::stdout()` again goes to the original pipe
/// (for example `pbcopy`) so the final JSON can be emitted there.
///
/// Note:
/// `stdout` is file descriptor 1 (FD 1), not the screen itself.
/// Its destination is chosen by the shell when the process starts:
/// terminal (`cmd`), file (`cmd > out.txt`), or pipe (`cmd | next`).
/// Therefore, we cannot just write to `stdout` for TUI rendering when it's piped.
/// Instead, we must write directly to the terminal device (`/dev/tty` on Unix,
/// `CONOUT$` on Windows).
pub struct StdoutRedirect {
    #[cfg(unix)]
    saved_stdout: Option<OwnedFd>,
    #[cfg(windows)]
    saved_stdout: Option<OwnedHandle>,
    #[cfg(windows)]
    tty_stdout: Option<OwnedHandle>,
}

impl StdoutRedirect {
    pub fn try_new_for_tui(write_to_stdout: bool) -> anyhow::Result<Self> {
        if !write_to_stdout || io::stdout().is_terminal() {
            return Ok(Self {
                #[cfg(unix)]
                saved_stdout: None,
                #[cfg(windows)]
                saved_stdout: None,
                #[cfg(windows)]
                tty_stdout: None,
            });
        }

        #[cfg(unix)]
        {
            let tty = File::options()
                .read(true)
                .write(true)
                .open("/dev/tty")
                .map_err(|e| anyhow!("Failed to open /dev/tty for TUI rendering: {e}"))?;

            let saved_fd = dup(stdout()).map_err(|e| anyhow!("Failed to duplicate stdout: {e}"))?;
            dup2_stdout(&tty).map_err(|e| anyhow!("Failed to redirect stdout to /dev/tty: {e}"))?;

            Ok(Self {
                saved_stdout: Some(saved_fd),
                #[cfg(windows)]
                tty_stdout: None,
            })
        }

        #[cfg(windows)]
        {
            let saved_stdout = duplicate_stdout_handle()
                .map_err(|e| anyhow!("Failed to duplicate stdout: {e}"))?;
            let tty_stdout = open_conout()
                .map_err(|e| anyhow!("Failed to open CONOUT$ for TUI rendering: {e}"))?;

            set_process_stdout(&tty_stdout)
                .map_err(|e| anyhow!("Failed to redirect stdout to CONOUT$: {e}"))?;

            Ok(Self {
                saved_stdout: Some(saved_stdout),
                tty_stdout: Some(tty_stdout),
            })
        }

        #[cfg(not(any(unix, windows)))]
        {
            Err(anyhow!(
                "`--write-to-stdout` with piped stdout is not supported on this platform"
            ))
        }
    }

    pub fn restore(&mut self) -> anyhow::Result<()> {
        #[cfg(unix)]
        if let Some(saved_stdout) = self.saved_stdout.take() {
            dup2_stdout(&saved_stdout).map_err(|e| anyhow!("Failed to restore stdout: {e}"))?;
        }
        #[cfg(windows)]
        if let Some(saved_stdout) = self.saved_stdout.take() {
            set_process_stdout(&saved_stdout)
                .map_err(|e| anyhow!("Failed to restore stdout: {e}"))?;
            self.tty_stdout.take();
        }

        Ok(())
    }
}

impl Drop for StdoutRedirect {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

#[cfg(windows)]
fn duplicate_stdout_handle() -> io::Result<OwnedHandle> {
    unsafe {
        let source = GetStdHandle(STD_OUTPUT_HANDLE);
        if source == 0 || source == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }

        let process = GetCurrentProcess();
        let mut duplicated = 0;
        let ok = DuplicateHandle(
            process,
            source,
            process,
            &mut duplicated,
            0,
            0,
            windows_sys::Win32::Foundation::DUPLICATE_SAME_ACCESS,
        );
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }

        Ok(OwnedHandle::from_raw_handle(duplicated as _))
    }
}

#[cfg(windows)]
fn open_conout() -> io::Result<OwnedHandle> {
    let mut conout: Vec<u16> = "CONOUT$".encode_utf16().collect();
    conout.push(0);

    unsafe {
        let handle = CreateFileW(
            conout.as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            ptr::null(),
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            ptr::null_mut(),
        );
        if handle == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }

        Ok(OwnedHandle::from_raw_handle(handle as _))
    }
}

#[cfg(windows)]
fn set_process_stdout(handle: &OwnedHandle) -> io::Result<()> {
    unsafe {
        let ok = SetStdHandle(STD_OUTPUT_HANDLE, handle.as_raw_handle() as _);
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}
