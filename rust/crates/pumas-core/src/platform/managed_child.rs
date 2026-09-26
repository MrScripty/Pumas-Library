//! Generation-owned child supervision for managed Torch operations.
//!
//! Unix children lead a private process group. Windows children are admitted
//! suspended to a retained kill-on-close Job before their first instruction.

use crate::models::RuntimeEndpointUrl;
use std::io;
use std::process::{Child, Command, ExitStatus};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const POLL_INTERVAL: Duration = Duration::from_millis(20);

pub struct ManagedChild {
    child: Option<Child>,
    drained: bool,
    cleanup_lease: Option<Arc<dyn Send + Sync>>,
    custody: Option<Arc<ManagedChildCustodySlot>>,
    #[cfg(test)]
    force_observation_failure: Arc<std::sync::atomic::AtomicBool>,
    #[cfg(all(test, target_os = "macos"))]
    force_group_signal_permission_denied: Arc<std::sync::atomic::AtomicBool>,
    #[cfg(windows)]
    job: std::sync::Arc<std::os::windows::io::OwnedHandle>,
}

#[derive(Debug, Default)]
pub struct ManagedChildCustodySlot {
    parked: Mutex<Option<ManagedChild>>,
    active: std::sync::atomic::AtomicBool,
    cleanup_pending: std::sync::atomic::AtomicBool,
    admission_complete: std::sync::atomic::AtomicBool,
    changed: tokio::sync::Notify,
}

impl ManagedChildCustodySlot {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn has_parked_child(&self) -> bool {
        self.parked
            .lock()
            .expect("Child custody poisoned")
            .is_some()
    }

    pub fn is_active(&self) -> bool {
        self.active.load(std::sync::atomic::Ordering::Acquire)
    }

    pub fn is_cleanup_pending(&self) -> bool {
        self.cleanup_pending
            .load(std::sync::atomic::Ordering::Acquire)
    }

    pub async fn wait_for_park_or_completion(&self) -> bool {
        loop {
            let notified = self.changed.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if self.has_parked_child() {
                return true;
            }
            if self
                .admission_complete
                .load(std::sync::atomic::Ordering::Acquire)
                && !self.is_active()
            {
                return false;
            }
            notified.await;
        }
    }

    /// Retry a bounded process-tree drain. Failure leaves custody in this slot.
    pub fn drain(&self, deadline: Duration) -> io::Result<bool> {
        let Some(mut child) = self
            .parked
            .lock()
            .map_err(|_| io::Error::other("Child custody poisoned"))?
            .take()
        else {
            return if self.is_active() {
                Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "Owned child has not returned custody",
                ))
            } else {
                Ok(false)
            };
        };
        match child.terminate_and_drain(deadline) {
            Ok(_) => Ok(true),
            Err(error) => {
                *self
                    .parked
                    .lock()
                    .map_err(|_| io::Error::other("Child custody poisoned"))? = Some(child);
                Err(error)
            }
        }
    }
}

