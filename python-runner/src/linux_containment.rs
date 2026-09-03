//! Linux-native outer containment for the standalone Python runner.

use std::{ffi::CString, fs::File, io, os::unix::ffi::OsStrExt, path::Path};

use anyhow::{Context, Result, anyhow};
use serde::Serialize;

use crate::PythonExecutionResult;

const LANDLOCK_CREATE_RULESET_VERSION: libc::c_int = 1;
const LANDLOCK_RULE_PATH_BENEATH: libc::c_int = 1;
const LANDLOCK_ACCESS_FS_EXECUTE: u64 = 1 << 0;
const LANDLOCK_ACCESS_FS_WRITE_FILE: u64 = 1 << 1;
const LANDLOCK_ACCESS_FS_READ_FILE: u64 = 1 << 2;
const LANDLOCK_ACCESS_FS_READ_DIR: u64 = 1 << 3;
const LANDLOCK_ACCESS_FS_REMOVE_DIR: u64 = 1 << 4;
const LANDLOCK_ACCESS_FS_REMOVE_FILE: u64 = 1 << 5;
const LANDLOCK_ACCESS_FS_MAKE_CHAR: u64 = 1 << 6;
const LANDLOCK_ACCESS_FS_MAKE_DIR: u64 = 1 << 7;
const LANDLOCK_ACCESS_FS_MAKE_REG: u64 = 1 << 8;
const LANDLOCK_ACCESS_FS_MAKE_SOCK: u64 = 1 << 9;
const LANDLOCK_ACCESS_FS_MAKE_FIFO: u64 = 1 << 10;
const LANDLOCK_ACCESS_FS_MAKE_BLOCK: u64 = 1 << 11;
const LANDLOCK_ACCESS_FS_MAKE_SYM: u64 = 1 << 12;
const LANDLOCK_ACCESS_FS_REFER: u64 = 1 << 13;
const LANDLOCK_ACCESS_FS_TRUNCATE: u64 = 1 << 14;
const LANDLOCK_BASE_RIGHTS: u64 = LANDLOCK_ACCESS_FS_EXECUTE
    | LANDLOCK_ACCESS_FS_WRITE_FILE
    | LANDLOCK_ACCESS_FS_READ_FILE
    | LANDLOCK_ACCESS_FS_READ_DIR
    | LANDLOCK_ACCESS_FS_REMOVE_DIR
    | LANDLOCK_ACCESS_FS_REMOVE_FILE
    | LANDLOCK_ACCESS_FS_MAKE_CHAR
    | LANDLOCK_ACCESS_FS_MAKE_DIR
    | LANDLOCK_ACCESS_FS_MAKE_REG
    | LANDLOCK_ACCESS_FS_MAKE_SOCK
    | LANDLOCK_ACCESS_FS_MAKE_FIFO
    | LANDLOCK_ACCESS_FS_MAKE_BLOCK
    | LANDLOCK_ACCESS_FS_MAKE_SYM;
const LANDLOCK_READ_RIGHTS: u64 = LANDLOCK_ACCESS_FS_READ_FILE | LANDLOCK_ACCESS_FS_READ_DIR;
const ADDRESS_SPACE_LIMIT_BYTES: libc::rlim_t = 8 * 1_024 * 1_024 * 1_024;
const DATA_LIMIT_BYTES: libc::rlim_t = 768 * 1_024 * 1_024;
const CPU_LIMIT_SECONDS: libc::rlim_t = 120;
const FILE_LIMIT_BYTES: libc::rlim_t = 1_024 * 1_024;
const OPEN_FILE_LIMIT: libc::rlim_t = 64;
const SECCOMP_MODE_FILTER_OPERATION: libc::c_ulong = 1;
const SECCOMP_FILTER_FLAG_TSYNC: libc::c_ulong = 1;
const SECCOMP_RET_KILL_PROCESS: u32 = 0x8000_0000;
const SECCOMP_RET_ALLOW: u32 = 0x7fff_0000;
const SECCOMP_RET_ERRNO: u32 = 0x0005_0000;
const BPF_LOAD_WORD_ABSOLUTE: u16 = 0x20;
const BPF_JUMP_EQUAL: u16 = 0x15;
const BPF_AND: u16 = 0x54;
const BPF_RETURN: u16 = 0x06;
const SECCOMP_ARCH_OFFSET: u32 = 4;
const SECCOMP_SYSCALL_OFFSET: u32 = 0;
const SECCOMP_ARGUMENT_ZERO_OFFSET: u32 = 16;

