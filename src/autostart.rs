//! Autostart support: register beam as a logon task so it starts with the
//! user session (required for keystroke injection — a Windows Service in
//! session 0 cannot reach the focused window) without a visible console.
//!
//! Windows implementation: a Task Scheduler task with an "at log on" trigger
//! scoped to the current user. Standard users may create such a self-scoped
//! task without elevation (unlike `schtasks /sc onlogon`, which needs admin).
//! Both operations are idempotent and safe to re-run.

use std::path::Path;

use anyhow::Context;

/// Settings frozen into the logon task by `beam install`.
#[derive(Debug, Clone, Default)]
pub struct InstallOptions {
    pub host: Option<String>,
    pub port: Option<u16>,
    pub mock: bool,
}

const TASK_NAME: &str = "beam";

#[cfg(windows)]
pub fn install(options: &InstallOptions) -> anyhow::Result<()> {
    let exe = std::env::current_exe().context("locating the running executable")?;
    let dir = exe
        .parent()
        .context("locating the executable directory")?
        .to_string_lossy()
        .into_owned();
    let task_args = task_arguments(options);

    if exe.components().any(|c| c.as_os_str() == "debug") {
        println!(
            "note: registering a debug build; a release build is recommended for everyday use"
        );
    }

    let outcome = run_powershell(&install_script(&exe, &dir, &task_args))?;
    let mut words = outcome.splitn(2, ' ');
    match words.next() {
        Some("installed") | Some("updated") => {}
        _ => anyhow::bail!("unexpected Task Scheduler response: {outcome:?}"),
    }
    let previous = words.next().filter(|_| outcome.starts_with("updated "));

    println!("registered logon task '{TASK_NAME}'");
    if let Some(old) = previous {
        println!("  replaced previous entry that pointed at: {old}");
    }
    println!("  runs at sign-in: {} {}", exe.display(), task_args);
    if let Some(log) = status_log_path() {
        println!("  status log:      {}", log.display());
    }
    println!("  start now with:  schtasks /run /tn {TASK_NAME}");
    println!("  remove with:     beam uninstall");
    Ok(())
}

#[cfg(windows)]
pub fn uninstall() -> anyhow::Result<()> {
    match run_powershell(&uninstall_script())?.as_str() {
        "removed" => {
            println!("removed logon task '{TASK_NAME}' (a task-started beam was stopped too)")
        }
        "not-installed" => println!("beam is not registered as a logon task"),
        other => anyhow::bail!("unexpected Task Scheduler response: {other:?}"),
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn install(_options: &InstallOptions) -> anyhow::Result<()> {
    anyhow::bail!("`beam install` is Windows-only right now (macOS/Linux: see docs/IDEAS.md)")
}

#[cfg(not(windows))]
pub fn uninstall() -> anyhow::Result<()> {
    anyhow::bail!("`beam uninstall` is Windows-only right now (macOS/Linux: see docs/IDEAS.md)")
}

/// Hides the console window this process owns (no-op when there is none, or
/// on platforms where the autostart task is not implemented yet).
#[cfg(windows)]
pub fn hide_console() {
    const SW_HIDE: i32 = 0;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetConsoleWindow() -> *mut core::ffi::c_void;
    }
    #[link(name = "user32")]
    unsafe extern "system" {
        fn ShowWindow(hwnd: *mut core::ffi::c_void, command: i32) -> i32;
    }

    unsafe {
        let console = GetConsoleWindow();
        if !console.is_null() {
            ShowWindow(console, SW_HIDE);
        }
    }
}

#[cfg(not(windows))]
pub fn hide_console() {}

/// Records the current run's status line to `%LOCALAPPDATA%\beam\beam.log`.
///
/// Only useful with `--hidden`, where stdout is invisible; each run replaces
/// the file, so it always describes the latest run.
#[cfg(windows)]
pub fn log_status(message: &str) {
    let Some(path) = status_log_path() else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let _ = std::fs::write(path, format!("[{secs}] {message}\n"));
}

#[cfg(not(windows))]
pub fn log_status(_message: &str) {}

#[cfg(windows)]
pub fn status_log_path() -> Option<std::path::PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .map(|base| std::path::PathBuf::from(base).join("beam").join("beam.log"))
}

/// The exact command line the logon task will run: `--hidden` plus whatever
/// options the user froze in at install time.
fn task_arguments(options: &InstallOptions) -> String {
    let mut parts = vec!["--hidden".to_owned()];
    if let Some(host) = &options.host {
        parts.push("--host".to_owned());
        parts.push(host.clone());
    }
    if let Some(port) = options.port {
        parts.push("--port".to_owned());
        parts.push(port.to_string());
    }
    if options.mock {
        parts.push("--mock".to_owned());
    }
    parts.join(" ")
}

/// Single-quoted PowerShell string literal; embedded quotes are doubled.
fn ps_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

