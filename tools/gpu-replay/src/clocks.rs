//! The GPU's clocks during a replay (R05.T14.k; decision-r05-trace-windows-2.md, addendum B,
//! ruling 2), read from the sources the client reads (`apps/hyperion/src/main/gpuClocks.ts`):
//! `nvidia-smi -q -x` on NVIDIA, and under Linux the replayed adapter's DRM card's sysfs files,
//! i915's `gt_act_freq_mhz` against `gt_RP0_freq_mhz` (or `gt_boost_freq_mhz` without it) and
//! amdgpu's `pp_dpm_sclk` and `pp_dpm_mclk`. Elsewhere there is no unprivileged reading, and the
//! clocks are null with the reason.
//!
//! A [`ClockSampler`] reads once before the first frame, once a second on its own thread while the
//! frames replay, and once after the last, so that T19 can compare a pass across the browser and
//! the replay with both runs' clocks stated. A reading it cannot take is missing with its reason;
//! it never fails the replay.

use std::fs;
use std::io::{self, Read as _};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// A clock reading in whole MHz, or a performance state's number, or why it is missing.
pub(crate) type Reading = Result<u32, String>;

/// Where a replay's clocks are read, as the results file names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum ClockSourceKind {
    /// `nvidia-smi -q -x`.
    NvidiaSmi,
    /// i915's sysfs frequency files.
    I915Sysfs,
    /// amdgpu's `pp_dpm_*` files.
    AmdgpuSysfs,
}

impl ClockSourceKind {
    /// The source's name in the results file (`GpuClockSource`).
    #[must_use]
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::NvidiaSmi => "nvidia-smi",
            Self::I915Sysfs => "i915-sysfs",
            Self::AmdgpuSysfs => "amdgpu-sysfs",
        }
    }
}

/// The GPU's clocks at one reading.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct ClockReadings {
    /// Where they were read.
    pub(crate) source: ClockSourceKind,
    /// The GPU's maximum graphics clock, MHz.
    pub(crate) max_graphics_mhz: Reading,
    /// The graphics clock, MHz.
    pub(crate) graphics_mhz: Reading,
    /// The memory clock, MHz.
    pub(crate) memory_mhz: Reading,
    /// The performance state's number (P0, the highest, is 0).
    pub(crate) performance_state: Reading,
}

/// One sample of the clocks: when, and the readings or why the replay's GPU has none.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct ClockSample {
    /// Milliseconds since the replay started.
    pub(crate) t_ms: u64,
    /// The readings, or why there is no clock source for the replay's GPU.
    pub(crate) readings: Result<ClockReadings, String>,
}

impl ClockSample {
    /// A sample at `t_ms` of `readings`.
    #[must_use]
    pub(crate) fn new(t_ms: u64, readings: Result<ClockReadings, String>) -> Self {
        Self { t_ms, readings }
    }
}

/// The PCI vendor IDs with a clock reader.
const NVIDIA: u32 = 0x10de;
const INTEL: u32 = 0x8086;
const AMD: u32 = 0x1002;

/// Why i915 has no memory clock.
const I915_MEMORY_REASON: &str = "i915 gives no memory clock: the GPU shares the system's memory";
/// Why i915 has no performance state.
const I915_STATE_REASON: &str = "i915 has no performance states";
/// Why amdgpu has no performance state.
const AMDGPU_STATE_REASON: &str = "amdgpu gives clock levels, not performance states";

/// How long `nvidia-smi` may take before its reading is given up, as the client's sampler gives it.
const NVIDIA_SMI_TIMEOUT: Duration = Duration::from_secs(5);

/// How often the sampler reads the clocks while the frames replay.
const SAMPLE_INTERVAL: Duration = Duration::from_secs(1);

/// The source of a replay's clocks, chosen once from its adapter.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum ClockSource {
    /// `nvidia-smi -q -x`.
    NvidiaSmi,
    /// i915's sysfs files under a DRM card's directory.
    I915Sysfs(PathBuf),
    /// amdgpu's sysfs files under a DRM card's directory.
    AmdgpuSysfs(PathBuf),
    /// None, and why.
    Unavailable(String),
}

