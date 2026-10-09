//! Mandatory Linux content-read confinement for a private audio child.
//!
//! This restricts opens by the ELF loader, Python and native extensions, not
//! merely Python imports. It is not recipe/model trust or byte immutability:
//! the source owner must retain and validate those separately. No path, wire
//! manifest, environment variable or successful probe grants audio admission.

// This private platform module owns the raw Linux Landlock/seccomp ABI.
// The standard library has no safe interface for these operations. Every file
// descriptor is owned by a File and every kernel pointer borrows live, fixed
// storage. Preparation allocates in the parent; pre_exec only makes raw calls
// after fork and retains all borrowed storage until exec or spawn refusal.
#![allow(unsafe_code)]

use std::fs::File;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};

const MIN_ABI: i32 = 6;
const READ_FILE: u64 = 1 << 2;
const READ_DIR: u64 = 1 << 3;
const EXECUTE: u64 = 1;
// ABI 6: all filesystem actions through IOCTL_DEV, TCP and both scopes.
const HANDLED_FS: u64 = (1 << 16) - 1;
const MAX_HANDLES: usize = 200_000;

#[repr(C)]
struct RulesetAttr {
    filesystem: u64,
    network: u64,
    scoped: u64,
}

#[repr(C, packed)]
struct PathRule {
    access: u64,
    parent_fd: i32,
}

#[repr(C)]
struct CapabilityHeader {
    version: u32,
    pid: i32,
}

#[repr(C)]
struct CapabilityData {
    effective: u32,
    permitted: u32,
    inheritable: u32,
}

/// A read-only capability selected by the source-owned recipe. Directories
/// permit enumeration only; they never grant recursive file-content access.
pub(crate) struct AudioReadGrant {
    file: File,
    access: u64,
}

impl AudioReadGrant {
    pub(crate) fn file(file: File, executable: bool) -> io::Result<Self> {
        if !file.metadata()?.is_file() || !readonly(&file)? {
            return Err(refusal(
                "audio content grant requires a read-only regular file",
            ));
        }
        Ok(Self {
            file,
            access: READ_FILE | if executable { EXECUTE } else { 0 },
        })
    }

    pub(crate) fn directory(file: File) -> io::Result<Self> {
        if !file.metadata()?.is_dir() || !readonly(&file)? {
            return Err(refusal(
                "audio enumeration grant requires a read-only directory",
            ));
        }
        Ok(Self {
            file,
            access: READ_DIR,
        })
    }
}

fn readonly(file: &File) -> io::Result<bool> {
    // SAFETY: F_GETFL inspects an owned, live descriptor.
    let flags = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETFL) };
    if flags < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(flags & libc::O_ACCMODE == libc::O_RDONLY)
}

/// Prepared entirely in the parent. Admission must keep original byte custody
/// through the managed child's lifetime; this ruleset is not a substitute.
pub(crate) struct AudioReadBoundary {
    ruleset: File,
    inherited: Vec<File>,
    filter: Vec<libc::sock_filter>,
}

