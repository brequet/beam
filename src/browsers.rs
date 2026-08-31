use std::path::PathBuf;
use std::sync::Mutex;

use thiserror::Error;

/// Whether a known browser's DevTools channel can be driven on this machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CdpSupport {
    /// The `--remote-debugging-port` flag works on the default profile
    /// (Brave: the 136+ hardening is Google-branding-gated).
    Yes,
    /// Chromium 136+ ignores the flag on default data dirs; driving would
    /// need a custom `--user-data-dir` (Chrome, Edge — deferred).
    Degraded,
    /// No CDP at all; keystrokes only (Firefox, until BiDi).
    No,
}

impl CdpSupport {
    /// Whether a live DevTools port could belong to this browser.
    fn drivable(self) -> bool {
        matches!(self, CdpSupport::Yes | CdpSupport::Degraded)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct BrowserDef {
    /// Process image name as Windows reports it.
    pub process: &'static str,
    /// Human label the page shows.
    pub label: &'static str,
    pub cdp: CdpSupport,
    /// Candidate exe paths, standard install first, per-user second;
    /// `{ProgramFiles}` / `{LocalAppData}` expand from the environment.
    /// Empty for browsers beam cannot launch yet (Chrome/Edge deferred,
    /// Firefox has no CDP to enable).
    pub install_paths: &'static [&'static str],
}

impl BrowserDef {
    /// The known browser with this process image name, if any
    /// (case-insensitive; the catalogue is the single source of truth).
    pub fn by_process(name: &str) -> Option<&'static BrowserDef> {
        BROWSERS
            .iter()
            .find(|def| def.process.eq_ignore_ascii_case(name))
    }
}

/// The known browsers, brave first: when several are running, table order
/// decides who a live CDP port belongs to (see [`attributed_states`]).
pub const BROWSERS: &[BrowserDef] = &[
    BrowserDef {
        process: "brave.exe",
        label: "Brave",
        cdp: CdpSupport::Yes,
        install_paths: &[
            r"{ProgramFiles}\BraveSoftware\Brave-Browser\Application\brave.exe",
            r"{LocalAppData}\BraveSoftware\Brave-Browser\Application\brave.exe",
        ],
    },
    BrowserDef {
        process: "chrome.exe",
        label: "Chrome",
        cdp: CdpSupport::Degraded,
        install_paths: &[],
    },
    BrowserDef {
        process: "msedge.exe",
        label: "Edge",
        cdp: CdpSupport::Degraded,
        install_paths: &[],
    },
    BrowserDef {
        process: "firefox.exe",
        label: "Firefox",
        cdp: CdpSupport::No,
        install_paths: &[],
    },
];

/// Ports probed for a live DevTools endpoint: beam's own launch port first,
/// then the conventional 9222 recognizing a user-launched CDP browser.
/// Never extended with a non-loopback address — the port is unauthenticated
/// by design.
pub const CDP_PROBE_PORTS: &[u16] = &[9223, 9222];

/// The port beam launches browsers with — the first probe port.
pub const BEAM_CDP_PORT: u16 = CDP_PROBE_PORTS[0];

/// One running browser: which it is, and whether a DevTools port answered.
/// A browser absent from [`BrowserService::detect`] is not running; its
/// onboarding action is what [`onboarding_action`] offers instead.
#[derive(Debug, PartialEq, Eq)]
pub struct BrowserInfo {
    pub def: &'static BrowserDef,
    pub cdp_port: Option<u16>,
}

impl BrowserInfo {
    /// The one display line the page prints, matching the onboarding table
    /// in docs/HANDOFF-BROWSER-MANAGER.md.
    pub fn display(&self) -> String {
        match self.cdp_port {
            Some(port) => format!("{} — remote control active (port {port})", self.def.label),
            None => format!("{} — running, remote control off", self.def.label),
        }
    }
}

#[derive(Debug, Error)]
pub enum BrowserError {
    #[error("could not read browser state: {0}")]
    Detect(String),
    #[error("could not control the browser: {0}")]
    Act(String),
}