#[cfg(target_arch = "x86_64")]
const AUDIT_ARCH_NATIVE: u32 = 0xc000_003e;
#[cfg(target_arch = "aarch64")]
const AUDIT_ARCH_NATIVE: u32 = 0xc000_00b7;
#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
compile_error!("the Linux containment proof supports x86_64 and aarch64");

#[repr(C)]
struct LandlockRulesetAttr {
    handled_access_fs: u64,
}

#[repr(C)]
struct LandlockPathBeneathAttr {
    allowed_access: u64,
    parent_fd: libc::c_int,
}

/// Path-free evidence emitted only by the development containment proof.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinuxContainmentEvidence {
    #[serde(skip)]
    diagnostic_stages: bool,
    /// The child received only its private workspace environment variable.
    pub environment_isolated: bool,
    /// Native executable launch was denied after runner startup.
    pub exec_denied: bool,
    /// Landlock denied the proof's host-owned fixture.
    pub landlock_denied_host_fixture: bool,
    /// Native IPv4 socket creation was denied.
    pub network_denied: bool,
    /// Kernel parent-close termination is armed for the runner.
    pub parent_death_signal: bool,
    /// Native process creation was denied while threads remain usable.
    pub process_creation_denied: bool,
    /// Address-space, data, CPU, file-size, and descriptor ceilings are active.
    pub resource_limits: bool,
    /// The exact configured runtime remains readable.
    pub runtime_readable: bool,
    /// Fixed successful proof status.
    pub status: &'static str,
    /// The exact staged workspace remains readable.
    pub workspace_readable: bool,
}

/// One contained execution plus its path-free native-boundary evidence.
pub struct LinuxContainedExecution {
    /// Evidence collected after applying Landlock, seccomp, rlimits, and parent-close policy.
    pub evidence: LinuxContainmentEvidence,
    /// The unchanged bounded Python execution result.
    pub result: PythonExecutionResult,
}

/// Applies the filesystem, resource, and parent-close boundary before worker creation.
pub(crate) fn enter_filesystem_boundary(
    runtime: &Path,
    workspace: &Path,
    denied_fixture: Option<&Path>,
) -> Result<LinuxContainmentEvidence> {
    let diagnostic_stages = denied_fixture.is_some();
    if std::fs::read_dir("/proc/self/task")?.count() != 1 {
        return Err(anyhow!(
            "the runner was not single-threaded before Landlock"
        ));
    }
    mark_stage(diagnostic_stages, "preflight");
    let expected_parent = unsafe { libc::getppid() };
    set_parent_death_signal(expected_parent)?;
    mark_stage(diagnostic_stages, "parent");
    apply_landlock(runtime, workspace)?;
    mark_stage(diagnostic_stages, "landlock");

    let runtime_readable = File::open(runtime.join("LICENSE")).is_ok();
    let workspace_readable = File::open(workspace.join("main.py")).is_ok();
    let landlock_denied_host_fixture = denied_fixture.is_some_and(permission_denied);
    let environment_names: Vec<_> = std::env::vars_os().map(|(name, _)| name).collect();
    let environment_isolated = environment_names == [std::ffi::OsString::from("TMPDIR")];
    mark_stage(diagnostic_stages, "filesystem");

    Ok(LinuxContainmentEvidence {
        diagnostic_stages,
        environment_isolated,
        exec_denied: false,
        landlock_denied_host_fixture,
        network_denied: false,
        parent_death_signal: parent_death_signal_is_armed(),
        process_creation_denied: false,
        resource_limits: false,
        runtime_readable,
        status: "ok",
        workspace_readable,
    })
}