impl std::fmt::Debug for ManagedChild {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ManagedChild")
            .field("pid", &self.id())
            .field("drained", &self.drained)
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct ManagedListenerCustody {
    #[cfg(unix)]
    pid: u32,
    #[cfg(windows)]
    job: std::sync::Arc<std::os::windows::io::OwnedHandle>,
}

impl ManagedListenerCustody {
    pub fn owns_listener(&self, endpoint: &RuntimeEndpointUrl) -> io::Result<bool> {
        #[cfg(target_os = "linux")]
        {
            super::runtime_listener::owns_listener(self.pid, endpoint)
        }
        #[cfg(target_os = "macos")]
        {
            macos_owns_listener(self.pid, endpoint)
        }
        #[cfg(windows)]
        {
            windows::job_owns_listener(&self.job, endpoint)
        }
    }
}

impl ManagedChild {
    pub fn spawn(command: &mut Command, custody: Arc<ManagedChildCustodySlot>) -> io::Result<Self> {
        if custody
            .active
            .swap(true, std::sync::atomic::Ordering::AcqRel)
        {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "Custody slot already active",
            ));
        }
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
            let child = match command.spawn() {
                Ok(child) => child,
                Err(error) => {
                    custody
                        .active
                        .store(false, std::sync::atomic::Ordering::Release);
                    custody
                        .admission_complete
                        .store(true, std::sync::atomic::Ordering::Release);
                    custody.changed.notify_waiters();
                    return Err(error);
                }
            };
            custody
                .admission_complete
                .store(true, std::sync::atomic::Ordering::Release);
            custody.changed.notify_waiters();
            Ok(Self {
                child: Some(child),
                drained: false,
                cleanup_lease: None,
                custody: Some(custody),
                #[cfg(test)]
                force_observation_failure: Arc::new(std::sync::atomic::AtomicBool::new(false)),
                #[cfg(all(test, target_os = "macos"))]
                force_group_signal_permission_denied: Arc::new(std::sync::atomic::AtomicBool::new(
                    false,
                )),
            })
        }
        #[cfg(windows)]
        {
            let result = windows::spawn(command, custody.clone());
            custody
                .admission_complete
                .store(true, std::sync::atomic::Ordering::Release);
            if result.is_err() && !custody.is_cleanup_pending() {
                custody
                    .active
                    .store(false, std::sync::atomic::Ordering::Release);
            }
            custody.changed.notify_waiters();
            result
        }
    }

    pub fn id(&self) -> u32 {
        self.child.as_ref().expect("managed child retained").id()
    }

    pub fn attach_cleanup_lease<T: Send + Sync + 'static>(&mut self, lease: Arc<T>) {
        self.cleanup_lease = Some(lease);
    }

    pub fn listener_custody(&self) -> ManagedListenerCustody {
        ManagedListenerCustody {
            #[cfg(unix)]
            pid: self.id(),
            #[cfg(windows)]
            job: self.job.clone(),
        }
    }

    /// Observe the leader without releasing its Unix PID/PGID pin.
    pub fn observe_exit(&mut self) -> io::Result<Option<ExitStatus>> {
        #[cfg(target_os = "linux")]
        {
            super::linux_group::observe_exit(self.id())
        }
        #[cfg(target_os = "macos")]
        {
            use std::os::unix::process::ExitStatusExt;
            #[allow(unsafe_code)]
            let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
            // SAFETY: `info` is a valid output buffer; WNOWAIT retains the
            // direct child's PID until all members have drained.
            #[allow(unsafe_code)]
            let result = unsafe {
                libc::waitid(
                    libc::P_PID,
                    self.id(),
                    &mut info,
                    libc::WEXITED | libc::WNOWAIT | libc::WNOHANG,
                )
            };
            if result != 0 {
                return Err(io::Error::last_os_error());
            }
            // SAFETY: waitid initialized siginfo_t on success.
            #[allow(unsafe_code)]
            if unsafe { info.si_pid() } == 0 {
                return Ok(None);
            }
            #[allow(unsafe_code)]
            let status = unsafe { info.si_status() };
            Ok(Some(ExitStatus::from_raw(
                if info.si_code == libc::CLD_EXITED {
                    status << 8
                } else {
                    status
                },
            )))
        }
        #[cfg(windows)]
        {
            self.child
                .as_mut()
                .expect("managed child retained")
                .try_wait()
        }
    }

    /// Stop the owned generation, prove descendant drain, then reap its leader.
    /// An error retains custody so callers can retry rather than publishing or
    /// deleting a still-owned staging directory.
    pub fn terminate_and_drain(&mut self, deadline: Duration) -> io::Result<ExitStatus> {
        #[cfg(test)]
        if self
            .force_observation_failure
            .load(std::sync::atomic::Ordering::Acquire)
        {
            return Err(io::Error::other("Injected process observation failure"));
        }
        let until = Instant::now() + deadline;
        loop {
            #[cfg(target_os = "linux")]
            {
                super::linux_group::signal_group(self.id())?;
                if !super::linux_group::group_has_live_members(
                    i32::try_from(self.id()).map_err(|_| io::Error::other("Invalid group ID"))?,
                )? {
                    break;
                }
            }
            #[cfg(target_os = "macos")]
            {
                let pid =
                    i32::try_from(self.id()).map_err(|_| io::Error::other("Invalid group ID"))?;
                if !macos_drain_group_once(pid, macos_group_has_live_members, |group| {
                    #[cfg(test)]
                    if self
                        .force_group_signal_permission_denied
                        .load(std::sync::atomic::Ordering::Acquire)
                    {
                        return Err(io::Error::from_raw_os_error(libc::EPERM));
                    }
                    // SAFETY: the unreaped direct child pins its group ID; the
                    // group was created by this owner at spawn.
                    #[allow(unsafe_code)]
                    if unsafe { libc::killpg(group, libc::SIGKILL) } != 0 {
                        return Err(io::Error::last_os_error());
                    }
                    Ok(())
                })? {
                    break;
                }
            }
            #[cfg(windows)]
            {
                if !windows::terminate_and_has_members(&self.job)? {
                    break;
                }
            }
            if Instant::now() >= until {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "Owned process-tree drain timed out",
                ));
            }
            std::thread::sleep(POLL_INTERVAL);
        }
        let status = self
            .child
            .as_mut()
            .expect("managed child retained")
            .wait()?;
        self.drained = true;
        if let Some(custody) = &self.custody {
            custody
                .active
                .store(false, std::sync::atomic::Ordering::Release);
            custody
                .cleanup_pending
                .store(false, std::sync::atomic::Ordering::Release);
            custody.changed.notify_waiters();
        }
        Ok(status)
    }
}