impl ClockSource {
    /// The source for the adapter `vendor:device` on `os` (`std::env::consts::OS`), its DRM card
    /// searched under `sys_root` (`/sys`, or a fixture tree in a test).
    #[must_use]
    pub(crate) fn choose(os: &str, vendor: u32, device: u32, sys_root: &Path) -> Self {
        if os == "macos" {
            return Self::Unavailable(format!(
                "no unprivileged GPU clock reading on {os}; powermetrics needs root"
            ));
        }
        if vendor == NVIDIA {
            return Self::NvidiaSmi;
        }
        if os == "windows" {
            return Self::Unavailable(format!(
                "no unprivileged GPU clock reading on {os} for a non-NVIDIA GPU"
            ));
        }
        if os != "linux" {
            return Self::Unavailable(format!("no unprivileged GPU clock reading on {os}"));
        }
        if vendor != INTEL && vendor != AMD {
            return Self::Unavailable(format!(
                "no GPU clock reader for the vendor {vendor:#06x} on {os}"
            ));
        }
        match find_card(sys_root, vendor, device) {
            Ok(card) if vendor == INTEL => Self::I915Sysfs(card),
            Ok(card) => Self::AmdgpuSysfs(card),
            Err(reason) => Self::Unavailable(reason),
        }
    }

    /// The clocks now, or why the source has none.
    ///
    /// # Errors
    ///
    /// The reason, when there is no source for the replay's GPU.
    pub(crate) fn read(&self) -> Result<ClockReadings, String> {
        match self {
            Self::NvidiaSmi => Ok(match run_nvidia_smi() {
                Ok(xml) => nvidia_smi_clocks(&xml),
                Err(reason) => all_missing(ClockSourceKind::NvidiaSmi, &reason),
            }),
            Self::I915Sysfs(card) => Ok(i915_clocks(card)),
            Self::AmdgpuSysfs(card) => Ok(amdgpu_clocks(card)),
            Self::Unavailable(reason) => Err(reason.clone()),
        }
    }
}

/// Every reading of `source` missing for `reason`.
#[must_use]
fn all_missing(source: ClockSourceKind, reason: &str) -> ClockReadings {
    let none = || Err(reason.to_owned());
    ClockReadings {
        source,
        max_graphics_mhz: none(),
        graphics_mhz: none(),
        memory_mhz: none(),
        performance_state: none(),
    }
}

/// The text of the first `<tag>…</tag>` in `xml`, trimmed.
#[must_use]
fn tag_text<'a>(xml: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{tag}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&format!("</{tag}>"))? + start;
    Some(xml[start..end].trim())
}

/// `<n> MHz` as `n`.
#[must_use]
fn parse_mhz(text: &str) -> Option<u32> {
    text.strip_suffix("MHz")?.trim().parse().ok()
}

/// The clocks of the first GPU in a `nvidia-smi -q -x` report: `clocks/graphics_clock`,
/// `clocks/mem_clock`, `max_clocks/graphics_clock` and `performance_state`, each read within its
/// own block, since the other clock blocks use the same element names, and `N/A` as missing.
#[must_use]
pub(crate) fn nvidia_smi_clocks(xml: &str) -> ClockReadings {
    let Some(gpu) = tag_text_block(xml, "gpu") else {
        return all_missing(ClockSourceKind::NvidiaSmi, "nvidia-smi lists no GPU");
    };
    let clocks = tag_text(gpu, "clocks").unwrap_or_default();
    let max_clocks = tag_text(gpu, "max_clocks").unwrap_or_default();
    let mhz = |block: &str, element: &str, path: &str| {
        tag_text(block, element)
            .and_then(parse_mhz)
            .ok_or_else(|| format!("nvidia-smi gives no {path} (N/A)"))
    };
    ClockReadings {
        source: ClockSourceKind::NvidiaSmi,
        max_graphics_mhz: mhz(max_clocks, "graphics_clock", "max_clocks/graphics_clock"),
        graphics_mhz: mhz(clocks, "graphics_clock", "clocks/graphics_clock"),
        memory_mhz: mhz(clocks, "mem_clock", "clocks/mem_clock"),
        performance_state: tag_text(gpu, "performance_state")
            .and_then(|state| state.strip_prefix('P')?.parse().ok())
            .ok_or_else(|| "nvidia-smi gives no performance_state (N/A)".to_owned()),
    }
}