/// Applies seccomp to every runner thread and records direct denial probes.
pub(crate) fn restrict_syscalls(evidence: &mut LinuxContainmentEvidence) -> Result<()> {
    mark_stage(evidence.diagnostic_stages, "deadline");
    set_resource_limits()?;
    evidence.resource_limits = resource_limits_are_active();
    mark_stage(evidence.diagnostic_stages, "rlimits");
    install_seccomp()?;
    mark_stage(evidence.diagnostic_stages, "seccomp");
    evidence.network_denied = syscall_denied(
        libc::SYS_socket,
        &[libc::AF_INET.into(), libc::SOCK_STREAM.into(), 0],
    );
    evidence.process_creation_denied = clone_process_denied() && clone3_denied();
    evidence.exec_denied = exec_process_denied();
    mark_stage(evidence.diagnostic_stages, "probes");
    Ok(())
}

fn mark_stage(enabled: bool, stage: &str) {
    if enabled {
        eprintln!("BOTTIE_LINUX_STAGE={stage}");
    }
}

fn set_parent_death_signal(expected_parent: libc::pid_t) -> Result<()> {
    if unsafe { libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) } != 0 {
        return Err(io::Error::last_os_error()).context("could not arm parent-close termination");
    }
    if unsafe { libc::getppid() } != expected_parent {
        return Err(anyhow!("the containment parent exited during startup"));
    }
    Ok(())
}

fn set_resource_limits() -> Result<()> {
    for (resource, limit) in [
        (libc::RLIMIT_AS, ADDRESS_SPACE_LIMIT_BYTES),
        (libc::RLIMIT_DATA, DATA_LIMIT_BYTES),
        (libc::RLIMIT_CPU, CPU_LIMIT_SECONDS),
        (libc::RLIMIT_FSIZE, FILE_LIMIT_BYTES),
        (libc::RLIMIT_NOFILE, OPEN_FILE_LIMIT),
    ] {
        let value = libc::rlimit {
            rlim_cur: limit,
            rlim_max: limit,
        };
        if unsafe { libc::setrlimit(resource, &value) } != 0 {
            return Err(io::Error::last_os_error())
                .context("could not apply a native resource limit");
        }
    }
    Ok(())
}

fn resource_limits_are_active() -> bool {
    [
        (libc::RLIMIT_AS, ADDRESS_SPACE_LIMIT_BYTES),
        (libc::RLIMIT_DATA, DATA_LIMIT_BYTES),
        (libc::RLIMIT_CPU, CPU_LIMIT_SECONDS),
        (libc::RLIMIT_FSIZE, FILE_LIMIT_BYTES),
        (libc::RLIMIT_NOFILE, OPEN_FILE_LIMIT),
    ]
    .into_iter()
    .all(|(resource, expected)| {
        let mut value = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        (unsafe { libc::getrlimit(resource, &mut value) }) == 0
            && value.rlim_cur <= expected
            && value.rlim_max <= expected
    })
}

fn handled_landlock_rights(abi: libc::c_long) -> u64 {
    let mut rights = LANDLOCK_BASE_RIGHTS;
    if abi >= 2 {
        rights |= LANDLOCK_ACCESS_FS_REFER;
    }
    if abi >= 3 {
        rights |= LANDLOCK_ACCESS_FS_TRUNCATE;
    }
    rights
}

