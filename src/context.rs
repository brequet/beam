use thiserror::Error;

/// What the host currently has focused, read-only.
pub struct FocusInfo {
    pub title: String,
    pub process: String,
}

impl FocusInfo {
    /// The one display line the page prints: `"title (process)"`.
    pub fn display(&self) -> String {
        format!("{} ({})", self.title, self.process)
    }
}

#[derive(Debug, Error)]
pub enum ContextError {
    #[error("could not read host context: {0}")]
    Read(String),
}

/// Abstraction over host-side context awareness.
///
/// The web layer only knows this trait, so development (`--mock`) and tests
/// never read real host state.
pub trait ContextService: Send + Sync {
    /// The currently focused window, or `None` when the host reports none
    /// (locked screen, secure desktop).
    fn focused_window(&self) -> Result<Option<FocusInfo>, ContextError>;
}

/// Real backend reading the focused window from the OS.
///
/// Reads only — unlike [`crate::input::OsInput`] it never touches the host's
/// input state. The process is opened with
/// `PROCESS_QUERY_LIMITED_INFORMATION`, which Windows allows across
/// elevation boundaries, so even an elevated foreground window resolves.
pub struct OsContext;

#[cfg(target_os = "windows")]
mod os {
    use windows::core::PWSTR;
    use windows::Win32::Foundation::{CloseHandle, HWND};
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
    };

    use super::{ContextError, FocusInfo};

    /// The focused window, or `None` when the host reports none (locked
    /// screen, secure desktop).
    pub fn focused_window() -> Result<Option<FocusInfo>, ContextError> {
        let hwnd = unsafe { GetForegroundWindow() };
        if hwnd.is_invalid() {
            return Ok(None);
        }

        Ok(Some(FocusInfo {
            title: window_title(hwnd)?,
            process: window_process_name(hwnd)?,
        }))
    }

    fn window_title(hwnd: HWND) -> Result<String, ContextError> {
        let length = unsafe { GetWindowTextLengthW(hwnd) };
        if length < 0 {
            return Err(ContextError::Read(
                "could not read the window title".to_owned(),
            ));
        }

        let mut buffer = vec![0u16; length as usize + 1];
        let copied = (unsafe { GetWindowTextW(hwnd, &mut buffer) }.max(0)) as usize;
        Ok(String::from_utf16_lossy(
            &buffer[..copied.min(buffer.len() - 1)],
        ))
    }

    fn window_process_name(hwnd: HWND) -> Result<String, ContextError> {
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
        if pid == 0 {
            return Err(ContextError::Read(
                "could not identify the focused window's process".to_owned(),
            ));
        }

        let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }
            .map_err(|error| {
                ContextError::Read(format!("could not open the focused process: {error}"))
            })?;

        let mut buffer = [0u16; 1024];
        let mut len = buffer.len() as u32;
        let image = unsafe {
            QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, PWSTR(buffer.as_mut_ptr()), &mut len)
        };
        let _ = unsafe { CloseHandle(handle) };

        let image = image
            .map(|()| String::from_utf16_lossy(&buffer[..len as usize]))
            .map_err(|error| {
                ContextError::Read(format!("could not read the focused process's image path: {error}"))
            })?;

        std::path::Path::new(&image)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .ok_or_else(|| ContextError::Read("the focused process has no image name".to_owned()))
    }
}

#[cfg(target_os = "windows")]
impl ContextService for OsContext {
    fn focused_window(&self) -> Result<Option<FocusInfo>, ContextError> {
        os::focused_window()
    }
}

#[cfg(not(target_os = "windows"))]
impl ContextService for OsContext {
    fn focused_window(&self) -> Result<Option<FocusInfo>, ContextError> {
        Err(ContextError::Read(
            "focus awareness is Windows-only".to_owned(),
        ))
    }
}

/// Dev/test backend answering with a canned sample instead of the real host.
pub struct MockContext;

impl ContextService for MockContext {
    fn focused_window(&self) -> Result<Option<FocusInfo>, ContextError> {
        Ok(Some(FocusInfo {
            title: "S01E03 — Netflix — Brave".to_owned(),
            process: "brave.exe".to_owned(),
        }))
    }
}

/// One home for turning a [`ContextService`] result into the line the page
/// prints: the display string, a "nothing is focused" note, or the error.
pub fn focus_line(result: Result<Option<FocusInfo>, ContextError>) -> String {
    match result {
        Ok(Some(info)) => info.display(),
        Ok(None) => "nothing is focused".to_owned(),
        Err(error) => error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_answers_a_canned_sample_without_touching_the_os() {
        let focus = MockContext.focused_window().unwrap().unwrap();
        assert_eq!(focus.title, "S01E03 — Netflix — Brave");
        assert_eq!(focus.process, "brave.exe");
        assert_eq!(focus.display(), "S01E03 — Netflix — Brave (brave.exe)");
    }

    #[test]
    fn focus_line_covers_every_outcome_shape() {
        let info = FocusInfo {
            title: "t".to_owned(),
            process: "p".to_owned(),
        };
        assert_eq!(focus_line(Ok(Some(info))), "t (p)");
        assert_eq!(focus_line(Ok(None)), "nothing is focused");
        assert_eq!(
            focus_line(Err(ContextError::Read("boom".to_owned()))),
            "could not read host context: boom"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn os_backend_reads_the_real_focus_without_touching_input_state() {
        let focus = OsContext.focused_window().unwrap();
        if let Some(info) = focus {
            assert!(!info.process.is_empty(), "process name must resolve");
        }
    }
}