impl Drop for ManagedChild {
    fn drop(&mut self) {
        if !self.drained {
            let Some(child) = self.child.take() else {
                return;
            };
            let lease = self.cleanup_lease.take();
            let custody = self
                .custody
                .take()
                .expect("Admitted child custody registered");
            #[cfg(windows)]
            let job = self.job.clone();
            #[cfg(test)]
            let force_observation_failure = self.force_observation_failure.clone();
            #[cfg(all(test, target_os = "macos"))]
            let force_group_signal_permission_denied =
                self.force_group_signal_permission_denied.clone();
            let parked = ManagedChild {
                child: Some(child),
                drained: false,
                cleanup_lease: lease,
                custody: Some(custody.clone()),
                #[cfg(windows)]
                job,
                #[cfg(test)]
                force_observation_failure,
                #[cfg(all(test, target_os = "macos"))]
                force_group_signal_permission_denied,
            };
            let mut slot = custody
                .parked
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            assert!(slot.is_none(), "Child custody slot already occupied");
            custody
                .cleanup_pending
                .store(true, std::sync::atomic::Ordering::Release);
            *slot = Some(parked);
            custody.changed.notify_waiters();
        }
    }
}

#[cfg(target_os = "macos")]
fn macos_drain_group_once<O, S>(group: i32, mut observe: O, mut signal: S) -> io::Result<bool>
where
    O: FnMut(i32) -> io::Result<bool>,
    S: FnMut(i32) -> io::Result<()>,
{
    if !observe(group)? {
        return Ok(false);
    }
    match signal(group) {
        Ok(()) => {}
        Err(error) if error.raw_os_error() == Some(libc::ESRCH) => {}
        Err(error) if error.raw_os_error() == Some(libc::EPERM) => {
            if !observe(group)? {
                return Ok(false);
            }
            return Err(error);
        }
        Err(error) => return Err(error),
    }
    observe(group)
}