fn apply_landlock(runtime: &Path, workspace: &Path) -> Result<()> {
    let abi = unsafe {
        libc::syscall(
            libc::SYS_landlock_create_ruleset,
            std::ptr::null::<LandlockRulesetAttr>(),
            0,
            LANDLOCK_CREATE_RULESET_VERSION,
        )
    };
    if abi < 1 {
        return Err(anyhow!("Landlock is unavailable on this Linux host"));
    }
    let ruleset_attr = LandlockRulesetAttr {
        handled_access_fs: handled_landlock_rights(abi),
    };
    let ruleset = unsafe {
        libc::syscall(
            libc::SYS_landlock_create_ruleset,
            &ruleset_attr,
            std::mem::size_of::<LandlockRulesetAttr>(),
            0,
        )
    };
    if ruleset < 0 {
        return Err(io::Error::last_os_error()).context("could not create the Landlock ruleset");
    }
    let ruleset = OwnedFd(ruleset as libc::c_int);
    add_landlock_path(ruleset.0, runtime, LANDLOCK_READ_RIGHTS)?;
    add_landlock_path(ruleset.0, workspace, LANDLOCK_READ_RIGHTS)?;
    if unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } != 0 {
        return Err(io::Error::last_os_error()).context("could not disable privilege gain");
    }
    if unsafe { libc::syscall(libc::SYS_landlock_restrict_self, ruleset.0, 0) } != 0 {
        return Err(io::Error::last_os_error()).context("could not enforce the Landlock ruleset");
    }
    Ok(())
}

fn add_landlock_path(ruleset: libc::c_int, path: &Path, rights: u64) -> Result<()> {
    let encoded = CString::new(path.as_os_str().as_bytes())
        .context("containment path contained a NUL byte")?;
    let descriptor = unsafe { libc::open(encoded.as_ptr(), libc::O_PATH | libc::O_CLOEXEC) };
    if descriptor < 0 {
        return Err(io::Error::last_os_error()).context("could not open a Landlock path");
    }
    let descriptor = OwnedFd(descriptor);
    let rule = LandlockPathBeneathAttr {
        allowed_access: rights,
        parent_fd: descriptor.0,
    };
    if unsafe {
        libc::syscall(
            libc::SYS_landlock_add_rule,
            ruleset,
            LANDLOCK_RULE_PATH_BENEATH,
            &rule,
            0,
        )
    } != 0
    {
        return Err(io::Error::last_os_error()).context("could not add a Landlock path rule");
    }
    Ok(())
}

struct OwnedFd(libc::c_int);

impl Drop for OwnedFd {
    fn drop(&mut self) {
        unsafe { libc::close(self.0) };
    }
}

fn permission_denied(path: &Path) -> bool {
    File::open(path).is_err_and(|error| error.kind() == io::ErrorKind::PermissionDenied)
}

fn parent_death_signal_is_armed() -> bool {
    let mut signal: libc::c_int = 0;
    (unsafe { libc::prctl(libc::PR_GET_PDEATHSIG, &mut signal) }) == 0 && signal == libc::SIGKILL
}

fn filter_statement(code: u16, value: u32) -> libc::sock_filter {
    libc::sock_filter {
        code,
        jt: 0,
        jf: 0,
        k: value,
    }
}

fn filter_jump(value: u32, true_offset: u8, false_offset: u8) -> libc::sock_filter {
    libc::sock_filter {
        code: BPF_JUMP_EQUAL,
        jt: true_offset,
        jf: false_offset,
        k: value,
    }
}

