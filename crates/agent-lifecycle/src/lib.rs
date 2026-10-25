use anyhow::{Context, Result, bail};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::sync::atomic::{AtomicU64, Ordering};

const MANAGED_MARKER: &str = "managed-by-native-system-monitor";
static NEXT_TEMP_FILE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartupMode {
    SystemdUser,
    XdgAutostart,
    LaunchAgent,
    WindowsLogonTask,
}

impl StartupMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::SystemdUser => "systemd --user",
            Self::XdgAutostart => "autostart XDG (sem restart automático)",
            Self::LaunchAgent => "LaunchAgent do usuário",
            Self::WindowsLogonTask => "tarefa de logon do usuário",
        }
    }
}

pub fn register_for_user_login(agent_executable: &Path) -> Result<StartupMode> {
    let agent_path = validate_agent_path(agent_executable)?;
    platform::register(&agent_path)
}

pub fn ensure_started(agent_executable: &Path) -> Result<StartupMode> {
    let agent_path = validate_agent_path(agent_executable)?;
    platform::ensure_started(&agent_path)
}

fn validate_agent_path(path: &Path) -> Result<PathBuf> {
    if !path.is_absolute() {
        bail!("agent executable path must be absolute");
    }
    if !path.is_file() {
        bail!("agent executable does not exist: {}", path.display());
    }

    let canonical_path = path
        .canonicalize()
        .with_context(|| format!("resolve agent executable path {}", path.display()))?;
    let text = canonical_path
        .to_str()
        .context("agent executable path is not valid UTF-8")?;
    if text.chars().any(char::is_control) {
        bail!("agent executable path contains unsupported control characters");
    }

    Ok(canonical_path)
}

fn write_managed_file(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create user startup directory {}", parent.display()))?;
    }

    match fs::read_to_string(path) {
        Ok(existing) => {
            if !existing.contains(MANAGED_MARKER) {
                bail!(
                    "refusing to replace an unmanaged startup file at {}",
                    path.display()
                );
            }
            if existing == contents {
                return Ok(());
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error).with_context(|| format!("read startup file {}", path.display()));
        }
    }

    let temporary_path = temporary_path(path);
    let write_result = write_new_file(&temporary_path, contents);
    if let Err(error) = write_result {
        let _ = fs::remove_file(&temporary_path);
        return Err(error).with_context(|| format!("write startup file {}", path.display()));
    }

    #[cfg(windows)]
    if path.exists() {
        fs::remove_file(path)
            .with_context(|| format!("replace managed startup file {}", path.display()))?;
    }

    match fs::rename(&temporary_path, path) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = fs::remove_file(&temporary_path);
            Err(error).with_context(|| {
                format!(
                    "install startup file {} from {}",
                    path.display(),
                    temporary_path.display()
                )
            })
        }
    }
}

fn temporary_path(destination: &Path) -> PathBuf {
    let sequence = NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed);
    let file_name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("startup");
    destination.with_file_name(format!(
        ".{file_name}.{}.{}.tmp",
        std::process::id(),
        sequence
    ))
}

fn write_new_file(path: &Path, contents: &str) -> Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }

    let mut file = options
        .open(path)
        .with_context(|| format!("create temporary startup file {}", path.display()))?;
    file.write_all(contents.as_bytes())
        .with_context(|| format!("write temporary startup file {}", path.display()))?;
    file.sync_all()
        .with_context(|| format!("flush temporary startup file {}", path.display()))?;
    Ok(())
}

fn require_success(output: &Output, action: &str) -> Result<()> {
    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let detail = if stderr.trim().is_empty() {
        stdout.trim()
    } else {
        stderr.trim()
    };
    if detail.is_empty() {
        bail!("{action} failed with status {}", output.status);
    }

    bail!("{action} failed: {detail}");
}

#[cfg(any(target_os = "macos", windows))]
fn xml_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(character),
        }
    }
    escaped
}