#[cfg(target_os = "macos")]
fn macos_group_has_live_members(group: i32) -> io::Result<bool> {
    let output = Command::new("/bin/ps")
        .args(["-A", "-o", "pgid=,stat="])
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other("Could not observe managed process group"));
    }
    let text = String::from_utf8(output.stdout)
        .map_err(|_| io::Error::other("Invalid process-group observation"))?;
    for line in text.lines() {
        let mut fields = line.split_whitespace();
        let Some(pgid) = fields.next() else { continue };
        if pgid.parse::<i32>().ok() == Some(group) {
            let state = fields
                .next()
                .ok_or_else(|| io::Error::other("Incomplete process state"))?;
            if !state.starts_with('Z') {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

#[cfg(target_os = "macos")]
fn macos_owns_listener(pid: u32, endpoint: &RuntimeEndpointUrl) -> io::Result<bool> {
    let url = url::Url::parse(endpoint.as_str()).map_err(io::Error::other)?;
    let host = url
        .host_str()
        .ok_or_else(|| io::Error::other("Listener host absent"))?;
    if !host
        .parse::<std::net::IpAddr>()
        .map_err(io::Error::other)?
        .is_loopback()
    {
        return Err(io::Error::other("Managed listener must be loopback"));
    }
    let port = url
        .port_or_known_default()
        .ok_or_else(|| io::Error::other("Listener port absent"))?;
    let output = Command::new("/usr/sbin/lsof")
        .args([
            "-nP",
            "-a",
            "-p",
            &pid.to_string(),
            "-iTCP",
            "-sTCP:LISTEN",
            "-FfnT",
        ])
        .output()?;
    if !output.status.success() {
        return Ok(false);
    }
    let text = String::from_utf8(output.stdout)
        .map_err(|_| io::Error::other("Invalid listener observation"))?;
    Ok(macos_lsof_has_listener(&text, host, port))
}

#[cfg(target_os = "macos")]
fn macos_lsof_has_listener(output: &str, host: &str, port: u16) -> bool {
    let mut name_matches = false;
    let mut listening = false;
    for line in output.lines().chain(std::iter::once("f")) {
        if line.starts_with('f') {
            if name_matches && listening {
                return true;
            }
            name_matches = false;
            listening = false;
        } else if let Some(name) = line.strip_prefix('n') {
            name_matches = name == format!("{host}:{port}") || name == format!("[{host}]:{port}");
        } else if line == "TST=LISTEN" {
            listening = true;
        }
    }
    false
}

#[cfg(all(test, target_os = "macos"))]
#[test]
fn macos_listener_attribution_requires_structured_listen_state() {
    let observation =
        "p42\nf1\nn127.0.0.1:8080\nTST=LISTEN\nf2\nn127.0.0.1:8081\nTST=ESTABLISHED\n";
    assert!(macos_lsof_has_listener(observation, "127.0.0.1", 8080));
    assert!(!macos_lsof_has_listener(observation, "127.0.0.1", 8081));
    assert!(!macos_lsof_has_listener(observation, "127.0.0.2", 8080));
}

#[cfg(windows)]
#[allow(unsafe_code)]
mod windows {
    use super::*;
    use std::mem::{size_of, zeroed};
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use windows_sys::Win32::Foundation::{HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD, THREADENTRY32,
    };
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectBasicAccountingInformation,
        JobObjectExtendedLimitInformation, QueryInformationJobObject, SetInformationJobObject,
        TerminateJobObject, JOBOBJECT_BASIC_ACCOUNTING_INFORMATION,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };
    use windows_sys::Win32::System::Threading::{
        OpenThread, ResumeThread, CREATE_SUSPENDED, THREAD_SUSPEND_RESUME,
    };

    fn checked_tcp_row_count(
        buffer_size: usize,
        reported_size: usize,
        count: usize,
        row_size: usize,
    ) -> io::Result<usize> {
        if reported_size < size_of::<u32>()
            || reported_size > buffer_size
            || row_size == 0
            || count > (reported_size - size_of::<u32>()) / row_size
        {
            return Err(io::Error::other("Invalid TCP owner table"));
        }
        Ok(count)
    }

    pub(super) fn spawn(
        command: &mut Command,
        custody: Arc<ManagedChildCustodySlot>,
    ) -> io::Result<ManagedChild> {
        use std::os::windows::process::CommandExt;
        // The child cannot execute or create descendants before Job admission.
        command.creation_flags(CREATE_SUSPENDED);
        let raw_job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if raw_job.is_null() {
            return Err(io::Error::last_os_error());
        }
        let job = std::sync::Arc::new(unsafe { OwnedHandle::from_raw_handle(raw_job) });
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { zeroed() };
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if unsafe {
            SetInformationJobObject(
                raw_job,
                JobObjectExtendedLimitInformation,
                (&raw const limits).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        let mut child = command.spawn()?;
        if unsafe { AssignProcessToJobObject(raw_job, child.as_raw_handle()) } == 0 {
            let error = io::Error::last_os_error();
            // Still suspended: no child code or descendant could have run.
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
        #[cfg(test)]
        let forced_resume_failure = command.get_envs().any(|(key, value)| {
            key == "PUMAS_TEST_FORCE_RESUME_FAILURE" && value == Some(std::ffi::OsStr::new("1"))
        });
        let resumed = {
            #[cfg(test)]
            if forced_resume_failure {
                Err(io::Error::other("Injected resume failure"))
            } else {
                resume_initial_thread(child.id())
            }
            #[cfg(not(test))]
            {
                resume_initial_thread(child.id())
            }
        };
        if let Err(error) = resumed {
            // Still suspended and Job-owned. Park it for the registered
            // supervisor; caller failure never closes the Job prematurely.
            drop(ManagedChild {
                child: Some(child),
                job,
                drained: false,
                cleanup_lease: None,
                custody: Some(custody),
                #[cfg(test)]
                force_observation_failure: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            });
            return Err(error);
        }
        Ok(ManagedChild {
            child: Some(child),
            job,
            drained: false,
            cleanup_lease: None,
            custody: Some(custody),
            #[cfg(test)]
            force_observation_failure: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        })
    }

    fn resume_initial_thread(pid: u32) -> io::Result<()> {
        let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
        if snapshot == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        let snapshot = unsafe { OwnedHandle::from_raw_handle(snapshot) };
        let mut entry: THREADENTRY32 = unsafe { zeroed() };
        entry.dwSize = size_of::<THREADENTRY32>() as u32;
        let mut found = Vec::new();
        let mut has_entry = unsafe { Thread32First(snapshot.as_raw_handle(), &mut entry) } != 0;
        while has_entry {
            if entry.th32OwnerProcessID == pid {
                found.push(entry.th32ThreadID);
            }
            has_entry = unsafe { Thread32Next(snapshot.as_raw_handle(), &mut entry) } != 0;
        }
        if found.len() != 1 {
            return Err(io::Error::other(
                "Suspended child did not have one initial thread",
            ));
        }
        let thread = unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, found[0]) };
        if thread.is_null() {
            return Err(io::Error::last_os_error());
        }
        let thread = unsafe { OwnedHandle::from_raw_handle(thread) };
        if unsafe { ResumeThread(thread.as_raw_handle()) } == u32::MAX {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    pub(super) fn terminate_and_has_members(job: &OwnedHandle) -> io::Result<bool> {
        let raw = job.as_raw_handle() as HANDLE;
        if unsafe { TerminateJobObject(raw, 1) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut accounting: JOBOBJECT_BASIC_ACCOUNTING_INFORMATION = unsafe { zeroed() };
        if unsafe {
            QueryInformationJobObject(
                raw,
                JobObjectBasicAccountingInformation,
                (&raw mut accounting).cast(),
                size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
                std::ptr::null_mut(),
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(accounting.ActiveProcesses != 0)
    }

    pub(super) fn job_owns_listener(
        job: &OwnedHandle,
        endpoint: &RuntimeEndpointUrl,
    ) -> io::Result<bool> {
        use std::net::{IpAddr, Ipv4Addr};
        use windows_sys::Win32::NetworkManagement::IpHelper::{
            GetExtendedTcpTable, MIB_TCPROW_OWNER_PID, MIB_TCP_STATE_LISTEN,
            TCP_TABLE_OWNER_PID_LISTENER,
        };
        use windows_sys::Win32::Networking::WinSock::AF_INET;
        use windows_sys::Win32::System::JobObjects::IsProcessInJob;
        use windows_sys::Win32::System::Threading::{
            OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        };
        let url = url::Url::parse(endpoint.as_str()).map_err(io::Error::other)?;
        let address: IpAddr = url
            .host_str()
            .ok_or_else(|| io::Error::other("Listener host absent"))?
            .parse()
            .map_err(io::Error::other)?;
        if !address.is_loopback() {
            return Err(io::Error::other("Managed listener must be loopback"));
        }
        let port = url
            .port_or_known_default()
            .ok_or_else(|| io::Error::other("Listener port absent"))?;
        if !address.is_ipv4() {
            return Err(io::Error::other("IPv6 listener attribution unavailable"));
        }
        let mut size = 0;
        unsafe {
            GetExtendedTcpTable(
                std::ptr::null_mut(),
                &mut size,
                0,
                u32::from(AF_INET),
                TCP_TABLE_OWNER_PID_LISTENER,
                0,
            )
        };
        if size < size_of::<u32>() as u32 {
            return Err(io::Error::other("Invalid TCP owner table size"));
        }
        let mut table = vec![0u64; (size as usize).div_ceil(8)];
        let code = unsafe {
            GetExtendedTcpTable(
                table.as_mut_ptr().cast(),
                &mut size,
                0,
                u32::from(AF_INET),
                TCP_TABLE_OWNER_PID_LISTENER,
                0,
            )
        };
        if code != 0 {
            return Err(io::Error::from_raw_os_error(code as i32));
        }
        let reported_size = size as usize;
        if reported_size < size_of::<u32>() || reported_size > table.len() * size_of::<u64>() {
            return Err(io::Error::other("Invalid TCP owner table"));
        }
        let count = unsafe { table.as_ptr().cast::<u32>().read_unaligned() } as usize;
        let row_size = size_of::<MIB_TCPROW_OWNER_PID>();
        for index in 0..checked_tcp_row_count(
            table.len() * size_of::<u64>(),
            reported_size,
            count,
            row_size,
        )? {
            let offset = size_of::<u32>() + index * row_size;
            let row = unsafe {
                table
                    .as_ptr()
                    .cast::<u8>()
                    .add(offset)
                    .cast::<MIB_TCPROW_OWNER_PID>()
                    .read_unaligned()
            };
            if row.dwState != MIB_TCP_STATE_LISTEN as u32
                || Ipv4Addr::from(row.dwLocalAddr.to_ne_bytes()) != address
                || u16::from_be(row.dwLocalPort as u16) != port
            {
                continue;
            }
            let process =
                unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, row.dwOwningPid) };
            if process.is_null() {
                return Err(io::Error::last_os_error());
            }
            let process = unsafe { OwnedHandle::from_raw_handle(process) };
            let mut member = 0;
            if unsafe { IsProcessInJob(process.as_raw_handle(), job.as_raw_handle(), &mut member) }
                == 0
            {
                return Err(io::Error::last_os_error());
            }
            if member != 0 {
                return Ok(true);
            }
        }
        Ok(false)
    }

    #[cfg(test)]
    #[test]
    fn tcp_table_bounds_reject_empty_and_truncated_rows() {
        assert!(checked_tcp_row_count(0, 0, 0, 24).is_err());
        assert_eq!(checked_tcp_row_count(4, 4, 0, 24).unwrap(), 0);
        assert!(checked_tcp_row_count(4, 4, 1, 24).is_err());
        assert!(checked_tcp_row_count(8, 12, 0, 24).is_err());
        assert_eq!(checked_tcp_row_count(28, 28, 1, 24).unwrap(), 1);
    }
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
mod unix_tests {
    use super::*;

    fn wait_for(path: &std::path::Path) -> u32 {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Ok(contents) = std::fs::read_to_string(path) {
                if let Ok(pid) = contents.trim().parse() {
                    return pid;
                }
            }
            assert!(
                Instant::now() < deadline,
                "fixture descendant did not start"
            );
            std::thread::sleep(POLL_INTERVAL);
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn observed_exited_child_drains_without_signaling_zombie_group() {
        let custody = ManagedChildCustodySlot::new();
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "exit 0"]);
        let mut child = ManagedChild::spawn(&mut command, custody.clone()).unwrap();
        let until = Instant::now() + Duration::from_secs(5);
        let observed = loop {
            if let Some(status) = child.observe_exit().unwrap() {
                break status;
            }
            assert!(Instant::now() < until, "child did not exit");
            std::thread::sleep(POLL_INTERVAL);
        };
        assert!(observed.success());
        assert!(!macos_group_has_live_members(child.id() as i32).unwrap());
        assert!(child
            .terminate_and_drain(Duration::from_secs(5))
            .unwrap()
            .success());
        assert!(!custody.is_active());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn permission_denied_with_live_descendant_retains_custody_and_lease_for_retry() {
        use std::sync::atomic::{AtomicBool, Ordering};
        struct Lease(Arc<AtomicBool>);
        impl Drop for Lease {
            fn drop(&mut self) {
                self.0.store(true, Ordering::Release);
            }
        }

        let root = tempfile::tempdir().unwrap();
        let marker = root.path().join("descendant.pid");
        let mut command = Command::new("/bin/sh");
        command
            .args(["-c", "sleep 30 & echo $! > \"$1\"; wait", "sh"])
            .arg(&marker);
        let custody = ManagedChildCustodySlot::new();
        let mut child = ManagedChild::spawn(&mut command, custody.clone()).unwrap();
        let group = child.id() as i32;
        let descendant = wait_for(&marker);
        assert!(macos_group_has_live_members(group).unwrap());
        assert!(super::super::process::is_process_alive(descendant));

        let released = Arc::new(AtomicBool::new(false));
        child.attach_cleanup_lease(Arc::new(Lease(released.clone())));
        let force_eperm = child.force_group_signal_permission_denied.clone();
        force_eperm.store(true, Ordering::Release);
        let error = child
            .terminate_and_drain(Duration::from_secs(5))
            .unwrap_err();
        assert_eq!(error.raw_os_error(), Some(libc::EPERM));
        assert!(macos_group_has_live_members(group).unwrap());
        assert!(custody.is_active());
        assert!(!released.load(Ordering::Acquire));

        drop(child);
        assert!(custody.has_parked_child());
        assert_eq!(
            custody
                .drain(Duration::from_secs(5))
                .unwrap_err()
                .raw_os_error(),
            Some(libc::EPERM)
        );
        assert!(custody.has_parked_child());
        assert!(!released.load(Ordering::Acquire));

        force_eperm.store(false, Ordering::Release);
        assert!(custody.drain(Duration::from_secs(5)).unwrap());
        assert!(!macos_group_has_live_members(group).unwrap());
        assert!(!custody.has_parked_child());
        assert!(!custody.is_active());
        assert!(released.load(Ordering::Acquire));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn permission_denied_after_group_exits_is_accepted() {
        let mut observations = 0;
        let mut signals = 0;
        let live = macos_drain_group_once(
            42,
            |_| {
                observations += 1;
                Ok(observations == 1)
            },
            |_| {
                signals += 1;
                Err(io::Error::from_raw_os_error(libc::EPERM))
            },
        )
        .unwrap();
        assert!(!live);
        assert_eq!(observations, 2);
        assert_eq!(signals, 1);
    }

    #[test]
    fn managed_child_drains_process_group_descendants() {
        let root = tempfile::tempdir().unwrap();
        let marker = root.path().join("descendant.pid");
        let mut command = Command::new("sh");
        command
            .arg("-c")
            .arg("sleep 30 & echo $! > \"$1\"; wait")
            .arg("sh")
            .arg(&marker);
        let custody = ManagedChildCustodySlot::new();
        let mut child = ManagedChild::spawn(&mut command, custody).unwrap();
        let descendant = wait_for(&marker);
        assert!(super::super::process::is_process_alive(descendant));
        child.terminate_and_drain(Duration::from_secs(5)).unwrap();
        #[cfg(target_os = "linux")]
        assert!(!super::super::linux_group::group_has_live_members(child.id() as i32).unwrap());
        #[cfg(target_os = "macos")]
        assert!(!macos_group_has_live_members(child.id() as i32).unwrap());
    }

    #[test]
    fn cancelled_managed_child_drop_drains_group() {
        let root = tempfile::tempdir().unwrap();
        let marker = root.path().join("descendant.pid");
        let mut command = Command::new("sh");
        command
            .arg("-c")
            .arg("sleep 30 & echo $! > \"$1\"; wait")
            .arg("sh")
            .arg(&marker);
        let custody = ManagedChildCustodySlot::new();
        let child = ManagedChild::spawn(&mut command, custody.clone()).unwrap();
        let group = child.id() as i32;
        wait_for(&marker);
        drop(child);
        assert!(custody.has_parked_child());
        custody.drain(Duration::from_secs(5)).unwrap();
        #[cfg(target_os = "linux")]
        assert!(!super::super::linux_group::group_has_live_members(group).unwrap());
        #[cfg(target_os = "macos")]
        assert!(!macos_group_has_live_members(group).unwrap());
    }

    #[test]
    fn failed_observation_retains_lease_without_blocking_drop() {
        use std::sync::atomic::{AtomicBool, Ordering};
        struct Lease(Arc<AtomicBool>);
        impl Drop for Lease {
            fn drop(&mut self) {
                self.0.store(true, Ordering::Release);
            }
        }
        let mut command = Command::new("sh");
        command.args(["-c", "sleep 30"]);
        let custody = ManagedChildCustodySlot::new();
        let mut child = ManagedChild::spawn(&mut command, custody.clone()).unwrap();
        let failure = child.force_observation_failure.clone();
        let released = Arc::new(AtomicBool::new(false));
        child.attach_cleanup_lease(Arc::new(Lease(released.clone())));
        failure.store(true, Ordering::Release);
        let started = Instant::now();
        drop(child);
        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(custody.drain(Duration::from_millis(50)).is_err());
        assert!(!released.load(Ordering::Acquire));
        failure.store(false, Ordering::Release);
        custody.drain(Duration::from_secs(5)).unwrap();
        assert!(released.load(Ordering::Acquire));
    }
}

#[cfg(all(test, windows))]
mod windows_tests {
    use super::*;
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::System::JobObjects::IsProcessInJob;
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

    #[test]
    fn failed_resume_parks_suspended_job_for_bounded_retry() {
        let custody = ManagedChildCustodySlot::new();
        let mut command = Command::new("powershell");
        command
            .args(["-NoProfile", "-Command", "Start-Sleep 30"])
            .env("PUMAS_TEST_FORCE_RESUME_FAILURE", "1");
        let started = Instant::now();
        assert!(ManagedChild::spawn(&mut command, custody.clone()).is_err());
        assert!(started.elapsed() < Duration::from_secs(2));
        assert!(custody.is_cleanup_pending() && custody.has_parked_child());
        custody.drain(Duration::from_secs(10)).unwrap();
        assert!(!custody.is_active() && !custody.has_parked_child());
    }

    #[test]
    #[allow(unsafe_code)]
    fn suspended_admission_assigns_descendants_to_job_and_drains_them() {
        let root = tempfile::tempdir().unwrap();
        let marker = root.path().join("descendant.pid");
        let script = format!(
            "$p=Start-Process powershell -ArgumentList '-NoProfile','-Command','Start-Sleep 30' -PassThru; Set-Content -Path '{}' -Value $p.Id; Start-Sleep 30",
            marker.display()
        );
        let mut command = Command::new("powershell");
        command.args(["-NoProfile", "-Command", &script]);
        let custody = ManagedChildCustodySlot::new();
        let mut child = ManagedChild::spawn(&mut command, custody).unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        let descendant = loop {
            if let Ok(contents) = std::fs::read_to_string(&marker) {
                if let Ok(pid) = contents.trim().parse::<u32>() {
                    break pid;
                }
            }
            assert!(
                Instant::now() < deadline,
                "fixture descendant did not start"
            );
            std::thread::sleep(POLL_INTERVAL);
        };
        let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, descendant) };
        assert!(!process.is_null());
        let mut member = 0;
        assert_ne!(
            unsafe { IsProcessInJob(process, child.job.as_raw_handle(), &mut member) },
            0
        );
        assert_ne!(member, 0, "descendant escaped the admitted Job");
        unsafe { windows_sys::Win32::Foundation::CloseHandle(process) };
        child.terminate_and_drain(Duration::from_secs(10)).unwrap();
        assert!(!windows::terminate_and_has_members(&child.job).unwrap());
    }
}