/// The body of the first `<gpu id="…">` element.
#[must_use]
fn tag_text_block<'a>(xml: &'a str, tag: &str) -> Option<&'a str> {
    let start = xml.find(&format!("<{tag} "))?;
    let body = start + xml[start..].find('>')? + 1;
    let end = xml[body..].find(&format!("</{tag}>"))? + body;
    Some(&xml[body..end])
}

/// Runs `nvidia-smi -q -x` for at most [`NVIDIA_SMI_TIMEOUT`].
fn run_nvidia_smi() -> Result<String, String> {
    run_bounded("nvidia-smi", &["-q", "-x"], NVIDIA_SMI_TIMEOUT)
}

/// Runs `program` with `args` and gives its output, or why there is none, within `timeout`.
///
/// A run still going at the deadline is killed and given up without waiting for it: a hung
/// `nvidia-smi` is usually stuck in the driver, where even SIGKILL waits for the call to return,
/// and a blocking wait would hold the replay with it. The replayer's exit reaps it.
fn run_bounded(program: &str, args: &[&str], timeout: Duration) -> Result<String, String> {
    let mut child = match Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(format!("no {program} on this machine"));
        }
        Err(error) => return Err(format!("{program} failed: {error}")),
    };
    let Some(mut stdout) = child.stdout.take() else {
        return Err(format!("{program} failed: its output was not piped"));
    };
    // Read on a thread of its own, so that the wait below can give up on a hung run; the read
    // ends when the run's pipe closes, as it does when it exits or is killed.
    let reader = thread::spawn(move || {
        let mut text = String::new();
        stdout.read_to_string(&mut text).map(|_| text)
    });
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            Ok(None) => {
                // A run that has exited meanwhile cannot be killed, and needs nothing more.
                let _ = child.kill();
                return Err(format!("{program} failed: no answer within {timeout:?}"));
            }
            Err(error) => return Err(format!("{program} failed: {error}")),
        }
    };
    let text = reader
        .join()
        .map_err(|_| format!("{program} failed: its output's reader panicked"))?
        .map_err(|error| format!("{program} failed: {error}"))?;
    if status.success() {
        Ok(text)
    } else {
        Err(format!("{program} failed: {status}"))
    }
}

/// A sysfs file's text, or why it could not be read, its path named.
fn read_sysfs(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| {
        let cause = match error.kind() {
            io::ErrorKind::NotFound => "ENOENT".to_owned(),
            io::ErrorKind::PermissionDenied => "EACCES".to_owned(),
            _ => error.to_string(),
        };
        format!("{} could not be read ({cause})", path.display())
    })
}

/// A sysfs file holding one whole number of MHz, as i915's frequency files do.
fn read_mhz(path: &Path) -> Reading {
    let text = read_sysfs(path)?;
    let trimmed = text.trim();
    trimmed
        .parse()
        .map_err(|_| format!("{} holds no whole MHz: \"{trimmed}\"", path.display()))
}

/// An i915 card's clocks: the actual graphics clock against RP0, or boost without RP0's file.
fn i915_clocks(card: &Path) -> ClockReadings {
    let max_graphics_mhz = read_mhz(&card.join("gt_RP0_freq_mhz")).or_else(|rp0| {
        read_mhz(&card.join("gt_boost_freq_mhz")).map_err(|boost| format!("{rp0}; {boost}"))
    });
    ClockReadings {
        source: ClockSourceKind::I915Sysfs,
        max_graphics_mhz,
        graphics_mhz: read_mhz(&card.join("gt_act_freq_mhz")),
        memory_mhz: Err(I915_MEMORY_REASON.to_owned()),
        performance_state: Err(I915_STATE_REASON.to_owned()),
    }
}

/// One of amdgpu's `pp_dpm_*` lists: each level's clock in the file's order, and the level
/// marked `*`, if any.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub(crate) struct DpmLevels {
    /// Each level's clock, MHz, lowest first.
    pub(crate) levels_mhz: Vec<u32>,
    /// The current level's clock, MHz.
    pub(crate) current_mhz: Option<u32>,
}