#[cfg(target_os = "linux")]
mod platform {
    use super::{Result, StartupMode, require_success, write_managed_file};
    use anyhow::{Context, anyhow};
    use std::env;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};

    const SERVICE_NAME: &str = "native-monitor-agent.service";
    const SERVICE_MARKER: &str = "# managed-by-native-system-monitor";
    const DESKTOP_MARKER: &str = "X-NativeMonitor-Managed=managed-by-native-system-monitor";

    pub fn register(agent_path: &Path) -> Result<StartupMode> {
        configure(agent_path, false)
    }

    pub fn ensure_started(agent_path: &Path) -> Result<StartupMode> {
        configure(agent_path, true)
    }

    fn configure(agent_path: &Path, start_now: bool) -> Result<StartupMode> {
        if user_systemd_available() {
            let config_home = config_home()?;
            let unit_path = config_home.join("systemd").join("user").join(SERVICE_NAME);
            let unit_contents = systemd_unit(agent_path)?;
            write_managed_file(&unit_path, &unit_contents)?;

            run_systemctl(&["--user", "daemon-reload"], "reload systemd --user units")?;
            if start_now {
                run_systemctl(
                    &["--user", "enable", "--now", SERVICE_NAME],
                    "enable and start the per-user agent",
                )?;
            } else {
                run_systemctl(
                    &["--user", "enable", SERVICE_NAME],
                    "enable the per-user agent at login",
                )?;
            }

            return Ok(StartupMode::SystemdUser);
        }

        let autostart_path = config_home()?
            .join("autostart")
            .join("native-monitor-agent.desktop");
        let desktop_contents = autostart_entry(agent_path)?;
        write_managed_file(&autostart_path, &desktop_contents)?;
        if start_now {
            let mut detached = Command::new("setsid");
            detached
                .arg("--")
                .arg(agent_path)
                .arg("run")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            match detached.spawn() {
                Ok(_child) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    Command::new(agent_path)
                        .arg("run")
                        .stdin(Stdio::null())
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .spawn()
                        .context("start the agent through the XDG autostart fallback")?;
                }
                Err(error) => {
                    return Err(error)
                        .context("detach the agent through the XDG autostart fallback");
                }
            }
        }

        Ok(StartupMode::XdgAutostart)
    }

    fn user_systemd_available() -> bool {
        Command::new("systemctl")
            .args(["--user", "show-environment"])
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    fn run_systemctl(arguments: &[&str], action: &str) -> Result<()> {
        let output = Command::new("systemctl")
            .args(arguments)
            .output()
            .with_context(|| format!("run systemctl to {action}"))?;
        require_success(&output, action)
    }

    fn config_home() -> Result<PathBuf> {
        if let Some(value) = env::var_os("XDG_CONFIG_HOME") {
            let path = PathBuf::from(value);
            if path.is_absolute() {
                return Ok(path);
            }
            return Err(anyhow!("XDG_CONFIG_HOME must be an absolute path"));
        }

        let home = env::var_os("HOME").context("HOME is not set")?;
        let path = PathBuf::from(home);
        if !path.is_absolute() {
            return Err(anyhow!("HOME must be an absolute path"));
        }
        Ok(path.join(".config"))
    }

    fn systemd_unit(agent_path: &Path) -> Result<String> {
        let agent = agent_path
            .to_str()
            .context("agent path is not valid UTF-8")?;
        let executable = systemd_quote(agent)?;
        Ok(format!(
            "{SERVICE_MARKER}\n\
             [Unit]\n\
             Description=Native System Monitor Agent (per-user)\n\
             StartLimitIntervalSec=60s\n\
             StartLimitBurst=5\n\
             \n\
             [Service]\n\
             Type=simple\n\
             ExecStart={executable} run\n\
             Restart=on-failure\n\
             RestartSec=2s\n\
             \n\
             [Install]\n\
             WantedBy=default.target\n"
        ))
    }

    fn systemd_quote(value: &str) -> Result<String> {
        if value.chars().any(char::is_control) {
            return Err(anyhow!(
                "agent path contains unsupported control characters"
            ));
        }

        let mut quoted = String::from("\"");
        for character in value.chars() {
            match character {
                '\\' => quoted.push_str("\\\\"),
                '"' => quoted.push_str("\\\""),
                '%' => quoted.push_str("%%"),
                _ => quoted.push(character),
            }
        }
        quoted.push('"');
        Ok(quoted)
    }

    fn desktop_quote(value: &str) -> Result<String> {
        if value.chars().any(char::is_control) {
            return Err(anyhow!(
                "agent path contains unsupported control characters"
            ));
        }

        let mut quoted = String::from("\"");
        for character in value.chars() {
            match character {
                '%' => quoted.push_str("%%"),
                '\\' | '"' | '`' | '$' => {
                    quoted.push('\\');
                    quoted.push(character);
                }
                _ => quoted.push(character),
            }
        }
        quoted.push('"');
        Ok(quoted)
    }

    fn autostart_entry(agent_path: &Path) -> Result<String> {
        let agent = agent_path
            .to_str()
            .context("agent path is not valid UTF-8")?;
        let quoted_agent = desktop_quote(agent)?;
        Ok(format!(
            "[Desktop Entry]\n\
             Type=Application\n\
             Name=Native System Monitor Agent\n\
             Comment=Background data collection for Native System Monitor\n\
             Exec={quoted_agent} run\n\
             TryExec={quoted_agent}\n\
             Terminal=false\n\
             NoDisplay=true\n\
             X-GNOME-Autostart-enabled=true\n\
             {DESKTOP_MARKER}\n"
        ))
    }

    #[cfg(test)]
    mod tests {
        use super::{autostart_entry, systemd_unit};
        use std::path::Path;

        #[test]
        fn systemd_unit_quotes_paths_and_restarts_failed_agents() {
            let unit = systemd_unit(Path::new("/home/user/Monitor%build/agent")).unwrap();

            assert!(unit.contains("# managed-by-native-system-monitor"));
            assert!(unit.contains("ExecStart=\"/home/user/Monitor%%build/agent\" run"));
            assert!(unit.contains("Restart=on-failure"));
            assert!(unit.contains("WantedBy=default.target"));
        }

        #[test]
        fn xdg_autostart_escapes_exec_arguments_and_marks_ownership() {
            let entry = autostart_entry(Path::new("/home/user/Monitor $tools%/agent")).unwrap();

            assert!(entry.contains("Exec=\"/home/user/Monitor \\$tools%%/agent\" run"));
            assert!(entry.contains("NoDisplay=true"));
            assert!(entry.contains("X-NativeMonitor-Managed=managed-by-native-system-monitor"));
        }
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use super::{
        MANAGED_MARKER, Result, StartupMode, require_success, write_managed_file, xml_escape,
    };
    use anyhow::{Context, anyhow};
    use std::env;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    const LABEL: &str = "com.native-system-monitor.agent";

    pub fn register(agent_path: &Path) -> Result<StartupMode> {
        configure(agent_path, false)
    }

    pub fn ensure_started(agent_path: &Path) -> Result<StartupMode> {
        configure(agent_path, true)
    }

    fn configure(agent_path: &Path, start_now: bool) -> Result<StartupMode> {
        let home = env::var_os("HOME")
            .map(PathBuf::from)
            .context("HOME is not set")?;
        if !home.is_absolute() {
            return Err(anyhow!("HOME must be an absolute path"));
        }
        let launch_agents = home.join("Library").join("LaunchAgents");
        let plist_path = launch_agents.join(format!("{LABEL}.plist"));
        let contents = launch_agent_plist(agent_path)?;
        write_managed_file(&plist_path, &contents)?;

        if start_now {
            let domain = gui_domain()?;
            let target = format!("{domain}/{LABEL}");
            let loaded = Command::new("launchctl")
                .args(["print", target.as_str()])
                .output()
                .map(|output| output.status.success())
                .unwrap_or(false);

            if loaded {
                let output = Command::new("launchctl")
                    .args(["kickstart", "-k", target.as_str()])
                    .output()
                    .context("restart the per-user agent through launchd")?;
                require_success(&output, "restart the per-user agent through launchd")?;
            } else {
                let domain_argument = domain.as_str();
                let plist_argument = plist_path
                    .to_str()
                    .context("LaunchAgent path is not valid UTF-8")?;
                let output = Command::new("launchctl")
                    .args(["bootstrap", domain_argument, plist_argument])
                    .output()
                    .context("register and start the per-user LaunchAgent")?;
                require_success(&output, "register and start the per-user LaunchAgent")?;
            }
        }

        Ok(StartupMode::LaunchAgent)
    }

    fn gui_domain() -> Result<String> {
        let output = Command::new("id")
            .arg("-u")
            .output()
            .context("read the current user ID for launchd")?;
        require_success(&output, "read the current user ID for launchd")?;
        let user_id = String::from_utf8(output.stdout)
            .context("current user ID is not valid UTF-8")?
            .trim()
            .to_owned();
        if user_id.is_empty() || !user_id.chars().all(|character| character.is_ascii_digit()) {
            return Err(anyhow!(
                "could not determine the current launchd GUI domain"
            ));
        }
        Ok(format!("gui/{user_id}"))
    }

    fn launch_agent_plist(agent_path: &Path) -> Result<String> {
        let agent = agent_path
            .to_str()
            .context("agent path is not valid UTF-8")?;
        let escaped_agent = xml_escape(agent);
        Ok(format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
             <plist version=\"1.0\">\n\
             <dict>\n\
             <key>Label</key><string>{LABEL}</string>\n\
             <key>Description</key><string>{MANAGED_MARKER}</string>\n\
             <key>ProgramArguments</key><array><string>{escaped_agent}</string><string>run</string></array>\n\
             <key>RunAtLoad</key><true/>\n\
             <key>KeepAlive</key><dict><key>SuccessfulExit</key><false/></dict>\n\
             <key>ThrottleInterval</key><integer>5</integer>\n\
             <key>ProcessType</key><string>Background</string>\n\
             </dict>\n\
             </plist>\n"
        ))
    }

    #[cfg(test)]
    mod tests {
        use super::launch_agent_plist;
        use std::path::Path;

        #[test]
        fn launch_agent_runs_at_login_and_restarts_unexpected_exit() {
            let plist = launch_agent_plist(Path::new("/Applications/Monitor & Agent"))
                .expect("build LaunchAgent plist");

            assert!(plist.contains("managed-by-native-system-monitor"));
            assert!(plist.contains("/Applications/Monitor &amp; Agent"));
            assert!(plist.contains("<key>RunAtLoad</key><true/>"));
            assert!(plist.contains("<key>SuccessfulExit</key><false/>"));
        }
    }
}

