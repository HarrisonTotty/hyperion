//! The OS priority the CPU pool's bulk and background jobs run at (rendering plan R06, R06.T11.k).
//!
//! A sky's census is hours of CPU at full parallelism, and at the OS's default priority the pool's
//! workers took the cores the client draws with (the owner's Mac, 2026-11-01). So each worker's
//! twin, which runs the bulk and background jobs the worker takes ([`CpuPool`](super::CpuPool)),
//! lowers itself once when it starts, to [`LOWERED`]: macOS's quality-of-service class utility,
//! which also leans towards Apple silicon's efficiency cores, Linux's nice 10, and Windows's below
//! normal. It is scheduling only, so no job's value changes.
//!
//! The setting goes through gdt-cpus, since the workspace forbids `unsafe`: its
//! `set_thread_priority` sets `pthread_set_qos_class_self_np` on macOS, `setpriority` on the
//! thread's ID on Linux and `SetThreadPriority` on Windows (gdt-cpus 0.2606.1, `ThreadPriority`'s
//! table). A refusal only leaves the twin at the default priority, so it is logged and the pool
//! goes on.
//!
//! Under Linux's autogroup scheduling, a desktop's default, a thread's nice ranks it only against
//! the threads of its own session (sched(7), "The autogroup feature"): there the twins yield to the
//! server's interactive workers, and to a client only where it shares the server's session.

use gdt_cpus::{AppliedPriority, ThreadPriority, set_thread_priority};

/// The priority a worker's twin runs at: gdt-cpus's `Lowest`, which is macOS's `QOS_CLASS_UTILITY`
/// and Linux's nice 10 (its table calls it "E-core-leaning" on Apple silicon); on Windows its
/// `BelowNormal`, `THREAD_PRIORITY_BELOW_NORMAL`, where `Lowest` would be `THREAD_PRIORITY_LOWEST`.
pub(crate) const LOWERED: ThreadPriority = if cfg!(windows) {
    ThreadPriority::BelowNormal
} else {
    ThreadPriority::Lowest
};

/// Lowers the calling thread to [`LOWERED`] for good, and says what the OS gave it, or `None`
/// where it refused or has no such setting.
///
/// A refusal is logged at `warn` and a weaker grant at `debug`. On Linux an unprivileged thread
/// already at a nice above 10, because the server was started under `nice`, keeps it, since the
/// kernel refuses to lower a nice without `CAP_SYS_NICE`; a privileged one is set to 10.
pub(crate) fn lower_this_thread() -> Option<AppliedPriority> {
    let current = std::thread::current();
    let thread = current.name().unwrap_or("unnamed");
    match set_thread_priority(LOWERED) {
        Ok(applied) => {
            tracing::debug!(
                thread,
                %applied,
                degraded = applied.degraded(),
                "a cpu worker's twin lowered its priority"
            );
            Some(applied)
        }
        Err(error) => {
            tracing::warn!(
                thread,
                %error,
                "a cpu worker's twin could not lower its priority; its jobs run at the default"
            );
            None
        }
    }
}

/// The calling thread's nice, the 19th field of `/proc/thread-self/stat`, for the tests that check
/// where a job ran.
#[cfg(all(test, target_os = "linux"))]
pub(crate) fn nice_of_this_thread() -> i32 {
    let stat = std::fs::read_to_string("/proc/thread-self/stat").unwrap();
    // The fields after the command's closing parenthesis start at the 3rd, the state, so the nice
    // is the 17th of them.
    let rest = stat.rsplit_once(')').unwrap().1;
    rest.split_whitespace().nth(16).unwrap().parse().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A thread lowered on Linux sits at nice 10, or at the process's nice where that is higher.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_lowered_thread_runs_at_nice_10_on_linux() {
        let started = nice_of_this_thread();
        let (applied, nice) = std::thread::spawn(|| (lower_this_thread(), nice_of_this_thread()))
            .join()
            .unwrap();
        assert_eq!(nice, started.max(10), "{applied:?}");
        assert_eq!(
            nice_of_this_thread(),
            started,
            "the test's own thread is untouched"
        );
    }

    /// The level is the brief's on each platform: gdt-cpus's `BelowNormal` on Windows, and its
    /// `Lowest`, nice 10 and `QOS_CLASS_UTILITY`, elsewhere.
    #[test]
    fn the_lowered_level_is_below_normal_on_windows_and_the_lowest_elsewhere() {
        let expected = if cfg!(windows) {
            ThreadPriority::BelowNormal
        } else {
            ThreadPriority::Lowest
        };
        assert_eq!(LOWERED, expected);
    }

    /// A thread lowered on macOS or Windows is given [`LOWERED`] itself: `QOS_CLASS_UTILITY`, or
    /// below normal.
    #[cfg(any(target_os = "macos", windows))]
    #[test]
    fn a_lowered_thread_is_given_the_lowered_priority() {
        let applied = std::thread::spawn(lower_this_thread)
            .join()
            .unwrap()
            .expect("the OS lowers a thread's own priority");
        assert_eq!(
            (applied.effective(), applied.degraded()),
            (LOWERED, false),
            "{applied:?}"
        );
    }
}
