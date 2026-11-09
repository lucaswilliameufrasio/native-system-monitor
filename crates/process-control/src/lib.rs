use native_monitor_domain::ProcessIdentity;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessAction {
    Gentle,
    Force,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessActionResult {
    Submitted,
    NoWindowFound,
    ProcessNotFound,
    IdentityMismatch,
    PermissionDenied,
    Unsupported,
    Failed,
}

pub fn perform_action(identity: ProcessIdentity, action: ProcessAction) -> ProcessActionResult {
    if identity.pid == 0 || identity.start_time_unix_seconds == 0 {
        return ProcessActionResult::IdentityMismatch;
    }

    #[cfg(target_os = "linux")]
    {
        unix::perform_linux_action(identity, action)
    }

    #[cfg(all(unix, not(target_os = "linux")))]
    {
        unix::perform_unix_action(identity, action)
    }

    #[cfg(windows)]
    {
        windows::perform_windows_action(identity, action)
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = (identity, action);
        ProcessActionResult::Unsupported
    }
}

#[cfg(unix)]
mod unix {
    use super::{ProcessAction, ProcessActionResult};
    use native_monitor_domain::ProcessIdentity;
    use rustix::process::{Pid, Signal, kill_process};
    use sysinfo::{ProcessesToUpdate, System};

    #[cfg(target_os = "linux")]
    use rustix::process::{PidfdFlags, pidfd_open, pidfd_send_signal};

    #[cfg(not(target_os = "linux"))]
    pub(super) fn perform_unix_action(
        identity: ProcessIdentity,
        action: ProcessAction,
    ) -> ProcessActionResult {
        let validation = validate_process_identity(identity);
        if validation != ProcessActionResult::Submitted {
            return validation;
        }

        if identity.pid > i32::MAX as u32 {
            return ProcessActionResult::IdentityMismatch;
        }
        let Some(pid) = Pid::from_raw(identity.pid as i32) else {
            return ProcessActionResult::IdentityMismatch;
        };
        let signal = signal_for(action);
        match kill_process(pid, signal) {
            Ok(()) => ProcessActionResult::Submitted,
            Err(error) => result_from_error(std::io::Error::from(error)),
        }
    }

    #[cfg(target_os = "linux")]
    pub(super) fn perform_linux_action(
        identity: ProcessIdentity,
        action: ProcessAction,
    ) -> ProcessActionResult {
        let validation = validate_process_identity(identity);
        if validation != ProcessActionResult::Submitted {
            return validation;
        }

        if identity.pid > i32::MAX as u32 {
            return ProcessActionResult::IdentityMismatch;
        }
        let Some(pid) = Pid::from_raw(identity.pid as i32) else {
            return ProcessActionResult::IdentityMismatch;
        };
        let signal = signal_for(action);
        let pidfd = match pidfd_open(pid, PidfdFlags::empty()) {
            Ok(pidfd) => pidfd,
            Err(_) => {
                let validation = validate_process_identity(identity);
                if validation != ProcessActionResult::Submitted {
                    return validation;
                }
                return match kill_process(pid, signal) {
                    Ok(()) => ProcessActionResult::Submitted,
                    Err(error) => result_from_error(std::io::Error::from(error)),
                };
            }
        };

        let validation = validate_process_identity(identity);
        if validation != ProcessActionResult::Submitted {
            return validation;
        }

        match pidfd_send_signal(&pidfd, signal) {
            Ok(()) => ProcessActionResult::Submitted,
            Err(error) => result_from_error(std::io::Error::from(error)),
        }
    }

    fn validate_process_identity(identity: ProcessIdentity) -> ProcessActionResult {
        let mut system = System::new();
        let pid = sysinfo::Pid::from_u32(identity.pid);
        system.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
        let Some(process) = system.process(pid) else {
            return ProcessActionResult::ProcessNotFound;
        };
        if process.start_time() != identity.start_time_unix_seconds {
            return ProcessActionResult::IdentityMismatch;
        }
        ProcessActionResult::Submitted
    }

    fn signal_for(action: ProcessAction) -> Signal {
        match action {
            ProcessAction::Gentle => Signal::TERM,
            ProcessAction::Force => Signal::KILL,
        }
    }

    fn result_from_error(error: std::io::Error) -> ProcessActionResult {
        match error.kind() {
            std::io::ErrorKind::NotFound => ProcessActionResult::ProcessNotFound,
            std::io::ErrorKind::PermissionDenied => ProcessActionResult::PermissionDenied,
            std::io::ErrorKind::Unsupported => ProcessActionResult::Unsupported,
            _ => ProcessActionResult::Failed,
        }
    }
}

#[cfg(windows)]
#[allow(unsafe_code)]
mod windows {
    use super::{ProcessAction, ProcessActionResult};
    use native_monitor_domain::ProcessIdentity;
    use std::ffi::c_void;
    use std::mem::MaybeUninit;
    use windows_sys::Win32::Foundation::{
        CloseHandle, ERROR_ACCESS_DENIED, ERROR_INVALID_PARAMETER, FILETIME, GetLastError, HWND,
        LPARAM, WPARAM,
    };
    use windows_sys::Win32::System::Threading::{
        GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE,
        TerminateProcess,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowThreadProcessId, SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_CLOSE,
        WNDENUMPROC,
    };

    const UNIX_EPOCH_FILETIME_TICKS: u64 = 116_444_736_000_000_000;
    const PROCESS_QUERY_AND_TERMINATE: u32 = PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE;

    pub(super) fn perform_windows_action(
        identity: ProcessIdentity,
        action: ProcessAction,
    ) -> ProcessActionResult {
        let desired_access = match action {
            ProcessAction::Gentle => PROCESS_QUERY_LIMITED_INFORMATION,
            ProcessAction::Force => PROCESS_QUERY_AND_TERMINATE,
        };
        let process_handle = unsafe { OpenProcess(desired_access, 0, identity.pid) };
        if process_handle.is_null() {
            return result_from_last_error();
        }

        match process_creation_time(process_handle) {
            Ok(start_time) if start_time == identity.start_time_unix_seconds => {}
            Ok(_) => {
                unsafe {
                    CloseHandle(process_handle);
                }
                return ProcessActionResult::IdentityMismatch;
            }
            Err(result) => {
                unsafe {
                    CloseHandle(process_handle);
                }
                return result;
            }
        }

        let result = match action {
            ProcessAction::Gentle => send_close_to_process_windows(identity),
            ProcessAction::Force => {
                if unsafe { TerminateProcess(process_handle, 1) } != 0 {
                    ProcessActionResult::Submitted
                } else {
                    result_from_last_error()
                }
            }
        };
        unsafe {
            CloseHandle(process_handle);
        }
        result
    }

    fn process_creation_time(process_handle: *mut c_void) -> Result<u64, ProcessActionResult> {
        let mut creation_time = MaybeUninit::<FILETIME>::uninit();
        let mut exit_time = MaybeUninit::<FILETIME>::uninit();
        let mut kernel_time = MaybeUninit::<FILETIME>::uninit();
        let mut user_time = MaybeUninit::<FILETIME>::uninit();
        let succeeded = unsafe {
            GetProcessTimes(
                process_handle,
                creation_time.as_mut_ptr(),
                exit_time.as_mut_ptr(),
                kernel_time.as_mut_ptr(),
                user_time.as_mut_ptr(),
            )
        };
        if succeeded == 0 {
            return Err(result_from_last_error());
        }
        let creation_time = unsafe { creation_time.assume_init() };
        let ticks = (u64::from(creation_time.dwHighDateTime) << 32)
            | u64::from(creation_time.dwLowDateTime);
        Ok(ticks.saturating_sub(UNIX_EPOCH_FILETIME_TICKS) / 10_000_000)
    }

    fn send_close_to_process_windows(identity: ProcessIdentity) -> ProcessActionResult {
        let mut context = WindowSearchContext {
            identity,
            windows_seen: 0,
            windows_closed: 0,
        };
        let callback: WNDENUMPROC = Some(enumerate_process_window);
        unsafe {
            EnumWindows(
                callback,
                (&mut context as *mut WindowSearchContext) as LPARAM,
            );
        }
        if context.windows_closed > 0 {
            ProcessActionResult::Submitted
        } else if context.windows_seen > 0 {
            ProcessActionResult::Failed
        } else {
            ProcessActionResult::NoWindowFound
        }
    }

    struct WindowSearchContext {
        identity: ProcessIdentity,
        windows_seen: usize,
        windows_closed: usize,
    }

    unsafe extern "system" fn enumerate_process_window(hwnd: HWND, parameter: LPARAM) -> i32 {
        let context = unsafe { &mut *(parameter as *mut WindowSearchContext) };
        if context.windows_seen >= 8 {
            return 0;
        }
        let mut window_pid = 0;
        unsafe {
            GetWindowThreadProcessId(hwnd, &mut window_pid);
        }
        if window_pid != context.identity.pid {
            return 1;
        }

        let window_process =
            unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, window_pid) };
        if window_process.is_null() {
            return 1;
        }
        let start_time_matches = process_creation_time(window_process)
            .is_ok_and(|start_time| start_time == context.identity.start_time_unix_seconds);
        unsafe {
            CloseHandle(window_process);
        }
        if !start_time_matches {
            return 1;
        }

        context.windows_seen += 1;
        let mut message_result = 0usize;
        let sent = unsafe {
            SendMessageTimeoutW(
                hwnd,
                WM_CLOSE,
                WPARAM::default(),
                LPARAM::default(),
                SMTO_ABORTIFHUNG,
                250,
                &mut message_result,
            )
        };
        if sent != 0 {
            context.windows_closed += 1;
        }
        1
    }

    fn result_from_last_error() -> ProcessActionResult {
        match unsafe { GetLastError() } {
            ERROR_ACCESS_DENIED => ProcessActionResult::PermissionDenied,
            ERROR_INVALID_PARAMETER => ProcessActionResult::ProcessNotFound,
            _ => ProcessActionResult::Failed,
        }
    }
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use super::{ProcessAction, ProcessActionResult, perform_action};
    #[cfg(unix)]
    use native_monitor_domain::ProcessIdentity;
    #[cfg(unix)]
    use std::ops::{Deref, DerefMut};
    #[cfg(unix)]
    use std::process::{Child, Command, Stdio};
    use std::thread;
    use std::time::Duration;
    #[cfg(unix)]
    use std::time::Instant;
    #[cfg(unix)]
    use sysinfo::{ProcessesToUpdate, System};

    #[cfg(unix)]
    const CHILD_TEST_NAME: &str = "tests::fixture_child_waits_until_terminated";
    const FIXTURE_ENVIRONMENT: &str = "NATIVE_MONITOR_PROCESS_FIXTURE";

    #[cfg(unix)]
    struct FixtureChild(Child);

    #[cfg(unix)]
    impl Deref for FixtureChild {
        type Target = Child;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    #[cfg(unix)]
    impl DerefMut for FixtureChild {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.0
        }
    }

    #[cfg(unix)]
    impl Drop for FixtureChild {
        fn drop(&mut self) {
            if !matches!(self.0.try_wait(), Ok(Some(_))) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
    }

    #[test]
    fn fixture_child_waits_until_terminated() {
        if std::env::var_os(FIXTURE_ENVIRONMENT).is_none() {
            return;
        }
        loop {
            thread::sleep(Duration::from_secs(1));
        }
    }

    #[cfg(unix)]
    #[test]
    fn gentle_action_terminates_only_the_matching_fixture_process() {
        let mut child = spawn_fixture();
        let identity = wait_for_identity(&mut child);

        assert_eq!(
            perform_action(identity, ProcessAction::Gentle),
            ProcessActionResult::Submitted
        );
        let status = child
            .wait()
            .expect("wait for the fixture process to terminate");
        assert!(!status.success());
    }

    #[cfg(unix)]
    #[test]
    fn reused_or_stale_identity_is_rejected_without_signalling_the_fixture() {
        let mut child = spawn_fixture();
        let mut identity = wait_for_identity(&mut child);
        identity.start_time_unix_seconds = identity.start_time_unix_seconds.saturating_sub(1);

        assert_eq!(
            perform_action(identity, ProcessAction::Force),
            ProcessActionResult::IdentityMismatch
        );
        assert!(child.try_wait().expect("check fixture status").is_none());
        child.kill().expect("clean up the owned fixture process");
        let _status = child.wait().expect("reap fixture process");
    }

    #[cfg(unix)]
    #[test]
    fn missing_process_identity_is_rejected_without_signalling() {
        let identity = ProcessIdentity {
            pid: u32::MAX,
            start_time_unix_seconds: 1,
        };

        assert_eq!(
            perform_action(identity, ProcessAction::Gentle),
            ProcessActionResult::ProcessNotFound
        );
    }

    #[cfg(unix)]
    #[test]
    fn force_action_terminates_only_the_matching_fixture_process() {
        let mut child = spawn_fixture();
        let identity = wait_for_identity(&mut child);

        assert_eq!(
            perform_action(identity, ProcessAction::Force),
            ProcessActionResult::Submitted
        );
        let status = child
            .wait()
            .expect("wait for the fixture process to terminate");
        assert!(!status.success());
    }

    #[cfg(unix)]
    fn spawn_fixture() -> FixtureChild {
        let child = Command::new(std::env::current_exe().expect("current test executable"))
            .args(["--exact", CHILD_TEST_NAME, "--nocapture"])
            .env(FIXTURE_ENVIRONMENT, "1")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("start an isolated child fixture");
        FixtureChild(child)
    }

    #[cfg(unix)]
    fn wait_for_identity(child: &mut FixtureChild) -> ProcessIdentity {
        let deadline = Instant::now() + Duration::from_secs(5);
        let pid = sysinfo::Pid::from_u32(child.id());
        let mut system = System::new();
        while Instant::now() < deadline {
            system.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
            if let Some(process) = system.process(pid) {
                return ProcessIdentity {
                    pid: child.id(),
                    start_time_unix_seconds: process.start_time(),
                };
            }
            thread::sleep(Duration::from_millis(10));
        }
        panic!("fixture process did not appear in the live process table");
    }
}
