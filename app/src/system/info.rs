use std::collections::VecDeque;
use std::ffi::OsStr;

use byte_unit::Byte;
use chrono::{DateTime, Local, Utc};
use sysinfo::ProcessesToUpdate;
use warpui::{AppContext, Entity, ModelContext, SingletonEntity};
/// The threshold at which we emit a memory usage warning.
const MEMORY_USAGE_WARNING_THRESHOLD: Option<Byte> = byte_unit::Byte::GIGABYTE.multiply(10);

/// The refresh interval for system information, in seconds.
const REFRESH_INTERVAL_S: usize = 5;
/// The refresh interval for system information.
const REFRESH_INTERVAL: std::time::Duration =
    std::time::Duration::from_secs(REFRESH_INTERVAL_S as u64);

/// The time window that a resource usage report covers, in seconds.
const REPORT_WINDOW_S: usize = 300;
/// The number of data points aggregated into a resource usage report.
const REPORT_SAMPLE_COUNT: usize = REPORT_WINDOW_S / REFRESH_INTERVAL_S;

// Make sure the refresh interval cleanly divides the report window into an
// integral number of samples.
static_assertions::const_assert_eq!(REPORT_WINDOW_S % REFRESH_INTERVAL_S, 0);

pub enum SystemInfoEvent {
    /// There is new system info available for consumers to query.
    Refreshed,
    /// The application is using a large quantity of memory.
    MemoryUsageHigh,
}

pub struct SystemInfo {
    /// A structure we can use to efficiently query system information.
    system: sysinfo::System,
    /// Whether or not we've already emitted an event due to high memory usage.
    has_emitted_memory_warning_event: bool,
    /// A circular buffer storing resource usage data.
    stats: StatsBuffer,
}