#[cfg(windows)]
mod platform {
    use super::{MANAGED_MARKER, Result, StartupMode, require_success, write_new_file, xml_escape};
    use anyhow::{Context, anyhow, bail};
    use std::env;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    const TASK_NAME: &str = "Native System Monitor Agent";

    pub fn register(agent_path: &Path) -> Result<StartupMode> {
        configure(agent_path, false)
    }

    pub fn ensure_started(agent_path: &Path) -> Result<StartupMode> {
        configure(agent_path, true)
    }

    fn configure(agent_path: &Path, start_now: bool) -> Result<StartupMode> {
        let task_xml = task_xml(agent_path)?;
        ensure_task_is_managed()?;
        let xml_path = temporary_xml_path();
        if let Err(error) = write_new_file(&xml_path, &task_xml) {
            let _ = fs::remove_file(&xml_path);
            return Err(error);
        }
        let output_result = Command::new("schtasks.exe")
            .args(["/Create", "/TN", TASK_NAME, "/XML"])
            .arg(&xml_path)
            .arg("/F")
            .output()
            .context("register the per-user logon task");
        let _ = fs::remove_file(&xml_path);
        let output = output_result?;
        require_success(&output, "register the per-user logon task")?;

        if start_now {
            let output = Command::new("schtasks.exe")
                .args(["/Run", "/TN", TASK_NAME])
                .output()
                .context("start the per-user agent task")?;
            require_success(&output, "start the per-user agent task")?;
        }

        Ok(StartupMode::WindowsLogonTask)
    }

