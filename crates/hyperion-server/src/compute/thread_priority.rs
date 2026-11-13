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
//!
//! On macOS a twin also follows each job's class ([`TwinLevel`], R06.T11.o): a job whose token is
//! lowered to the background class (a sky's census after its first reply, `--sky-qos background`,
//! the default there) runs at [`BACKGROUND`], `QOS_CLASS_BACKGROUND`, which gdt-cpus's table gives
//! as efficiency cores only on Apple silicon, and the next job at [`LOWERED`] again. A thread sets
//! its own class up as well as down there (`pthread_set_qos_class_self_np`, libpthread's `qos.h`),
//! where Linux never lets an unprivileged thread lower its nice again, so Linux, and Windows, keep
//! their twins' one level.
//!
//! A test's pool may keep its twins at their workers' priority instead ([`TwinPriority`]).

use gdt_cpus::{AppliedPriority, ThreadPriority, set_thread_priority};

/// The OS priority a pool's twins run its bulk and background jobs at: lowered, the server's, or
/// their workers' own, a test's.
///
/// A test runs beside other tests at the default priority, all in nextest's one session, so a
/// lowered twin's job takes about a tenth of a core it shares with one of their threads: Linux
/// weighs a thread at nice 10 as 110 against 1,024 at nice 0 (`sched_prio_to_weight`,
/// `kernel/sched/core.c`). Beside 32 busy threads at nice 0 on a pod's 32 CPUs, five of six server
/// tests that wait on bulk work ran past their patience with lowered twins, and all six passed
/// with their workers' (pod, 2026-11-06). It is scheduling only: no job's value, and not the order
/// the workers take jobs in, changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum TwinPriority {
    /// Lowered from the twin's start, the server's.
    ///
    /// Nice 10 on Linux, the class utility on macOS, where it also follows each job's class, and
    /// below normal on Windows.
    #[default]
    Lowered,
    /// The priority the twin started with, its worker's, for every job: a test's, whose bulk work
    /// would otherwise wait on the neighbouring tests' threads under load.
    Workers,
}

/// The priority a worker's twin runs at: gdt-cpus's `Lowest`, which is macOS's `QOS_CLASS_UTILITY`
/// and Linux's nice 10 (its table calls it "E-core-leaning" on Apple silicon); on Windows its
/// `BelowNormal`, `THREAD_PRIORITY_BELOW_NORMAL`, where `Lowest` would be `THREAD_PRIORITY_LOWEST`.
pub(crate) const LOWERED: ThreadPriority = if cfg!(windows) {
    ThreadPriority::BelowNormal
} else {
    ThreadPriority::Lowest
};

/// The priority a macOS twin runs a job at whose token is lowered to the background class
/// ([`CancelToken::lower_to_background`](super::CancelToken::lower_to_background)): gdt-cpus's
/// `Background`, macOS's `QOS_CLASS_BACKGROUND` (gdt-cpus 0.2606.1, `ThreadPriority`'s table:
/// `BACKGROUND`, "E-cores").
pub(crate) const BACKGROUND: ThreadPriority = ThreadPriority::Background;

/// Whether a twin follows each job's class, on macOS alone (the module docs).
pub(crate) const FOLLOWS_EACH_JOB: bool = cfg!(target_os = "macos");

/// A twin's priority job by job: [`LOWERED`], as [`lower_this_thread`] left it, or [`BACKGROUND`]
/// for a job whose token is lowered, set only when it changes and only where
/// [`FOLLOWS_EACH_JOB`] for a lowered twin. A refusal is logged at `warn`, and the twin then keeps
/// the level it has. A twin at its worker's priority ([`TwinPriority::Workers`]) never follows, so
/// its `current` is never read.
#[derive(Debug)]
pub(crate) struct TwinLevel {
    current: ThreadPriority,
    /// What the OS gave the last change, if any.
    applied: Option<AppliedPriority>,
    /// Whether it still follows the jobs: false off macOS, or once the OS has refused a change.
    follows: bool,
}

impl TwinLevel {
    /// A twin's level from its start at `twins`: after [`lower_this_thread`] where they are
    /// [`TwinPriority::Lowered`]; its worker's, never changed, where they are the workers'.
    #[must_use]
    pub(crate) fn new(twins: TwinPriority) -> Self {
        Self {
            current: LOWERED,
            applied: None,
            follows: match twins {
                TwinPriority::Lowered => FOLLOWS_EACH_JOB,
                TwinPriority::Workers => false,
            },
        }
    }

    /// Sets the calling twin to the level of a job whose token is `lowered`, or not, if it differs
    /// from the twin's.
    pub(crate) fn follow(&mut self, lowered: bool) {
        if !self.follows {
            return;
        }
        let wanted = if lowered { BACKGROUND } else { LOWERED };
        if wanted == self.current {
            return;
        }
        match set_thread_priority(wanted) {
            Ok(applied) => {
                tracing::debug!(
                    %applied,
                    degraded = applied.degraded(),
                    "a cpu worker's twin changed its priority for a job"
                );
                self.current = wanted;
                self.applied = Some(applied);
            }
            Err(error) => {
                let current = std::thread::current();
                tracing::warn!(
                    thread = current.name().unwrap_or("unnamed"),
                    %error,
                    "a cpu worker's twin could not change its priority for a job; it keeps its own"
                );
                self.follows = false;
            }
        }
    }
}

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

    /// A twin at its worker's priority follows no job's class, on any platform: it changes nothing
    /// for a lowered job or the next.
    #[test]
    fn a_twin_at_its_workers_priority_follows_no_jobs_class() {
        let applied = std::thread::spawn(|| {
            let mut level = TwinLevel::new(TwinPriority::Workers);
            level.follow(true);
            level.follow(false);
            level.applied
        })
        .join()
        .unwrap();
        assert!(applied.is_none(), "{applied:?}");
    }

    /// On Linux a twin keeps nice 10 whatever its jobs' class, and changes nothing.
    #[cfg(target_os = "linux")]
    #[test]
    fn on_linux_a_twin_keeps_its_level_whatever_its_jobs_class() {
        let started = nice_of_this_thread();
        let (nices, applied) = std::thread::spawn(|| {
            lower_this_thread();
            let mut level = TwinLevel::new(TwinPriority::Lowered);
            level.follow(true);
            let lowered = nice_of_this_thread();
            level.follow(false);
            ([lowered, nice_of_this_thread()], level.applied)
        })
        .join()
        .unwrap();
        assert_eq!(nices, [started.max(10); 2]);
        assert!(applied.is_none(), "{applied:?}");
    }

    /// On macOS a twin runs a lowered job's class at `QOS_CLASS_BACKGROUND` and the next job's at
    /// `QOS_CLASS_UTILITY` again, changing only when the class does.
    #[cfg(target_os = "macos")]
    #[test]
    fn on_macos_a_twin_follows_each_jobs_class() {
        let given = std::thread::spawn(|| {
            lower_this_thread();
            let mut level = TwinLevel::new(TwinPriority::Lowered);
            level.follow(false);
            let unchanged = level.applied.take();
            level.follow(true);
            let background = level.applied.take();
            level.follow(false);
            (unchanged, background, level.applied.take())
        })
        .join()
        .unwrap();
        let effective = |applied: Option<AppliedPriority>| {
            applied.map(|applied| (applied.effective(), applied.degraded()))
        };
        assert_eq!(
            (effective(given.0), effective(given.1), effective(given.2)),
            (None, Some((BACKGROUND, false)), Some((LOWERED, false)))
        );
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