/// Abstraction over host-side browser awareness and control.
///
/// The web layer only knows this trait, so development (`--mock`) and tests
/// never read real host state. Detection carries no cache: a probe answers
/// fresh on every call, because the port only exists while the browser that
/// opened it does. Start/restart end in a verification probe, so an `Ok`
/// means the endpoint is already answering.
pub trait BrowserService: Send + Sync {
    /// The known browsers currently running, in table order (brave first).
    fn detect(&self) -> Result<Vec<BrowserInfo>, BrowserError>;

    /// Cold-starts the browser with beam's DevTools port and verifies the
    /// endpoint answers. Refuses when the browser is already running: a
    /// running instance's ProcessSingleton silently drops the flag.
    fn start(&self, def: &'static BrowserDef) -> Result<BrowserInfo, BrowserError>;

    /// Gracefully closes the browser, force-stops leftovers, cold-starts it
    /// with beam's DevTools port, and verifies. Refuses when remote control
    /// is already active — the restart would only close a working browser.
    fn restart(&self, def: &'static BrowserDef) -> Result<BrowserInfo, BrowserError>;
}

/// One home for turning a [`BrowserService`] result into the line the page
/// prints: the first running browser (table order makes brave the headline),
/// a note when none runs, or the error.
pub fn browser_line(result: Result<Vec<BrowserInfo>, BrowserError>) -> String {
    match result {
        Ok(infos) => infos
            .first()
            .map(BrowserInfo::display)
            .unwrap_or_else(|| "no known browser running".to_owned()),
        Err(error) => error.to_string(),
    }
}

/// The one onboarding action the page offers for the headline browser —
/// the button IS the consent (docs/HANDOFF-BROWSER-MANAGER.md).
#[derive(Debug, PartialEq, Eq)]
pub enum OnboardingAction {
    /// Nothing running: cold-start it with remote control.
    Start { process: &'static str },
    /// Running without CDP: restart into remote control, at the cost of a
    /// browser close (the warning is the consent copy).
    Restart {
        process: &'static str,
        warning: &'static str,
    },
    /// Remote control is up: no action, the deep remote takes over later.
    Active,
    /// Detection failed (non-Windows host): say so, offer nothing.
    Unavailable,
}

/// Whether beam can launch this browser at all. Chrome/Edge/Firefox are
/// display-only today: Edge's startup boost keeps msedge.exe processes
/// running with no window in sight, and a browser beam cannot relaunch must
/// never surface a restart button that could only ever fail.
fn startable(def: &BrowserDef) -> bool {
    !def.install_paths.is_empty()
}

/// Derives the onboarding action from the detected state, from the same
/// `detect()` result the headline line prints.
pub fn onboarding_action(result: &Result<Vec<BrowserInfo>, BrowserError>) -> OnboardingAction {
    match result {
        Ok(infos) => match infos.iter().find(|info| startable(info.def)) {
            Some(info) => match info.cdp_port {
                Some(_) => OnboardingAction::Active,
                None => OnboardingAction::Restart {
                    process: info.def.process,
                    warning: "Tabs restore, unsaved work is lost.",
                },
            },
            None => OnboardingAction::Start {
                process: BROWSERS[0].process,
            },
        },
        Err(_) => OnboardingAction::Unavailable,
    }
}

/// The known browsers among the given process names, in table order and
/// deduplicated: a browser runs as many processes but is one browser.
fn running_browsers(process_names: &[String]) -> Vec<&'static BrowserDef> {
    BROWSERS
        .iter()
        .filter(|def| {
            process_names
                .iter()
                .any(|name| name.eq_ignore_ascii_case(def.process))
        })
        .collect()
}

/// Attributes a live CDP port, if any, to the first running CDP-capable
/// browser: the probe says *a* devtools port is up, not which process owns
/// it, so table order decides.
fn attributed_states(defs: &[&'static BrowserDef], cdp_port: Option<u16>) -> Vec<BrowserInfo> {
    let mut attributed = false;
    defs.iter()
        .copied()
        .map(|def| {
            let cdp_port = cdp_port.filter(|_| !attributed && def.cdp.drivable());
            if cdp_port.is_some() {
                attributed = true;
            }
            BrowserInfo { def, cdp_port }
        })
        .collect()
}

/// The first existing install path for a browser: standard install first,
/// per-user install second; registry lookup deliberately deferred.
fn resolve_exe(def: &BrowserDef) -> Result<PathBuf, BrowserError> {
    if def.install_paths.is_empty() {
        return Err(unstartable_error(def));
    }
    for template in def.install_paths {
        let path = expand_install_path(template, &program_files_dir(), &local_app_data_dir());
        if path.exists() {
            return Ok(path);
        }
    }
    Err(not_installed_error(def))
}

/// Expands exactly the two known tokens; anything else passes through
/// untouched (no general environment expansion near launcher arguments).
fn expand_install_path(template: &str, program_files: &str, local_app_data: &str) -> PathBuf {
    PathBuf::from(
        template
            .replace("{ProgramFiles}", program_files)
            .replace("{LocalAppData}", local_app_data),
    )
}

fn program_files_dir() -> String {
    std::env::var("ProgramFiles").unwrap_or_else(|_| r"C:\Program Files".to_owned())
}

fn local_app_data_dir() -> String {
    std::env::var("LOCALAPPDATA").unwrap_or_else(|_| String::new())
}

fn already_active_error(def: &BrowserDef, port: u16) -> BrowserError {
    BrowserError::Act(format!(
        "{} already has remote control active on port {port} — nothing to do",
        def.label
    ))
}

fn running_without_cdp_error(def: &BrowserDef) -> BrowserError {
    BrowserError::Act(format!(
        "{} is already running without remote control — use Restart with remote control (tabs restore, unsaved work is lost)",
        def.label
    ))
}

fn unstartable_error(def: &BrowserDef) -> BrowserError {
    BrowserError::Act(format!("{} cannot be started by beam yet", def.label))
}

fn not_installed_error(def: &BrowserDef) -> BrowserError {
    BrowserError::Act(format!(
        "{} is not installed in a known location",
        def.label
    ))
}

/// Real backend reading the OS process table and probing DevTools ports.
///
/// Reads only during detection; start/restart act on the browser process as
/// the onboarding buttons describe (close, relaunch with the port).
pub struct OsBrowser;

#[cfg(target_os = "windows")]
// The restart flow's timings: graceful close gets the longest chance (the
// browser may be mid-shutdown), the force stop is quick, and the endpoint
// verification covers a cold browser start.
const CLOSE_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(10_000);
#[cfg(target_os = "windows")]
const FORCE_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(3_000);
#[cfg(target_os = "windows")]
const VERIFY_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(15_000);

#[cfg(target_os = "windows")]
impl BrowserService for OsBrowser {
    fn detect(&self) -> Result<Vec<BrowserInfo>, BrowserError> {
        let names = os::running_process_names()?;
        let defs = running_browsers(&names);
        // Probe only when some running browser could own the port.
        let port = if defs.iter().any(|def| def.cdp.drivable()) {
            os::cdp_port()
        } else {
            None
        };
        Ok(attributed_states(&defs, port))
    }

    fn start(&self, def: &'static BrowserDef) -> Result<BrowserInfo, BrowserError> {
        if let Some(info) = self.running(def)? {
            return Err(match info.cdp_port {
                Some(port) => already_active_error(def, port),
                None => running_without_cdp_error(def),
            });
        }
        // Fail fast on a missing exe while nothing has been closed yet.
        let exe = resolve_exe(def)?;
        os::launch_with_cdp(&exe)?;
        os::wait_for_cdp(def.label, VERIFY_TIMEOUT)?;
        Ok(BrowserInfo {
            def,
            cdp_port: Some(BEAM_CDP_PORT),
        })
    }

    fn restart(&self, def: &'static BrowserDef) -> Result<BrowserInfo, BrowserError> {
        if let Some(info) = self.running(def)?
            && let Some(port) = info.cdp_port
        {
            return Err(already_active_error(def, port));
        }
        // Fail fast on a missing exe while nothing has been closed yet.
        let exe = resolve_exe(def)?;
        // The validated restart flow: graceful close (WM_CLOSE), then treat
        // "closed" as "process tree fully exited" — background mode keeps
        // processes holding the ProcessSingleton after the last window goes.
        os::graceful_close(def.process)?;
        os::wait_until_gone(def.process, CLOSE_TIMEOUT);
        os::force_stop(def.process)?;
        if !os::wait_until_gone(def.process, FORCE_TIMEOUT) {
            return Err(BrowserError::Act(format!(
                "{} did not fully exit even after the force stop",
                def.label
            )));
        }
        os::launch_with_cdp(&exe)?;
        os::wait_for_cdp(def.label, VERIFY_TIMEOUT)?;
        Ok(BrowserInfo {
            def,
            cdp_port: Some(BEAM_CDP_PORT),
        })
    }
}

#[cfg(target_os = "windows")]
impl OsBrowser {
    /// The detected state of one browser, or `None` when not running.
    fn running(&self, def: &BrowserDef) -> Result<Option<BrowserInfo>, BrowserError> {
        Ok(self
            .detect()?
            .into_iter()
            .find(|info| info.def.process == def.process))
    }
}

#[cfg(not(target_os = "windows"))]
impl BrowserService for OsBrowser {
    fn detect(&self) -> Result<Vec<BrowserInfo>, BrowserError> {
        Err(BrowserError::Detect(
            "browser awareness is Windows-only".to_owned(),
        ))
    }

    fn start(&self, _def: &'static BrowserDef) -> Result<BrowserInfo, BrowserError> {
        Err(BrowserError::Act(
            "browser control is Windows-only".to_owned(),
        ))
    }

    fn restart(&self, _def: &'static BrowserDef) -> Result<BrowserInfo, BrowserError> {
        Err(BrowserError::Act(
            "browser control is Windows-only".to_owned(),
        ))
    }
}

/// Dev/test backend that fakes the browser state machine and records the
/// actions, so the whole onboarding flow is testable under `--mock`.
///
/// Starts in the onboarding's most interesting state: Brave running without
/// remote control, so the page offers the restart flow.
pub struct MockBrowser {
    state: Mutex<MockState>,
    pub events: Mutex<Vec<String>>,
}

/// The faked states, mirroring the onboarding table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MockState {
    // Only tests start from here; the mock's default state is running.
    #[allow(dead_code)]
    NotRunning,
    Running {
        cdp: Option<u16>,
    },
}

impl Default for MockBrowser {
    fn default() -> Self {
        Self::new()
    }
}

impl MockBrowser {
    pub fn new() -> Self {
        Self::with_state(MockState::Running { cdp: None })
    }

    pub fn with_state(state: MockState) -> Self {
        Self {
            state: Mutex::new(state),
            events: Mutex::new(Vec::new()),
        }
    }

    fn record(&self, event: String) {
        println!("[mock browser] {event}");
        self.events
            .lock()
            .expect("mock browser mutex poisoned")
            .push(event);
    }

    fn set_state(&self, state: MockState) {
        *self.state.lock().expect("mock browser mutex poisoned") = state;
    }
}

impl BrowserService for MockBrowser {
    fn detect(&self) -> Result<Vec<BrowserInfo>, BrowserError> {
        Ok(
            match *self.state.lock().expect("mock browser mutex poisoned") {
                MockState::NotRunning => vec![],
                MockState::Running { cdp } => vec![BrowserInfo {
                    def: &BROWSERS[0],
                    cdp_port: cdp,
                }],
            },
        )
    }

    fn start(&self, def: &'static BrowserDef) -> Result<BrowserInfo, BrowserError> {
        if def.install_paths.is_empty() {
            return Err(unstartable_error(def));
        }
        // Bound first: a match-scrutinee guard would still be held while an
        // arm body relocks the same mutex in set_state.
        let current = *self.state.lock().expect("mock browser mutex poisoned");
        match current {
            MockState::Running { cdp: Some(port) } => Err(already_active_error(def, port)),
            MockState::Running { cdp: None } => Err(running_without_cdp_error(def)),
            MockState::NotRunning => {
                self.record(format!("start {}", def.process));
                self.set_state(MockState::Running {
                    cdp: Some(BEAM_CDP_PORT),
                });
                Ok(BrowserInfo {
                    def,
                    cdp_port: Some(BEAM_CDP_PORT),
                })
            }
        }
    }

    fn restart(&self, def: &'static BrowserDef) -> Result<BrowserInfo, BrowserError> {
        if def.install_paths.is_empty() {
            return Err(unstartable_error(def));
        }
        let current = *self.state.lock().expect("mock browser mutex poisoned");
        if let MockState::Running { cdp: Some(port) } = current {
            return Err(already_active_error(def, port));
        }
        self.record(format!("restart {}", def.process));
        self.set_state(MockState::Running {
            cdp: Some(BEAM_CDP_PORT),
        });
        Ok(BrowserInfo {
            def,
            cdp_port: Some(BEAM_CDP_PORT),
        })
    }
}

#[cfg(target_os = "windows")]
mod os {
    use std::io::{Read, Write};
    use std::net::{SocketAddr, TcpStream};
    use std::path::Path;
    use std::process::Command;
    use std::time::{Duration, Instant};

    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS,
    };

    use super::{BEAM_CDP_PORT, BrowserError, CDP_PROBE_PORTS};

    const PROBE_TIMEOUT: Duration = Duration::from_millis(500);
    const MAX_PROBE_RESPONSE: usize = 8 * 1024;
    const POLL_STEP: Duration = Duration::from_millis(250);

    /// Every process image name on the system, via one tool-help snapshot.
    pub fn running_process_names() -> Result<Vec<String>, BrowserError> {
        let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }
            .map_err(|error| BrowserError::Detect(format!("could not list processes: {error}")))?;

        let mut names = Vec::new();
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        if unsafe { Process32FirstW(snapshot, &mut entry) }.is_ok() {
            loop {
                let len = entry
                    .szExeFile
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(entry.szExeFile.len());
                names.push(String::from_utf16_lossy(&entry.szExeFile[..len]));
                if unsafe { Process32NextW(snapshot, &mut entry) }.is_err() {
                    break;
                }
            }
        }
        let _ = unsafe { CloseHandle(snapshot) };
        Ok(names)
    }

