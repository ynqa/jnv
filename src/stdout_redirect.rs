use std::{
    fs::File,
    io::{self, IsTerminal},
};

use anyhow::anyhow;

#[cfg(unix)]
use rustix::{
    io::dup,
    stdio::{dup2_stdout, stdout},
};
#[cfg(unix)]
use std::os::fd::OwnedFd;

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
pub(crate) struct StdoutRedirect {
    #[cfg(unix)]
    saved_stdout: Option<OwnedFd>,
}

impl StdoutRedirect {
    pub(crate) fn try_new_for_tui(write_to_stdout: bool) -> anyhow::Result<Self> {
        if !write_to_stdout || io::stdout().is_terminal() {
            return Ok(Self {
                #[cfg(unix)]
                saved_stdout: None,
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
            })
        }

        #[cfg(not(unix))]
        {
            Err(anyhow!(
                "`--write-to-stdout` with piped stdout is not supported on this platform"
            ))
        }
    }

    pub(crate) fn restore(&mut self) -> anyhow::Result<()> {
        #[cfg(unix)]
        if let Some(saved_stdout) = self.saved_stdout.take() {
            dup2_stdout(&saved_stdout).map_err(|e| anyhow!("Failed to restore stdout: {e}"))?;
        }

        Ok(())
    }
}

impl Drop for StdoutRedirect {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}