impl SystemInfo {
    /// Creates a new [`SystemInfo`] model and begins periodic fetching of
    /// system information.
    ///
    /// Currently only retrieves and exposes memory usage information for the
    /// current process.
    pub fn new(ctx: &mut ModelContext<Self>) -> Self {
        let mut me = Self {
            system: sysinfo::System::new(),
            has_emitted_memory_warning_event: false,
            stats: Default::default(),
        };

        // Initialize the underlying system info.  This is necessary in order
        // for our first read of CPU stats to be accurate, as they are computed
        // as a delta between the previous refresh and now.
        me.system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[Self::current_pid()]),
            false, /* refresh_dead_processes */
            Self::refresh_kind(),
        );

        // If we're doing automated heap usage tracking, set up periodic
        // refreshes of the memory usage data.
        Self::schedule_refresh(ctx);

        me
    }

    /// Returns the amount of memory being used by the current process, in
    /// bytes.
    pub fn used_memory(&self) -> Byte {
        self.system
            .process(Self::current_pid())
            .expect("current process should exist")
            .memory()
            .into()
    }

    /// Returns the full memory footprint of the current process, in bytes.
    ///
    /// Unlike [`used_memory`] (RSS), this includes memory that has been
    /// swapped out or compressed by the OS.  On macOS this matches the value
    /// shown by Activity Monitor.
    /// Returns the average CPU usage over the refresh interval.
    ///
    /// If one CPU core is utilized at 100%, this will return 1.  It may return
    /// a value >1 on multi-core machines.
    pub fn cpu_usage(&self) -> f32 {
        let total_usage = self
            .system
            .process(Self::current_pid())
            .expect("current process should exist")
            .cpu_usage();
        total_usage / 100.
    }

    fn schedule_refresh(ctx: &mut ModelContext<Self>) {
        ctx.spawn(
            async {
                warpui::r#async::Timer::after(REFRESH_INTERVAL).await;
            },
            |me, _, ctx| {
                me.refresh(ctx);
                Self::schedule_refresh(ctx);
            },
        );
    }

    fn refresh(&mut self, ctx: &mut ModelContext<Self>) {
        self.system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[Self::current_pid()]),
            false, /* refresh_dead_processes */
            Self::refresh_kind(),
        );
        ctx.emit(SystemInfoEvent::Refreshed);

        // Add resource usage information to our circular buffer.
        self.stats.push(Sample {
            cpu: self.cpu_usage(),
        });

        let rss = self.used_memory();
        let footprint = rss;
        self.check_for_excessive_memory_usage(rss, footprint, ctx);

        // Once we have a full buffer of statistics, consider sending a report
        // each time we store new resource usage data.
    }

    /// Checks for excessive memory usage.  This may send a telemetry event
    /// and trigger a Sentry heap profile dump if excessive usage is detected.
    ///
    /// The threshold check uses `memory_footprint` (which includes swapped
    /// and compressed pages) so we actually detect high memory situations.
    /// The Rudderstack telemetry event still reports `rss` so existing
    /// dashboards are unaffected.
    fn check_for_excessive_memory_usage(
        &mut self,
        _rss: Byte,
        memory_footprint: Byte,
        ctx: &mut ModelContext<Self>,
    ) {
        if self.has_emitted_memory_warning_event {
            return;
        }

        // Use footprint (not RSS) for the threshold so we catch memory
        // that has been swapped out or compressed by the OS.
        if memory_footprint
            < MEMORY_USAGE_WARNING_THRESHOLD.expect("Threshold should not overflow u64")
        {
            return;
        }

        // Collect a detailed memory breakdown for diagnostics.
        ctx.emit(SystemInfoEvent::MemoryUsageHigh);
        self.has_emitted_memory_warning_event = true;
    }

    /// Returns the pid of the current process.
    fn current_pid() -> sysinfo::Pid {
        sysinfo::get_current_pid().expect("Platform should support process IDs")
    }

    /// Returns the [`sysinfo::ProcessRefreshKind`] that should be used when
    /// retrieving information about the current process.
    fn refresh_kind() -> sysinfo::ProcessRefreshKind {
        sysinfo::ProcessRefreshKind::nothing()
            .with_memory()
            .with_cpu()
    }

    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn refresh_all_processes(&mut self) {
        self.system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true, /* remove_dead_processes */
            Self::refresh_kind(),
        );
    }

    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn processes_by_name<'a>(
        &'a self,
        name: &'a str,
    ) -> impl Iterator<Item = &'a sysinfo::Process> {
        self.system.processes_by_name(OsStr::new(name))
    }
}

impl Entity for SystemInfo {
    type Event = SystemInfoEvent;
}

impl SingletonEntity for SystemInfo {}

/// A single resource usage sample point.
struct Sample {
    /// The CPU usage since the last sample, represented as a value in the
    /// range [0, num_cpus].
    cpu: f32,
}

/// A simple fixed-size circular buffer for storing resource usage sample
/// points.
struct StatsBuffer {
    stats: VecDeque<Sample>,
}

impl StatsBuffer {
    /// Constructs a new [`StatsBuffer`].
    fn new() -> Self {
        Self {
            stats: VecDeque::with_capacity(REPORT_SAMPLE_COUNT),
        }
    }

    /// Returns whether or not the buffer is full of samples.
    ///
    /// If true, adding a sample will replace the oldest sample in the buffer.
    fn is_full(&self) -> bool {
        self.stats.len() == self.stats.capacity()
    }

    /// Pushes a new sample into the buffer.  If the buffer is at capacity,
    /// the oldest sample will be removed to make room for the new one.
    fn push(&mut self, sample: Sample) {
        if self.is_full() {
            self.stats.pop_front();
        }
        self.stats.push_back(sample);
    }

    /// Returns an iterator over all samples in the buffer.
    fn iter(&self) -> impl Iterator<Item = &Sample> {
        self.stats.iter()
    }
}

impl Default for StatsBuffer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "info_tests.rs"]
mod tests;