    /// The first probed port whose DevTools endpoint answers, if any.
    ///
    /// A plain loopback `GET /json/version`: connection refused on a closed
    /// port is instant, and the short timeout bounds any firewalled stall.
    pub fn cdp_port() -> Option<u16> {
        CDP_PROBE_PORTS
            .iter()
            .copied()
            .find(|port| cdp_answers(*port))
    }

    fn cdp_answers(port: u16) -> bool {
        let address = SocketAddr::from(([127, 0, 0, 1], port));
        let Ok(mut stream) = TcpStream::connect_timeout(&address, PROBE_TIMEOUT) else {
            return false;
        };
        let _ = stream.set_read_timeout(Some(PROBE_TIMEOUT));
        let _ = stream.set_write_timeout(Some(PROBE_TIMEOUT));

        let request = format!(
            "GET /json/version HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"
        );
        if stream.write_all(request.as_bytes()).is_err() {
            return false;
        }

        // `Connection: close` makes Chromium end the response with EOF; the
        // cap keeps a misbehaving server from growing the buffer forever.
        let mut response = Vec::new();
        let mut chunk = [0u8; 1024];
        loop {
            match stream.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => {
                    response.extend_from_slice(&chunk[..n]);
                    if response.len() > MAX_PROBE_RESPONSE {
                        break;
                    }
                }
                Err(_) => return false,
            }
        }
        is_cdp_response(&response)
    }

    /// Whether raw bytes look like the `200` the DevTools endpoint answers with.
    fn is_cdp_response(raw: &[u8]) -> bool {
        let head = String::from_utf8_lossy(raw);
        head.lines().next().is_some_and(|line| {
            let mut parts = line.split_whitespace();
            let version_ok = parts.next().is_some_and(|version| {
                version.eq_ignore_ascii_case("HTTP/1.1") || version.eq_ignore_ascii_case("HTTP/1.0")
            });
            version_ok && parts.next().is_some_and(|code| code == "200")
        })
    }

    /// Posts WM_CLOSE to every window of the process (`taskkill /IM`, no
    /// /F): the graceful part of the restart flow. "Nothing to close" is
    /// fine, so the exit code is ignored.
    pub fn graceful_close(process: &str) -> Result<(), BrowserError> {
        close_process(process, false)
    }

    /// Force-stops leftovers (`taskkill /IM /F`): background mode keeps
    /// processes (and the ProcessSingleton) alive after the last window
    /// closes, and they would silently drop the relaunch flags.
    pub fn force_stop(process: &str) -> Result<(), BrowserError> {
        close_process(process, true)
    }

    fn close_process(process: &str, force: bool) -> Result<(), BrowserError> {
        let mut command = Command::new("taskkill");
        command.arg("/IM").arg(process);
        if force {
            command.arg("/F");
        }
        command
            .output()
            .map_err(|error| BrowserError::Act(format!("could not run taskkill: {error}")))?;
        Ok(())
    }

    /// Whether the process has fully exited from the process table within
    /// the timeout ("closed" means the whole tree is gone).
    pub fn wait_until_gone(process: &str, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        loop {
            if let Ok(names) = running_process_names()
                && !names.iter().any(|name| name.eq_ignore_ascii_case(process))
            {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(POLL_STEP);
        }
    }

    /// Cold-starts the browser with beam's DevTools port.
    pub fn launch_with_cdp(exe: &Path) -> Result<(), BrowserError> {
        Command::new(exe)
            .arg(format!("--remote-debugging-port={BEAM_CDP_PORT}"))
            .arg("--no-first-run")
            .arg("--no-default-browser-check")
            .spawn()
            .map_err(|error| {
                BrowserError::Act(format!("could not launch {}: {error}", exe.display()))
            })?;
        Ok(())
    }

    /// Polls beam's DevTools port until it answers or the timeout passes —
    /// the honest-error clause: say the browser may have dropped support,
    /// don't hang.
    pub fn wait_for_cdp(label: &str, timeout: Duration) -> Result<(), BrowserError> {
        let deadline = Instant::now() + timeout;
        loop {
            if cdp_answers(BEAM_CDP_PORT) {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(BrowserError::Act(format!(
                    "{label} started but remote control never answered on port {BEAM_CDP_PORT} — a browser update may have dropped debugging support"
                )));
            }
            std::thread::sleep(POLL_STEP);
        }
    }

    #[cfg(test)]
    mod tests {
        use super::is_cdp_response;

        #[test]
        fn only_a_real_200_counts_as_cdp() {
            assert!(is_cdp_response(
                b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{}"
            ));
            assert!(is_cdp_response(b"HTTP/1.0 200 OK\r\n\r\n"));
            assert!(!is_cdp_response(b"HTTP/1.1 404 Not Found\r\n\r\n"));
            assert!(!is_cdp_response(b"garbage"));
            assert!(!is_cdp_response(b""));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn running_browsers_follows_table_order_and_dedups() {
        let names = vec![
            "CHROME.EXE".to_owned(),
            "brave.exe".to_owned(),
            "brave.exe".to_owned(),
            "notepad.exe".to_owned(),
        ];
        let labels: Vec<_> = running_browsers(&names)
            .iter()
            .map(|def| def.label)
            .collect();
        assert_eq!(labels, ["Brave", "Chrome"]);
    }

    #[test]
    fn a_cdp_port_is_attributed_once_to_the_first_drivable_browser() {
        let (brave, chrome, firefox) = (&BROWSERS[0], &BROWSERS[1], &BROWSERS[3]);
        assert_eq!(
            attributed_states(&[brave, chrome], Some(9222)),
            vec![
                BrowserInfo {
                    def: brave,
                    cdp_port: Some(9222),
                },
                BrowserInfo {
                    def: chrome,
                    cdp_port: None,
                },
            ]
        );
        assert_eq!(
            attributed_states(&[firefox], Some(9222)),
            vec![BrowserInfo {
                def: firefox,
                cdp_port: None,
            }]
        );
        assert_eq!(
            attributed_states(&[brave], None),
            vec![BrowserInfo {
                def: brave,
                cdp_port: None,
            }]
        );
    }

    #[test]
    fn display_lines_cover_the_onboarding_states() {
        let brave = &BROWSERS[0];
        assert_eq!(
            BrowserInfo {
                def: brave,
                cdp_port: None,
            }
            .display(),
            "Brave — running, remote control off"
        );
        assert_eq!(
            BrowserInfo {
                def: brave,
                cdp_port: Some(9223),
            }
            .display(),
            "Brave — remote control active (port 9223)"
        );
    }

    #[test]
    fn browser_line_covers_every_outcome_shape() {
        let brave = &BROWSERS[0];
        assert_eq!(
            browser_line(Ok(vec![BrowserInfo {
                def: brave,
                cdp_port: Some(9223),
            }])),
            "Brave — remote control active (port 9223)"
        );
        assert_eq!(
            browser_line(Ok(vec![BrowserInfo {
                def: brave,
                cdp_port: None,
            }])),
            "Brave — running, remote control off"
        );
        assert_eq!(browser_line(Ok(vec![])), "no known browser running");
        assert_eq!(
            browser_line(Err(BrowserError::Detect("boom".to_owned()))),
            "could not read browser state: boom"
        );
        assert_eq!(
            browser_line(Err(BrowserError::Act("boom".to_owned()))),
            "could not control the browser: boom"
        );
    }

    #[test]
    fn by_process_is_case_insensitive_and_rejects_unknown() {
        assert_eq!(
            BrowserDef::by_process("BRAVE.EXE").map(|def| def.label),
            Some("Brave")
        );
        assert!(BrowserDef::by_process("notepad.exe").is_none());
    }

    #[test]
    fn onboarding_action_covers_every_detected_shape() {
        let brave = &BROWSERS[0];
        assert_eq!(
            onboarding_action(&Ok(vec![])),
            OnboardingAction::Start {
                process: "brave.exe"
            }
        );
        assert_eq!(
            onboarding_action(&Ok(vec![BrowserInfo {
                def: brave,
                cdp_port: None,
            }])),
            OnboardingAction::Restart {
                process: "brave.exe",
                warning: "Tabs restore, unsaved work is lost.",
            }
        );
        assert_eq!(
            onboarding_action(&Ok(vec![BrowserInfo {
                def: brave,
                cdp_port: Some(9222),
            }])),
            OnboardingAction::Active
        );
        assert_eq!(
            onboarding_action(&Err(BrowserError::Detect("boom".to_owned()))),
            OnboardingAction::Unavailable
        );
    }

    #[test]
    fn onboarding_offers_no_buttons_for_browsers_beam_cannot_start() {
        let (brave, edge) = (&BROWSERS[0], &BROWSERS[2]);
        // Edge's startup boost keeps processes running with no window in
        // sight — they must not surface a button that could never work.
        assert_eq!(
            onboarding_action(&Ok(vec![BrowserInfo {
                def: edge,
                cdp_port: None,
            }])),
            OnboardingAction::Start {
                process: "brave.exe"
            }
        );
        assert_eq!(
            onboarding_action(&Ok(vec![BrowserInfo {
                def: edge,
                cdp_port: Some(9222),
            }])),
            OnboardingAction::Start {
                process: "brave.exe"
            }
        );
        // The startable headline wins over non-startable runners.
        assert_eq!(
            onboarding_action(&Ok(vec![
                BrowserInfo {
                    def: brave,
                    cdp_port: None,
                },
                BrowserInfo {
                    def: edge,
                    cdp_port: None,
                },
            ])),
            OnboardingAction::Restart {
                process: "brave.exe",
                warning: "Tabs restore, unsaved work is lost.",
            }
        );
    }

    #[test]
    fn expand_install_path_expands_known_tokens_only() {
        assert_eq!(
            expand_install_path(
                r"{ProgramFiles}\X\x.exe",
                r"C:\Program Files",
                r"C:\Users\u\AppData\Local"
            ),
            Path::new(r"C:\Program Files\X\x.exe")
        );
        assert_eq!(
            expand_install_path(r"{LocalAppData}\X\x.exe", "", r"C:\Users\u\AppData\Local"),
            Path::new(r"C:\Users\u\AppData\Local\X\x.exe")
        );
        assert_eq!(
            expand_install_path(r"C:\Static\x.exe", "", ""),
            Path::new(r"C:\Static\x.exe")
        );
    }

    #[test]
    fn mock_answers_its_faked_state_without_touching_the_os() {
        let mock = MockBrowser::new();
        let infos = mock.detect().unwrap();
        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].def.process, "brave.exe");
        // Starts CDP-off so the page offers the restart flow.
        assert_eq!(infos[0].cdp_port, None);
    }

    #[test]
    fn mock_start_then_restart_drive_the_full_flow_and_record() {
        // Start: nothing running → cold start with remote control.
        let mock = MockBrowser::with_state(MockState::NotRunning);
        let info = mock.start(&BROWSERS[0]).unwrap();
        assert_eq!(info.cdp_port, Some(BEAM_CDP_PORT));
        assert_eq!(mock.detect().unwrap()[0].cdp_port, Some(BEAM_CDP_PORT));
        assert_eq!(*mock.events.lock().unwrap(), vec!["start brave.exe"]);
        // A further restart now refuses: the browser is already remote-controlled.
        assert!(mock.restart(&BROWSERS[0]).is_err());

        // Restart: running without remote control → the restart flow.
        let mock = MockBrowser::new();
        let info = mock.restart(&BROWSERS[0]).unwrap();
        assert_eq!(info.cdp_port, Some(BEAM_CDP_PORT));
        assert_eq!(*mock.events.lock().unwrap(), vec!["restart brave.exe"]);
    }

    #[test]
    fn mock_start_refuses_every_already_running_shape() {
        let mock = MockBrowser::with_state(MockState::Running {
            cdp: Some(BEAM_CDP_PORT),
        });
        let error = mock.start(&BROWSERS[0]).unwrap_err().to_string();
        assert!(
            error.contains("already has remote control active"),
            "{error}"
        );

        let mock = MockBrowser::new();
        let error = mock.start(&BROWSERS[0]).unwrap_err().to_string();
        assert!(error.contains("use Restart with remote control"), "{error}");
    }

    #[test]
    fn mock_restart_refuses_when_already_active_but_starts_when_closed() {
        let mock = MockBrowser::with_state(MockState::Running {
            cdp: Some(BEAM_CDP_PORT),
        });
        let error = mock.restart(&BROWSERS[0]).unwrap_err().to_string();
        assert!(
            error.contains("already has remote control active"),
            "{error}"
        );

        let mock = MockBrowser::with_state(MockState::NotRunning);
        mock.restart(&BROWSERS[0]).unwrap();
        assert_eq!(mock.detect().unwrap()[0].cdp_port, Some(BEAM_CDP_PORT));
    }

    #[test]
    fn mock_mirrors_the_real_deferred_browsers() {
        let mock = MockBrowser::new();
        let error = mock.start(&BROWSERS[1]).unwrap_err().to_string();
        assert!(error.contains("cannot be started by beam yet"), "{error}");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn os_backend_detects_without_touching_anything() {
        // Whatever runs on this machine, detection must answer, not fail.
        let detected = OsBrowser.detect().expect("process enumeration must work");
        for info in &detected {
            assert!(
                BROWSERS.iter().any(|def| def.process == info.def.process),
                "only known browsers may be reported"
            );
        }
    }
}