impl AudioReadBoundary {
    pub(crate) fn supported_abi() -> io::Result<i32> {
        // SAFETY: VERSION is a read-only kernel capability query with null attr.
        let abi = unsafe {
            libc::syscall(
                libc::SYS_landlock_create_ruleset,
                std::ptr::null::<u8>(),
                0,
                1,
            )
        };
        if abi < 0 {
            return Err(io::Error::last_os_error());
        }
        if abi < MIN_ABI as libc::c_long {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "audio requires Landlock ABI >= 6",
            ));
        }
        Ok(abi as i32)
    }

    /// Stream exact grants: an installed recipe can contain more files than
    /// the process descriptor limit. Landlock owns each inode rule after add;
    /// only explicitly inherited descriptors need remain open through exec.
    pub(crate) fn prepare(
        grants: impl IntoIterator<Item = io::Result<AudioReadGrant>>,
        inherited: Vec<File>,
    ) -> io::Result<Self> {
        Self::supported_abi()?;
        let attributes = RulesetAttr {
            filesystem: HANDLED_FS,
            network: 3,
            scoped: 3,
        };
        // SAFETY: layout and length are the Linux ABI 6 ruleset structure.
        let fd = unsafe {
            libc::syscall(
                libc::SYS_landlock_create_ruleset,
                &attributes,
                std::mem::size_of::<RulesetAttr>(),
                0,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: successful create_ruleset returns a new owned descriptor.
        let ruleset = unsafe { File::from_raw_fd(fd as i32) };
        visit_grants(grants, &inherited, |grant| {
            let rule = PathRule {
                access: grant.access,
                parent_fd: grant.file.as_raw_fd(),
            };
            // SAFETY: rule has the packed Linux PATH_BENEATH ABI and live FDs.
            if unsafe {
                libc::syscall(
                    libc::SYS_landlock_add_rule,
                    ruleset.as_raw_fd(),
                    1,
                    &rule,
                    0,
                )
            } < 0
            {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        })?;
        Ok(Self {
            ruleset,
            inherited,
            filter: syscall_filter(),
        })
    }

    /// Consume into the private owner's command before ManagedChild::spawn.
    /// No additional pre-exec callback may be appended by that owner. Standard
    /// streams are new control/diagnostic pipes, never ambient inherited files.
    /// This affects the child only; it changes no system-wide security setting.
    pub(crate) fn confine_command(self, command: &mut Command) {
        command
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // SAFETY: all allocations/rules were prepared before fork. The callback
        // calls only raw syscalls/fcntl/prctl and constructs errno-only errors.
        // Captured files and filter memory stay live through exec or failure.
        unsafe {
            command.pre_exec(move || {
                if libc::syscall(libc::SYS_close_range, 3u32, u32::MAX, 4u32) < 0 {
                    return Err(io::Error::last_os_error());
                }
                for file in &self.inherited {
                    if libc::fcntl(file.as_raw_fd(), libc::F_SETFD, 0) < 0 {
                        return Err(io::Error::last_os_error());
                    }
                }
                // no_new_privs prevents gaining privilege at exec; it does not
                // remove capabilities the parent already has. Drop those too.
                let header = CapabilityHeader {
                    version: 0x2008_0522,
                    pid: 0,
                };
                let empty = [
                    CapabilityData {
                        effective: 0,
                        permitted: 0,
                        inheritable: 0,
                    },
                    CapabilityData {
                        effective: 0,
                        permitted: 0,
                        inheritable: 0,
                    },
                ];
                if libc::prctl(
                    libc::PR_CAP_AMBIENT,
                    libc::PR_CAP_AMBIENT_CLEAR_ALL,
                    0,
                    0,
                    0,
                ) < 0
                    || libc::syscall(libc::SYS_capset, &header, empty.as_ptr()) < 0
                {
                    return Err(io::Error::last_os_error());
                }
                if libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) < 0
                    || libc::syscall(
                        libc::SYS_landlock_restrict_self,
                        self.ruleset.as_raw_fd(),
                        0,
                    ) < 0
                {
                    return Err(io::Error::last_os_error());
                }
                let program = libc::sock_fprog {
                    len: self.filter.len() as u16,
                    filter: self.filter.as_ptr().cast_mut(),
                };
                if libc::prctl(libc::PR_SET_SECCOMP, 2, &program, 0, 0) < 0 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }
}

fn visit_grants(
    grants: impl IntoIterator<Item = io::Result<AudioReadGrant>>,
    inherited: &[File],
    mut add: impl FnMut(&AudioReadGrant) -> io::Result<()>,
) -> io::Result<()> {
    use std::os::unix::fs::MetadataExt;
    if inherited.len() > MAX_HANDLES {
        return Err(refusal(
            "audio read boundary has an invalid capability count",
        ));
    }
    let mut remaining = std::collections::BTreeSet::new();
    for file in inherited {
        let metadata = file.metadata()?;
        if file.as_raw_fd() < 3 || (!metadata.is_file() && !metadata.is_dir()) || !readonly(file)? {
            return Err(refusal(
                "audio inherited capability is not a read-only file or directory",
            ));
        }
        remaining.insert((metadata.dev(), metadata.ino()));
    }
    let mut count = 0usize;
    for grant in grants {
        count += 1;
        if count > MAX_HANDLES {
            return Err(refusal(
                "audio read boundary has an invalid capability count",
            ));
        }
        let grant = grant?;
        let metadata = grant.file.metadata()?;
        remaining.remove(&(metadata.dev(), metadata.ino()));
        add(&grant)?;
        // The capability closes here after the kernel has retained its rule.
    }
    if count == 0 || !remaining.is_empty() {
        return Err(refusal(
            "audio inherited capability is outside the selected read set",
        ));
    }
    Ok(())
}

fn syscall_filter() -> Vec<libc::sock_filter> {
    const LOAD: u16 = 0x20;
    const EQUAL: u16 = 0x15;
    const SET: u16 = 0x45;
    const AND: u16 = 0x54;
    const RETURN: u16 = 0x06;
    const DENY: u32 = 0x0005_0000 | libc::EPERM as u32;
    let instruction = |code, jt, jf, k| libc::sock_filter { code, jt, jf, k };
    let mut code = vec![
        instruction(LOAD, 0, 0, 4),             // seccomp_data.arch
        instruction(EQUAL, 1, 0, 0xc000_003e),  // AUDIT_ARCH_X86_64
        instruction(RETURN, 0, 0, 0x8000_0000), // KILL_PROCESS for another ABI
        instruction(LOAD, 0, 0, 0),             // seccomp_data.nr
        instruction(SET, 0, 1, 0x4000_0000),    // refuse x32 syscall dispatch
        instruction(RETURN, 0, 0, DENY),
    ];
    for number in [
        libc::SYS_socket,
        libc::SYS_connect,
        libc::SYS_bind,
        libc::SYS_listen,
        libc::SYS_accept,
        libc::SYS_accept4,
        // No ambient System V or POSIX IPC object may introduce another read source.
        libc::SYS_shmget,
        libc::SYS_shmat,
        libc::SYS_shmctl,
        libc::SYS_msgget,
        libc::SYS_msgrcv,
        libc::SYS_msgsnd,
        libc::SYS_msgctl,
        libc::SYS_semget,
        libc::SYS_semctl,
        libc::SYS_semop,
        libc::SYS_semtimedop,
        libc::SYS_mq_open,
        libc::SYS_mq_unlink,
        libc::SYS_mq_timedsend,
        libc::SYS_mq_timedreceive,
        libc::SYS_mq_notify,
        libc::SYS_mq_getsetattr,
        libc::SYS_ptrace,
        libc::SYS_process_vm_readv,
        libc::SYS_process_vm_writev,
        libc::SYS_pidfd_open,
        libc::SYS_pidfd_getfd,
        libc::SYS_open_by_handle_at,
        libc::SYS_name_to_handle_at,
        libc::SYS_io_uring_setup,
        libc::SYS_fanotify_init,
        libc::SYS_fanotify_mark,
        libc::SYS_keyctl,
        libc::SYS_add_key,
        libc::SYS_request_key,
        libc::SYS_mount,
        libc::SYS_umount2,
        libc::SYS_pivot_root,
        libc::SYS_chroot,
        libc::SYS_unshare,
        libc::SYS_setns,
        libc::SYS_fsopen,
        libc::SYS_fsmount,
        libc::SYS_fspick,
        libc::SYS_open_tree,
        libc::SYS_move_mount,
        libc::SYS_mount_setattr,
        libc::SYS_bpf,
        libc::SYS_perf_event_open,
        libc::SYS_kexec_load,
        libc::SYS_kexec_file_load,
        libc::SYS_init_module,
        libc::SYS_finit_module,
        libc::SYS_delete_module,
        libc::SYS_setsid,
        libc::SYS_setpgid,
    ] {
        code.push(instruction(EQUAL, 0, 1, number as u32));
        code.push(instruction(RETURN, 0, 0, DENY));
    }
    // glibc may use clone3 first for threads. ENOSYS allows its safe clone
    // fallback; inspecting a caller-writable clone3 pointer would be racy.
    code.push(instruction(EQUAL, 0, 1, libc::SYS_clone3 as u32));
    code.push(instruction(RETURN, 0, 0, 0x0005_0000 | libc::ENOSYS as u32));
    // asyncio requires a private wakeup pair. Only connected local stream
    // pairs are allowed; socket/connect/bind/listen/accept remain denied, so a
    // pair cannot become an ambient IPC/network endpoint. No socket is inherited.
    code.extend([
        instruction(EQUAL, 0, 11, libc::SYS_socketpair as u32),
        instruction(LOAD, 0, 0, 16), // domain
        instruction(EQUAL, 1, 0, libc::AF_UNIX as u32),
        instruction(RETURN, 0, 0, DENY),
        instruction(LOAD, 0, 0, 24), // type plus nonblocking/close-on-exec flags
        instruction(
            AND,
            0,
            0,
            !(libc::SOCK_CLOEXEC | libc::SOCK_NONBLOCK) as u32,
        ),
        instruction(EQUAL, 1, 0, libc::SOCK_STREAM as u32),
        instruction(RETURN, 0, 0, DENY),
        instruction(LOAD, 0, 0, 32), // protocol
        instruction(EQUAL, 1, 0, 0),
        instruction(RETURN, 0, 0, DENY),
        instruction(RETURN, 0, 0, 0x7fff_0000),
        instruction(EQUAL, 0, 4, libc::SYS_clone as u32),
        instruction(LOAD, 0, 0, 16), // clone flags, low 32 bits of args[0]
        instruction(
            AND,
            0,
            0,
            (libc::CLONE_NEWCGROUP
                | libc::CLONE_NEWIPC
                | libc::CLONE_NEWNET
                | libc::CLONE_NEWNS
                | libc::CLONE_NEWPID
                | libc::CLONE_NEWTIME
                | libc::CLONE_NEWUSER
                | libc::CLONE_NEWUTS
                | libc::CLONE_PARENT) as u32,
        ),
        instruction(EQUAL, 1, 0, 0),
        instruction(RETURN, 0, 0, DENY),
        instruction(RETURN, 0, 0, 0x7fff_0000),
    ]);
    code
}

fn refusal(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Interpret the actual small generated classic-BPF program. These are
    // filter-construction tests, not a claim of kernel enforcement.
    fn verdict(arch: u32, syscall: u32, flags: u32) -> u32 {
        verdict_args(arch, syscall, [flags, 0, 0])
    }

    fn verdict_args(arch: u32, syscall: u32, args: [u32; 3]) -> u32 {
        let code = syscall_filter();
        let (mut pc, mut accumulator) = (0usize, 0u32);
        while pc < code.len() {
            let op = code[pc];
            pc += 1;
            match op.code {
                0x20 => {
                    accumulator = match op.k {
                        0 => syscall,
                        4 => arch,
                        16 => args[0],
                        24 => args[1],
                        32 => args[2],
                        _ => panic!("unexpected seccomp data access"),
                    }
                }
                0x15 => pc += if accumulator == op.k { op.jt } else { op.jf } as usize,
                0x45 => {
                    pc += if accumulator & op.k != 0 {
                        op.jt
                    } else {
                        op.jf
                    } as usize
                }
                0x54 => accumulator &= op.k,
                0x06 => return op.k,
                _ => panic!("unexpected filter instruction"),
            }
        }
        panic!("filter fell through")
    }

    #[test]
    fn filter_refuses_other_abis_and_native_escape_paths() {
        assert_eq!(verdict(0x4000_0003, libc::SYS_read as u32, 0), 0x8000_0000);
        for number in [
            libc::SYS_socket,
            libc::SYS_connect,
            libc::SYS_bind,
            libc::SYS_listen,
            libc::SYS_accept,
            libc::SYS_accept4,
            libc::SYS_shmget,
            libc::SYS_shmat,
            libc::SYS_shmctl,
            libc::SYS_msgget,
            libc::SYS_msgrcv,
            libc::SYS_msgsnd,
            libc::SYS_msgctl,
            libc::SYS_semget,
            libc::SYS_semctl,
            libc::SYS_semop,
            libc::SYS_semtimedop,
            libc::SYS_mq_open,
            libc::SYS_mq_unlink,
            libc::SYS_mq_timedsend,
            libc::SYS_mq_timedreceive,
            libc::SYS_mq_notify,
            libc::SYS_mq_getsetattr,
            libc::SYS_ptrace,
            libc::SYS_pidfd_getfd,
            libc::SYS_process_vm_readv,
            libc::SYS_io_uring_setup,
            libc::SYS_fanotify_init,
            libc::SYS_fanotify_mark,
            libc::SYS_keyctl,
            libc::SYS_add_key,
            libc::SYS_request_key,
            libc::SYS_open_by_handle_at,
            libc::SYS_unshare,
            libc::SYS_setns,
            libc::SYS_mount,
            libc::SYS_setsid,
            libc::SYS_setpgid,
        ] {
            assert_eq!(
                verdict(0xc000_003e, number as u32, 0),
                0x50000 | libc::EPERM as u32
            );
        }
        assert_eq!(
            verdict(0xc000_003e, 0x4000_0000 | libc::SYS_read as u32, 0),
            0x50000 | libc::EPERM as u32
        );
    }

    #[test]
    fn filter_allows_threads_but_not_namespace_or_parent_escape() {
        for flags in [
            0,
            libc::SIGCHLD,
            libc::CLONE_VM | libc::CLONE_THREAD | libc::CLONE_SIGHAND,
        ] {
            assert_eq!(
                verdict(0xc000_003e, libc::SYS_clone as u32, flags as u32),
                0x7fff_0000
            );
        }
        for flag in [
            libc::CLONE_NEWUSER,
            libc::CLONE_NEWNS,
            libc::CLONE_NEWPID,
            libc::CLONE_PARENT,
        ] {
            assert_eq!(
                verdict(0xc000_003e, libc::SYS_clone as u32, flag as u32),
                0x50000 | libc::EPERM as u32
            );
        }
        assert_eq!(
            verdict(0xc000_003e, libc::SYS_clone3 as u32, 0),
            0x50000 | libc::ENOSYS as u32
        );
        for number in [
            libc::SYS_read,
            libc::SYS_openat,
            libc::SYS_mmap,
            libc::SYS_futex,
            libc::SYS_execve,
        ] {
            assert_eq!(verdict(0xc000_003e, number as u32, 0), 0x7fff_0000);
        }
    }

    #[test]
    fn filter_allows_only_private_stream_wakeup_pairs() {
        for flags in [
            0,
            libc::SOCK_CLOEXEC,
            libc::SOCK_NONBLOCK,
            libc::SOCK_CLOEXEC | libc::SOCK_NONBLOCK,
        ] {
            assert_eq!(
                verdict_args(
                    0xc000_003e,
                    libc::SYS_socketpair as u32,
                    [libc::AF_UNIX as u32, (libc::SOCK_STREAM | flags) as u32, 0]
                ),
                0x7fff_0000
            );
        }
        for args in [
            [libc::AF_INET as u32, libc::SOCK_STREAM as u32, 0],
            [libc::AF_UNIX as u32, libc::SOCK_DGRAM as u32, 0],
            [libc::AF_UNIX as u32, libc::SOCK_SEQPACKET as u32, 0],
            [libc::AF_UNIX as u32, libc::SOCK_STREAM as u32, 1],
            [libc::AF_UNIX as u32, libc::SOCK_STREAM as u32 | 0x1000, 0],
        ] {
            assert_eq!(
                verdict_args(0xc000_003e, libc::SYS_socketpair as u32, args),
                0x50000 | libc::EPERM as u32
            );
        }
    }

    #[test]
    fn grants_refuse_writable_and_mistyped_capabilities() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("selected");
        let writable = File::create(&path).unwrap();
        assert!(AudioReadGrant::file(writable, false).is_err());
        assert!(AudioReadGrant::file(File::open(root.path()).unwrap(), false).is_err());
        assert!(AudioReadGrant::directory(File::open(&path).unwrap()).is_err());
        assert_eq!(
            AudioReadGrant::directory(File::open(root.path()).unwrap())
                .unwrap()
                .access,
            READ_DIR
        );
        assert_eq!(
            AudioReadGrant::file(File::open(&path).unwrap(), false)
                .unwrap()
                .access,
            READ_FILE
        );
    }

    #[test]
    fn inherited_unselected_inode_cannot_complete_rule_selection() {
        let root = tempfile::tempdir().unwrap();
        let selected = root.path().join("selected");
        let other = root.path().join("other");
        std::fs::write(&selected, b"same bytes").unwrap();
        std::fs::write(&other, b"same bytes").unwrap();
        let grant = AudioReadGrant::file(File::open(selected).unwrap(), false).unwrap();
        let result = visit_grants([Ok(grant)], &[File::open(other).unwrap()], |_| Ok(()));
        assert_eq!(
            result.err().unwrap().kind(),
            io::ErrorKind::PermissionDenied
        );
    }

    #[test]
    fn grant_stream_closes_each_descriptor_before_opening_the_next() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("selected");
        std::fs::write(&path, b"fixture").unwrap();
        let path = path.canonicalize().unwrap();
        let previous = std::cell::Cell::new(None);
        let assert_previous_closed = || {
            if let Some(fd) = previous.get() {
                // Another concurrent test may reuse the number. It cannot own
                // this test's private file, so compare the selected identity's
                // private path rather than a racy process-wide FD count.
                assert_ne!(
                    std::fs::read_link(format!("/proc/self/fd/{fd}")).ok(),
                    Some(path.clone())
                );
            }
        };
        let grants = (0..32).map(|_| {
            assert_previous_closed();
            let grant = AudioReadGrant::file(File::open(&path)?, false)?;
            previous.set(Some(grant.file.as_raw_fd()));
            Ok(grant)
        });
        visit_grants(grants, &[], |_| Ok(())).unwrap();
        assert_previous_closed();
    }

    #[test]
    fn failed_grant_stream_does_not_return_a_completed_selection() {
        let result = visit_grants(
            [Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "fixture read failure",
            ))],
            &[],
            |_| panic!("failed grant cannot enter kernel selection"),
        );
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::Interrupted);
        assert!(visit_grants(std::iter::empty(), &[], |_| Ok(())).is_err());
    }

    #[test]
    fn actual_kernel_preflight_never_downgrades_unavailable_enforcement() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("selected");
        std::fs::write(&path, b"fixture only").unwrap();
        let grant = AudioReadGrant::file(File::open(path).unwrap(), false).unwrap();
        let supported = AudioReadBoundary::supported_abi();
        let prepared = AudioReadBoundary::prepare([Ok(grant)], vec![]);
        match supported {
            Err(error) => {
                let refused = prepared
                    .err()
                    .expect("unavailable kernel cannot produce a boundary");
                assert_eq!(refused.kind(), error.kind());
                eprintln!("native enforcement unavailable: {error}; refusal verified, no positive qualification");
            }
            Ok(abi) => {
                assert!(abi >= MIN_ABI);
                // The availability probe is not enforcement acceptance. A
                // later rule creation denial must also stay a refusal.
                if let Err(error) = prepared {
                    eprintln!("ruleset creation refused: {error}; no positive qualification");
                }
            }
        }
    }

    /// Opt-in mechanism acceptance on a supported host. Mapped libraries below
    /// are a test selection, NEVER a production recipe or discovered allowlist.
    #[test]
    #[ignore = "requires a qualification host exposing Landlock ABI >= 6"]
    fn kernel_exec_denies_unselected_reads_and_ambient_descriptors() {
        AudioReadBoundary::supported_abi().expect("required Landlock ABI is unavailable");
        let root = tempfile::tempdir().unwrap();
        let selected = root.path().join("selected");
        let denied = root.path().join("denied");
        std::fs::write(&selected, b"selected fixture").unwrap();
        std::fs::write(&denied, b"ambient fixture").unwrap();
        let ambient = File::open(&denied).unwrap();
        // Deliberately simulate a preexisting descriptor lacking CLOEXEC.
        // SAFETY: only this test's owned descriptor flags are modified.
        assert_eq!(
            unsafe { libc::fcntl(ambient.as_raw_fd(), libc::F_SETFD, 0) },
            0
        );
        let mut paths = std::collections::BTreeSet::new();
        for line in std::fs::read_to_string("/proc/self/maps").unwrap().lines() {
            let columns: Vec<_> = line.split_whitespace().collect();
            if columns.len() >= 6 && columns[5].starts_with('/') {
                paths.insert(columns[5..].join(" "));
            }
        }
        let mut grants: Vec<_> = paths
            .into_iter()
            .map(|path| AudioReadGrant::file(File::open(path).unwrap(), true).unwrap())
            .collect();
        grants.push(AudioReadGrant::file(File::open(&selected).unwrap(), false).unwrap());
        // Enumeration must not recursively grant the sibling's file contents.
        grants.push(AudioReadGrant::directory(File::open(root.path()).unwrap()).unwrap());
        let boundary = AudioReadBoundary::prepare(grants.into_iter().map(Ok), vec![]).unwrap();
        let mut command = Command::new(std::env::current_exe().unwrap());
        command.args([
            "--ignored",
            "--exact",
            &format!(
                "{}::kernel_probe_child",
                module_path!().split_once("::").unwrap().1
            ),
            "--test-threads=1",
        ]);
        boundary.confine_command(&mut command);
        use std::os::unix::fs::MetadataExt;
        let identity = ambient.metadata().unwrap();
        command
            .env("PUMAS_BOUNDARY_FIXTURE", root.path())
            .env("PUMAS_BOUNDARY_AMBIENT_FD", ambient.as_raw_fd().to_string())
            .env("PUMAS_BOUNDARY_AMBIENT_DEV", identity.dev().to_string())
            .env("PUMAS_BOUNDARY_AMBIENT_INO", identity.ino().to_string());
        let result = command.output().unwrap();
        assert!(
            result.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(String::from_utf8_lossy(&result.stdout).contains("1 passed"));
    }

    #[test]
    #[ignore = "private child of the opt-in kernel enforcement test"]
    fn kernel_probe_child() {
        let root = std::path::PathBuf::from(
            std::env::var_os("PUMAS_BOUNDARY_FIXTURE").expect("private fixture child only"),
        );
        let fd: i32 = std::env::var("PUMAS_BOUNDARY_AMBIENT_FD")
            .unwrap()
            .parse()
            .unwrap();
        let dev: u64 = std::env::var("PUMAS_BOUNDARY_AMBIENT_DEV")
            .unwrap()
            .parse()
            .unwrap();
        let ino: u64 = std::env::var("PUMAS_BOUNDARY_AMBIENT_INO")
            .unwrap()
            .parse()
            .unwrap();
        let mut info = std::mem::MaybeUninit::<libc::stat>::uninit();
        // SAFETY: fstat either initializes this correctly sized buffer or fails.
        if unsafe { libc::fstat(fd, info.as_mut_ptr()) } == 0 {
            // SAFETY: successful fstat above initialized the complete stat value.
            let info = unsafe { info.assume_init() };
            assert_ne!(
                (info.st_dev, info.st_ino),
                (dev, ino),
                "ambient descriptor survived exec"
            );
        }
        assert_eq!(
            std::fs::read(root.join("selected")).unwrap(),
            b"selected fixture"
        );
        assert_eq!(
            std::fs::read(root.join("denied")).unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );
        assert_eq!(
            std::fs::write(root.join("selected"), b"mutation")
                .unwrap_err()
                .kind(),
            io::ErrorKind::PermissionDenied
        );
        // SAFETY: socket is a fresh test-owned resource if an unexpected success
        // occurs; close it before failing the assertion.
        let socket = unsafe { libc::socket(libc::AF_INET, libc::SOCK_STREAM, 0) };
        if socket >= 0 {
            // SAFETY: unexpected socket success created this test-owned descriptor.
            unsafe {
                libc::close(socket);
            }
        }
        assert!(socket < 0, "network creation escaped seccomp");
        assert_eq!(io::Error::last_os_error().raw_os_error(), Some(libc::EPERM));
        // The useful asyncio wakeup primitive remains available after exec.
        use std::io::{Read, Write};
        let (mut sender, mut receiver) = std::os::unix::net::UnixStream::pair().unwrap();
        sender.write_all(b"wake").unwrap();
        let mut wake = [0; 4];
        receiver.read_exact(&mut wake).unwrap();
        assert_eq!(&wake, b"wake");
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 2);
        // Invalid IDs/names make these probes non-mutating even if a broken
        // filter accidentally allows them. EPERM must come from confinement.
        // SAFETY: msgrcv gets a valid zero-length buffer and an invalid queue ID.
        let mut message_type = 0 as libc::c_long;
        assert_eq!(
            unsafe {
                libc::msgrcv(
                    -1,
                    (&mut message_type as *mut libc::c_long).cast(),
                    0,
                    0,
                    libc::IPC_NOWAIT,
                )
            },
            -1
        );
        assert_eq!(io::Error::last_os_error().raw_os_error(), Some(libc::EPERM));
        // Probe the kernel directly: glibc rejects an empty name before making
        // a syscall, which would test its EINVAL rather than seccomp denial.
        // SAFETY: the NUL-only name is live; O_RDONLY cannot create a queue.
        assert_eq!(
            unsafe {
                libc::syscall(
                    libc::SYS_mq_open,
                    c"".as_ptr(),
                    libc::O_RDONLY,
                    0,
                    std::ptr::null::<libc::mq_attr>(),
                )
            },
            -1
        );
        assert_eq!(io::Error::last_os_error().raw_os_error(), Some(libc::EPERM));
        let denied = root.join("denied");
        assert_eq!(
            std::thread::spawn(move || std::fs::read(denied).unwrap_err().kind())
                .join()
                .unwrap(),
            io::ErrorKind::PermissionDenied
        );
    }
}