    fn ensure_task_is_managed() -> Result<()> {
        let output = Command::new("schtasks.exe")
            .args(["/Query", "/TN", TASK_NAME, "/XML"])
            .output()
            .context("check the existing per-user logon task")?;
        if output.status.success() {
            let contents = decode_task_xml(&output.stdout);
            if !contents.contains(MANAGED_MARKER) {
                bail!("refusing to replace an unmanaged scheduled task named {TASK_NAME}");
            }
            return Ok(());
        }

        let tasks = Command::new("schtasks.exe")
            .args(["/Query", "/FO", "CSV", "/NH"])
            .output()
            .context("check whether the scheduled task name is already in use")?;
        let task_list = decode_task_xml(&tasks.stdout);
        require_success(&tasks, "list scheduled tasks")?;
        let root_task_name = format!("\\{TASK_NAME}");
        for line in task_list.lines() {
            let task_name = line
                .split(',')
                .next()
                .unwrap_or_default()
                .trim()
                .trim_matches('"');
            if task_name == TASK_NAME || task_name == root_task_name {
                bail!(
                    "refusing to replace a scheduled task named {TASK_NAME} whose ownership could not be verified"
                );
            }
        }
        Ok(())
    }

    fn decode_task_xml(bytes: &[u8]) -> String {
        if bytes.starts_with(&[0xFF, 0xFE]) {
            let units = bytes[2..]
                .chunks_exact(2)
                .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
                .collect::<Vec<_>>();
            return String::from_utf16_lossy(&units);
        }
        if bytes.starts_with(&[0xFE, 0xFF]) {
            let units = bytes[2..]
                .chunks_exact(2)
                .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
                .collect::<Vec<_>>();
            return String::from_utf16_lossy(&units);
        }

        let null_bytes = bytes.iter().filter(|byte| **byte == 0).count();
        if null_bytes > bytes.len() / 8 {
            let units = bytes
                .chunks_exact(2)
                .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
                .collect::<Vec<_>>();
            return String::from_utf16_lossy(&units);
        }

        String::from_utf8_lossy(bytes).into_owned()
    }