/// Parses an amdgpu `pp_dpm_sclk` or `pp_dpm_mclk`: one level a line, `<index>: <clock>Mhz`, the
/// current one followed by `*` (an APU's sleep level is indexed `S`).
#[must_use]
pub(crate) fn parse_dpm_levels(text: &str) -> DpmLevels {
    let mut levels_mhz = Vec::new();
    let mut current_mhz = None;
    for line in text.lines() {
        let Some((index, rest)) = line.split_once(':') else {
            continue;
        };
        if index.trim().is_empty() || !index.trim().chars().all(char::is_alphanumeric) {
            continue;
        }
        let rest = rest.trim();
        let (clock, current) = match rest.strip_suffix('*') {
            Some(clock) => (clock.trim_end(), true),
            None => (rest, false),
        };
        let lower = clock.to_ascii_lowercase();
        let Some(mhz) = lower
            .strip_suffix("mhz")
            .and_then(|digits| digits.trim().parse::<u32>().ok())
        else {
            continue;
        };
        levels_mhz.push(mhz);
        if current {
            current_mhz = Some(mhz);
        }
    }
    DpmLevels {
        levels_mhz,
        current_mhz,
    }
}

/// A `pp_dpm_*` file's levels, or why it has none.
fn read_dpm(path: &Path) -> Result<DpmLevels, String> {
    let levels = parse_dpm_levels(&read_sysfs(path)?);
    if levels.levels_mhz.is_empty() {
        Err(format!("{} lists no clock level", path.display()))
    } else {
        Ok(levels)
    }
}

/// The current level of `levels`, read from `path`.
fn current_of(levels: &Result<DpmLevels, String>, path: &Path) -> Reading {
    match levels {
        Ok(levels) => levels
            .current_mhz
            .ok_or_else(|| format!("no level of {} is marked current", path.display())),
        Err(reason) => Err(reason.clone()),
    }
}

/// An amdgpu card's clocks: the current graphics and memory levels, against the highest.
fn amdgpu_clocks(card: &Path) -> ClockReadings {
    let sclk_path = card.join("device/pp_dpm_sclk");
    let mclk_path = card.join("device/pp_dpm_mclk");
    let sclk = read_dpm(&sclk_path);
    let mclk = read_dpm(&mclk_path);
    ClockReadings {
        source: ClockSourceKind::AmdgpuSysfs,
        max_graphics_mhz: sclk.as_ref().map_err(Clone::clone).and_then(|levels| {
            levels
                .levels_mhz
                .iter()
                .max()
                .copied()
                .ok_or_else(|| "no level".to_owned())
        }),
        graphics_mhz: current_of(&sclk, &sclk_path),
        memory_mhz: current_of(&mclk, &mclk_path),
        performance_state: Err(AMDGPU_STATE_REASON.to_owned()),
    }
}

/// A sysfs file holding a hex ID (`0x8086`).
fn read_hex_id(path: &Path) -> Option<u32> {
    // A card whose IDs cannot be read cannot be shown to be the replay's GPU, so it is passed by.
    let text = fs::read_to_string(path).ok()?;
    u32::from_str_radix(text.trim().strip_prefix("0x")?, 16).ok()
}

/// The DRM card of the adapter `vendor:device`: the lowest-numbered `card<N>` under
/// `<sys_root>/class/drm` whose `device/vendor` and `device/device` are its.
fn find_card(sys_root: &Path, vendor: u32, device: u32) -> Result<PathBuf, String> {
    let drm = sys_root.join("class/drm");
    let entries = fs::read_dir(&drm)
        .map_err(|error| format!("{} could not be read ({error})", drm.display()))?;
    let mut cards: Vec<(u32, PathBuf)> = entries
        // An entry that cannot be read is no card to choose.
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let n = name.strip_prefix("card")?.parse::<u32>().ok()?;
            Some((n, entry.path()))
        })
        .collect();
    cards.sort();
    cards
        .into_iter()
        .map(|(_, dir)| dir)
        .find(|dir| {
            read_hex_id(&dir.join("device/vendor")) == Some(vendor)
                && read_hex_id(&dir.join("device/device")) == Some(device)
        })
        .ok_or_else(|| {
            format!(
                "no DRM card under {} is the replay's GPU ({vendor:#06x}:{device:#06x})",
                drm.display()
            )
        })
}