#[cfg(windows)]
fn install_script(exe: &Path, dir: &str, args: &str) -> String {
    format!(
        r#"
$ErrorActionPreference = 'Stop'
$name = {name}
$exe = {exe}
$taskArgs = {args}
$dir = {dir}
$user = "$env:USERDOMAIN\$env:USERNAME"
$action = New-ScheduledTaskAction -Execute $exe -Argument $taskArgs -WorkingDirectory $dir
$trigger = New-ScheduledTaskTrigger -AtLogOn -User $user
$principal = New-ScheduledTaskPrincipal -UserId $user -LogonType Interactive -RunLevel Limited
$settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit ([TimeSpan]::Zero) -MultipleInstances IgnoreNew
$existing = Get-ScheduledTask -TaskName $name -ErrorAction SilentlyContinue
if ($existing) {{
    $old = @($existing.Actions)[0].Execute
    Set-ScheduledTask -TaskName $name -Action $action -Trigger $trigger -Settings $settings | Out-Null
    if ($old -ieq $exe) {{ Write-Output 'updated' }} else {{ Write-Output "updated $old" }}
}} else {{
    Register-ScheduledTask -TaskName $name -Action $action -Trigger $trigger -Principal $principal -Settings $settings -Description 'beam: phone-keyboard bridge, starts at logon' | Out-Null
    Write-Output 'installed'
}}
"#,
        name = ps_quote(TASK_NAME),
        exe = ps_quote(&exe.to_string_lossy()),
        args = ps_quote(args),
        dir = ps_quote(dir),
    )
}

#[cfg(windows)]
fn uninstall_script() -> String {
    format!(
        r#"
$ErrorActionPreference = 'Stop'
$name = {name}
$existing = Get-ScheduledTask -TaskName $name -ErrorAction SilentlyContinue
if ($existing) {{
    Stop-ScheduledTask -TaskName $name -ErrorAction SilentlyContinue
    Unregister-ScheduledTask -TaskName $name -Confirm:$false
    Write-Output 'removed'
}} else {{
    Write-Output 'not-installed'
}}
"#,
        name = ps_quote(TASK_NAME),
    )
}

/// Runs a script through Windows PowerShell with everything passed as a
/// base64-encoded UTF-16 command, so path quoting cannot go wrong.
#[cfg(windows)]
fn run_powershell(script: &str) -> anyhow::Result<String> {
    let encoded = utf16le_base64(script);
    let output = std::process::Command::new("powershell.exe")
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-EncodedCommand",
            &encoded,
        ])
        .output()
        .context("running powershell.exe")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let mut message = format!("Task Scheduler operation failed: {}", stderr.trim());
        let lowered = stderr.to_lowercase();
        if lowered.contains("access") && lowered.contains("denied") {
            message.push_str(
                "\nhint: the task could not be created from this shell; try an elevated terminal",
            );
        }
        anyhow::bail!("{message}");
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// Base64 of the UTF-16LE encoding, the payload format of
/// `powershell -EncodedCommand`.
fn utf16le_base64(text: &str) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes: Vec<u8> = text.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = u32::from(chunk[0]);
        let b1 = u32::from(chunk.get(1).copied().unwrap_or(0));
        let b2 = u32::from(chunk.get(2).copied().unwrap_or(0));
        let n = (b0 << 16) | (b1 << 8) | b2;
        const SIX: u32 = 0x3f;
        out.push(TABLE[(n >> 18 & SIX) as usize] as char);
        out.push(TABLE[(n >> 12 & SIX) as usize] as char);
        out.push(if chunk.len() > 1 {
            TABLE[(n >> 6 & SIX) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[(n & SIX) as usize] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16le_base64_matches_powershell_encoding() {
        // Reference: [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes('hi'))
        assert_eq!(utf16le_base64("hi"), "aABpAA==");
        assert_eq!(utf16le_base64(""), "");
    }

    #[test]
    fn ps_quote_escapes_single_quotes() {
        assert_eq!(
            ps_quote("C:\\Program Files\\beam.exe"),
            "'C:\\Program Files\\beam.exe'"
        );
        assert_eq!(ps_quote("it's"), "'it''s'");
    }

    #[test]
    fn task_arguments_snapshot_options() {
        assert_eq!(task_arguments(&InstallOptions::default()), "--hidden");
        assert_eq!(
            task_arguments(&InstallOptions {
                host: Some("127.0.0.1".to_owned()),
                port: Some(8080),
                mock: true,
            }),
            "--hidden --host 127.0.0.1 --port 8080 --mock"
        );
    }

    #[cfg(windows)]
    #[test]
    fn install_script_scopes_the_task_to_this_user() {
        let script = install_script(Path::new("C:\\apps\\beam.exe"), "C:\\apps", "--hidden");
        assert!(script.contains("-AtLogOn -User"), "{script}");
        assert!(script.contains("'C:\\apps\\beam.exe'"), "{script}");
        assert!(
            script.contains("ExecutionTimeLimit ([TimeSpan]::Zero)"),
            "{script}"
        );
        assert!(script.contains("Register-ScheduledTask"), "{script}");
        assert!(script.contains("Set-ScheduledTask"), "{script}");
    }
}