    fn temporary_xml_path() -> PathBuf {
        let sequence = super::NEXT_TEMP_FILE.fetch_add(1, super::Ordering::Relaxed);
        env::temp_dir().join(format!(
            "native-monitor-agent-{}-{sequence}.xml",
            std::process::id()
        ))
    }

    fn task_xml(agent_path: &Path) -> Result<String> {
        let agent = agent_path
            .to_str()
            .context("agent path is not valid UTF-8")?;
        let user_name = env::var("USERNAME")
            .or_else(|_| env::var("USER"))
            .context("current Windows user name is unavailable")?;
        let domain = env::var("USERDOMAIN").unwrap_or_default();
        let user_id = if domain.is_empty() {
            user_name
        } else {
            format!("{domain}\\{user_name}")
        };
        if user_id.is_empty() || user_id.chars().any(char::is_control) {
            return Err(anyhow!(
                "current Windows user name contains unsupported characters"
            ));
        }

        Ok(format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
             <Task version=\"1.2\" xmlns=\"http://schemas.microsoft.com/windows/2004/02/mit/task\">\r\n\
             <RegistrationInfo><Description>{MANAGED_MARKER}</Description></RegistrationInfo>\r\n\
             <Triggers><LogonTrigger><Enabled>true</Enabled></LogonTrigger></Triggers>\r\n\
             <Principals><Principal id=\"Author\"><UserId>{}</UserId><LogonType>InteractiveToken</LogonType><RunLevel>LeastPrivilege</RunLevel></Principal></Principals>\r\n\
             <Settings><MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy><DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries><StopIfGoingOnBatteries>false</StopIfGoingOnBatteries><StartWhenAvailable>true</StartWhenAvailable><Enabled>true</Enabled><Hidden>true</Hidden><ExecutionTimeLimit>PT0S</ExecutionTimeLimit><RestartOnFailure><Interval>PT1M</Interval><Count>3</Count></RestartOnFailure></Settings>\r\n\
             <Actions Context=\"Author\"><Exec><Command>{}</Command><Arguments>run</Arguments></Exec></Actions>\r\n\
             </Task>\r\n",
            xml_escape(&user_id),
            xml_escape(agent)
        ))
    }

    #[cfg(test)]
    mod tests {
        use super::task_xml;
        use std::path::Path;

        #[test]
        fn task_runs_for_the_interactive_user_and_retries_failures() {
            let task = task_xml(Path::new("C:\\Program Files\\Monitor & Agent.exe"))
                .expect("build scheduled task XML");

            assert!(task.contains("managed-by-native-system-monitor"));
            assert!(task.contains("<LogonType>InteractiveToken</LogonType>"));
            assert!(task.contains("<RunLevel>LeastPrivilege</RunLevel>"));
            assert!(task.contains("<RestartOnFailure>"));
            assert!(task.contains("C:\\Program Files\\Monitor &amp; Agent.exe"));
        }
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
mod platform {
    use super::{Result, StartupMode};
    use anyhow::bail;
    use std::path::Path;

    pub fn register(_agent_path: &Path) -> Result<StartupMode> {
        bail!("automatic per-user agent startup is not supported on this platform")
    }

    pub fn ensure_started(_agent_path: &Path) -> Result<StartupMode> {
        bail!("automatic per-user agent startup is not supported on this platform")
    }
}

#[cfg(test)]
mod tests {
    use super::{MANAGED_MARKER, require_success, write_managed_file};
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    fn temporary_directory() -> PathBuf {
        let id = NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "native-monitor-lifecycle-test-{}-{id}",
            std::process::id()
        ))
    }

    #[test]
    fn checking_command_success_preserves_stdout_for_the_caller() {
        let test_binary = std::env::current_exe().expect("locate the test binary");
        let output = Command::new(test_binary)
            .arg("--list")
            .output()
            .expect("list tests from the test binary");

        require_success(&output, "list tests").expect("test listing succeeds");

        assert!(!output.stdout.is_empty());
    }

    #[test]
    fn managed_startup_file_is_idempotently_updated() {
        let directory = temporary_directory();
        let path = directory.join("startup.conf");
        let first = format!("# {MANAGED_MARKER}\nversion=one\n");
        let second = format!("# {MANAGED_MARKER}\nversion=two\n");

        write_managed_file(&path, &first).expect("create startup file");
        write_managed_file(&path, &first).expect("keep identical startup file");
        write_managed_file(&path, &second).expect("update managed startup file");

        assert_eq!(
            fs::read_to_string(&path).expect("read startup file"),
            second
        );
        fs::remove_dir_all(directory).expect("remove test directory");
    }

    #[test]
    fn unmanaged_startup_file_is_never_overwritten() {
        let directory = temporary_directory();
        fs::create_dir_all(&directory).expect("create test directory");
        let path = directory.join("startup.conf");
        fs::write(&path, "user-owned configuration\n").expect("create user file");

        let result = write_managed_file(&path, &format!("# {MANAGED_MARKER}\n"));

        assert!(result.is_err());
        assert_eq!(
            fs::read_to_string(&path).expect("read user file"),
            "user-owned configuration\n"
        );
        fs::remove_dir_all(directory).expect("remove test directory");
    }
}