fn install_seccomp() -> Result<()> {
    let denied = (SECCOMP_RET_ERRNO | libc::EPERM as u32) as u32;
    let unavailable = (SECCOMP_RET_ERRNO | libc::ENOSYS as u32) as u32;
    let mut filter = vec![
        filter_statement(BPF_LOAD_WORD_ABSOLUTE, SECCOMP_ARCH_OFFSET),
        filter_jump(AUDIT_ARCH_NATIVE, 1, 0),
        filter_statement(BPF_RETURN, SECCOMP_RET_KILL_PROCESS),
        filter_statement(BPF_LOAD_WORD_ABSOLUTE, SECCOMP_SYSCALL_OFFSET),
        filter_jump(libc::SYS_clone3 as u32, 0, 1),
        filter_statement(BPF_RETURN, unavailable),
    ];
    for syscall in denied_syscalls() {
        filter.push(filter_jump(syscall as u32, 0, 1));
        filter.push(filter_statement(BPF_RETURN, denied));
    }
    filter.extend([
        filter_jump(libc::SYS_clone as u32, 0, 4),
        filter_statement(BPF_LOAD_WORD_ABSOLUTE, SECCOMP_ARGUMENT_ZERO_OFFSET),
        filter_statement(BPF_AND, libc::CLONE_THREAD as u32),
        filter_jump(libc::CLONE_THREAD as u32, 1, 0),
        filter_statement(BPF_RETURN, denied),
        filter_statement(BPF_RETURN, SECCOMP_RET_ALLOW),
    ]);
    let program = libc::sock_fprog {
        len: filter
            .len()
            .try_into()
            .context("seccomp program was too large")?,
        filter: filter.as_mut_ptr(),
    };
    if unsafe {
        libc::syscall(
            libc::SYS_seccomp,
            SECCOMP_MODE_FILTER_OPERATION,
            SECCOMP_FILTER_FLAG_TSYNC,
            &program,
        )
    } != 0
    {
        return Err(io::Error::last_os_error()).context("could not install the seccomp filter");
    }
    Ok(())
}

fn denied_syscalls() -> Vec<libc::c_long> {
    let syscalls = vec![
        libc::SYS_socket,
        libc::SYS_socketpair,
        libc::SYS_connect,
        libc::SYS_accept,
        libc::SYS_accept4,
        libc::SYS_bind,
        libc::SYS_listen,
        libc::SYS_sendto,
        libc::SYS_recvfrom,
        libc::SYS_sendmsg,
        libc::SYS_recvmsg,
        libc::SYS_execve,
        libc::SYS_execveat,
        libc::SYS_unshare,
        libc::SYS_setns,
        libc::SYS_io_uring_setup,
        libc::SYS_io_uring_enter,
        libc::SYS_io_uring_register,
        libc::SYS_ptrace,
        libc::SYS_process_vm_readv,
        libc::SYS_process_vm_writev,
    ];
    #[cfg(target_arch = "x86_64")]
    let syscalls = {
        let mut extended = syscalls;
        extended.extend([libc::SYS_fork, libc::SYS_vfork]);
        extended
    };
    syscalls
}

fn syscall_denied(number: libc::c_long, arguments: &[libc::c_long]) -> bool {
    let result = unsafe {
        match arguments {
            [one, two, three] => libc::syscall(number, *one, *two, *three),
            _ => return false,
        }
    };
    result == -1 && io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

fn clone_process_denied() -> bool {
    let result = unsafe {
        libc::syscall(
            libc::SYS_clone,
            libc::SIGCHLD,
            std::ptr::null_mut::<libc::c_void>(),
            0,
            0,
            0,
        )
    };
    if result == 0 {
        unsafe { libc::_exit(1) };
    }
    if result > 0 {
        unsafe { libc::waitpid(result as libc::pid_t, std::ptr::null_mut(), 0) };
        return false;
    }
    io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

fn clone3_denied() -> bool {
    let result = unsafe { libc::syscall(libc::SYS_clone3, std::ptr::null::<libc::c_void>(), 0) };
    result == -1 && io::Error::last_os_error().raw_os_error() == Some(libc::ENOSYS)
}

fn exec_process_denied() -> bool {
    let executable = CString::new("/bottie-proof-missing").expect("fixed executable has no NUL");
    let arguments = [executable.as_ptr(), std::ptr::null()];
    let environment = [std::ptr::null::<libc::c_char>()];
    let result = unsafe {
        libc::execve(
            executable.as_ptr(),
            arguments.as_ptr(),
            environment.as_ptr(),
        )
    };
    result == -1 && io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}