/// Milliseconds from `started` to now.
fn ms_since(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// Reads the clocks once before the frames, once a second while they replay, and once after.
#[derive(Debug)]
pub(crate) struct ClockSampler {
    source: ClockSource,
    started: Instant,
    first: ClockSample,
    stop: mpsc::Sender<()>,
    thread: JoinHandle<Vec<ClockSample>>,
}

impl ClockSampler {
    /// Takes the first reading now and starts reading once a second, the times counted from
    /// `started`, the replay's start.
    #[must_use]
    pub(crate) fn start(source: ClockSource, started: Instant) -> Self {
        Self::start_every(source, started, SAMPLE_INTERVAL)
    }

    /// [`ClockSampler::start`], reading every `interval` between the first and the last.
    #[must_use]
    pub(crate) fn start_every(source: ClockSource, started: Instant, interval: Duration) -> Self {
        let first = ClockSample::new(ms_since(started), source.read());
        let (stop, stopped) = mpsc::channel::<()>();
        let reader = source.clone();
        let thread = thread::spawn(move || {
            let mut samples = Vec::new();
            loop {
                match stopped.recv_timeout(interval) {
                    Err(RecvTimeoutError::Timeout) => {
                        samples.push(ClockSample::new(ms_since(started), reader.read()));
                    }
                    // Stopped, or its owner gone: the samples so far are the thread's result.
                    Ok(()) | Err(RecvTimeoutError::Disconnected) => return samples,
                }
            }
        });
        Self {
            source,
            started,
            first,
            stop,
            thread,
        }
    }

    /// Stops the sampling and takes the last reading: every sample, in time order.
    #[must_use]
    pub(crate) fn finish(self) -> Vec<ClockSample> {
        // A thread already gone has nothing more to be told.
        let _ = self.stop.send(());
        // A sampling thread that panicked leaves only the first and last readings.
        let between = self.thread.join().unwrap_or_default();
        let mut samples = Vec::with_capacity(between.len() + 2);
        samples.push(self.first);
        samples.extend(between);
        samples.push(ClockSample::new(ms_since(self.started), self.source.read()));
        samples
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The client's fixtures, which these sources are read from as the client reads them.
    fn fixtures() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/hyperion/src/main/fixtures")
    }

    #[test]
    fn nvidia_smi_gives_the_recorded_clocks_and_state() {
        let xml = fs::read_to_string(fixtures().join("nvidia-smi.xml")).expect("the fixture");
        assert_eq!(
            nvidia_smi_clocks(&xml),
            ClockReadings {
                source: ClockSourceKind::NvidiaSmi,
                max_graphics_mhz: Ok(2115),
                graphics_mhz: Ok(210),
                memory_mhz: Ok(405),
                performance_state: Ok(8),
            }
        );
        let na = xml.replace(
            "<graphics_clock>210 MHz</graphics_clock>",
            "<graphics_clock>N/A</graphics_clock>",
        );
        assert_eq!(
            nvidia_smi_clocks(&na).graphics_mhz,
            Err("nvidia-smi gives no clocks/graphics_clock (N/A)".to_owned())
        );
        assert_eq!(
            nvidia_smi_clocks("<nvidia_smi_log></nvidia_smi_log>").graphics_mhz,
            Err("nvidia-smi lists no GPU".to_owned())
        );
    }

    #[test]
    fn i915_reads_the_actual_clock_against_rp0() {
        let root = fixtures().join("sysfs/i915");
        let source = ClockSource::choose("linux", INTEL, 0x5917, &root);
        let card = root.join("class/drm/card0");
        assert_eq!(source, ClockSource::I915Sysfs(card));
        assert_eq!(
            source.read(),
            Ok(ClockReadings {
                source: ClockSourceKind::I915Sysfs,
                max_graphics_mhz: Ok(1150),
                graphics_mhz: Ok(600),
                memory_mhz: Err(I915_MEMORY_REASON.to_owned()),
                performance_state: Err(I915_STATE_REASON.to_owned()),
            })
        );
    }

    #[test]
    fn i915_falls_back_to_boost_and_names_a_missing_file() {
        // A card with neither RP0's file nor the actual clock's, its boost at 1,100 MHz.
        let card = fixtures().join("sysfs/i915-no-rp0/class/drm/card0");
        let readings = i915_clocks(&card);
        assert_eq!(readings.max_graphics_mhz, Ok(1100));
        assert_eq!(
            readings.graphics_mhz,
            Err(format!(
                "{} could not be read (ENOENT)",
                card.join("gt_act_freq_mhz").display()
            ))
        );
    }

    #[test]
    fn amdgpu_reads_the_marked_levels_of_its_own_card() {
        let root = fixtures().join("sysfs/amdgpu");
        let source = ClockSource::choose("linux", AMD, 0x73bf, &root);
        assert_eq!(
            source,
            ClockSource::AmdgpuSysfs(root.join("class/drm/card1"))
        );
        assert_eq!(
            source.read(),
            Ok(ClockReadings {
                source: ClockSourceKind::AmdgpuSysfs,
                max_graphics_mhz: Ok(2250),
                graphics_mhz: Ok(2045),
                memory_mhz: Ok(1000),
                performance_state: Err(AMDGPU_STATE_REASON.to_owned()),
            })
        );
    }

    #[test]
    fn dpm_levels_parse_as_the_kernel_writes_them() {
        assert_eq!(
            parse_dpm_levels("0: 500Mhz \n1: 2045Mhz *\n2: 2250Mhz \n"),
            DpmLevels {
                levels_mhz: vec![500, 2045, 2250],
                current_mhz: Some(2045),
            }
        );
        assert_eq!(
            parse_dpm_levels("S: 19Mhz *\n0: 400Mhz \n"),
            DpmLevels {
                levels_mhz: vec![19, 400],
                current_mhz: Some(19),
            }
        );
    }

    #[test]
    fn other_platforms_and_gpus_have_no_reading_with_the_reason() {
        let root = fixtures().join("sysfs/i915");
        assert_eq!(
            ClockSource::choose("macos", INTEL, 0x5917, &root).read(),
            Err("no unprivileged GPU clock reading on macos; powermetrics needs root".to_owned())
        );
        assert_eq!(
            ClockSource::choose("windows", AMD, 0x73bf, &root).read(),
            Err("no unprivileged GPU clock reading on windows for a non-NVIDIA GPU".to_owned())
        );
        assert_eq!(
            ClockSource::choose("windows", NVIDIA, 0x2206, &root),
            ClockSource::NvidiaSmi
        );
        assert_eq!(
            ClockSource::choose("linux", 0x1a03, 0x2000, &root).read(),
            Err("no GPU clock reader for the vendor 0x1a03 on linux".to_owned())
        );
        assert_eq!(
            ClockSource::choose("linux", AMD, 0x73bf, &root).read(),
            Err(format!(
                "no DRM card under {} is the replay's GPU (0x1002:0x73bf)",
                root.join("class/drm").display()
            ))
        );
    }

    /// No test runs for an hour, so a sampler at this interval reads only first and last.
    const NEVER: Duration = Duration::from_secs(3600);

    #[test]
    fn the_sampler_reads_before_the_frames_and_after_them() {
        let source = ClockSource::Unavailable("none here".to_owned());
        let samples = ClockSampler::start_every(source, Instant::now(), NEVER).finish();
        let readings: Vec<_> = samples.iter().map(|s| s.readings.clone()).collect();
        assert_eq!(readings, vec![Err("none here".to_owned()); 2]);
        assert!(samples[0].t_ms <= samples[1].t_ms, "{samples:?}");
    }

    #[test]
    fn the_samples_between_keep_time_order() {
        let source = ClockSource::Unavailable("none here".to_owned());
        let samples = ClockSampler::start_every(source, Instant::now(), Duration::ZERO).finish();
        assert!(samples.len() >= 2, "{}", samples.len());
        assert!(
            samples.windows(2).all(|pair| pair[0].t_ms <= pair[1].t_ms),
            "{samples:?}"
        );
    }

    #[test]
    fn a_missing_program_is_named() {
        assert_eq!(
            run_bounded("gpu-replay-no-such-program", &[], Duration::from_secs(5)),
            Err("no gpu-replay-no-such-program on this machine".to_owned())
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_run_past_its_bound_is_given_up() {
        assert_eq!(
            run_bounded("sleep", &["30"], Duration::from_millis(20)),
            Err("sleep failed: no answer within 20ms".to_owned())
        );
    }
}
