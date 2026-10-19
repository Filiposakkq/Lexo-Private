//! The clock. Core reads the time in a few places (the names of backups, the stamps of the event log, the age of a lock); they all ask a `Clock`, so a test can
//! fix the time. The Python Core calls `datetime.now()` and `time.time()` directly and the golden tool replaces those calls (docs/technical/GOLDEN.md, "data scenarios").
//!
//! The local time of the system comes from the operating system through two tiny declarations (`GetLocalTime`, `localtime_r`): the `chrono` features that would do
//! it pull in the Windows import libraries, which the GNU toolchain used for development cannot link without MinGW's `dlltool`.
use chrono::{DateTime, NaiveDateTime, Utc};
use std::cell::Cell;
use std::time::{SystemTime, UNIX_EPOCH};

pub trait Clock {
    /// `datetime.now()`: local time without a zone.
    fn now_local(&self) -> NaiveDateTime;
    /// `datetime.now(timezone.utc)`
    fn now_utc(&self) -> DateTime<Utc>;
    /// `time.time()`: seconds since 1970 (compared with the times of files).
    fn epoch(&self) -> f64;
}

pub struct SystemClock;

#[cfg(windows)]
fn local_offset_seconds(_utc_secs: i64) -> Option<i64> {
    #[repr(C)]
    struct SystemTimeParts {
        year: u16,
        month: u16,
        day_of_week: u16,
        day: u16,
        hour: u16,
        minute: u16,
        second: u16,
        milliseconds: u16,
    }
    extern "system" {
        fn GetLocalTime(out: *mut SystemTimeParts);
    }
    let mut t = SystemTimeParts { year: 0, month: 0, day_of_week: 0, day: 0, hour: 0, minute: 0, second: 0, milliseconds: 0 };
    // SAFETY: GetLocalTime only writes the structure it is given.
    unsafe { GetLocalTime(&mut t) };
    let local = chrono::NaiveDate::from_ymd_opt(t.year as i32, t.month as u32, t.day as u32)?.and_hms_opt(t.hour as u32, t.minute as u32, t.second as u32)?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs() as i64;
    // both were read within the same second (or one apart): round the difference to a whole quarter of an hour
    let diff = local.and_utc().timestamp() - now;
    Some(((diff as f64 / 900.0).round() as i64) * 900)
}

#[cfg(unix)]
fn local_offset_seconds(utc_secs: i64) -> Option<i64> {
    #[repr(C)]
    struct Tm {
        sec: i32,
        min: i32,
        hour: i32,
        mday: i32,
        mon: i32,
        year: i32,
        wday: i32,
        yday: i32,
        isdst: i32,
        gmtoff: std::os::raw::c_long,
        zone: *const std::os::raw::c_char,
    }
    extern "C" {
        fn localtime_r(t: *const i64, out: *mut Tm) -> *mut Tm;
    }
    let mut tm = Tm { sec: 0, min: 0, hour: 0, mday: 0, mon: 0, year: 0, wday: 0, yday: 0, isdst: 0, gmtoff: 0, zone: std::ptr::null() };
    // SAFETY: localtime_r writes only the structure it is given (the layout is that of glibc, musl and the BSDs).
    let done = unsafe { localtime_r(&utc_secs, &mut tm) };
    (!done.is_null()).then_some(tm.gmtoff as i64)
}

#[cfg(not(any(windows, unix)))]
fn local_offset_seconds(_utc_secs: i64) -> Option<i64> {
    None
}

impl Clock for SystemClock {
    fn now_utc(&self) -> DateTime<Utc> {
        let d = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
        DateTime::from_timestamp(d.as_secs() as i64, d.subsec_nanos()).unwrap_or_default()
    }
    fn now_local(&self) -> NaiveDateTime {
        let utc = self.now_utc();
        let offset = local_offset_seconds(utc.timestamp()).unwrap_or(0);
        utc.naive_utc() + chrono::Duration::seconds(offset)
    }
    fn epoch(&self) -> f64 {
        SystemTime::now().duration_since(UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64())
    }
}

/// Local and UTC time fixed; the epoch is the real one shifted by `advance` (files have real times), or fixed with `set_epoch`.
pub struct TestClock {
    local: Cell<NaiveDateTime>,
    pub utc: DateTime<Utc>,
    offset: Cell<f64>,
    fixed: Cell<Option<f64>>,
}

impl TestClock {
    pub fn new(local: NaiveDateTime, utc: DateTime<Utc>) -> TestClock {
        TestClock { local: Cell::new(local), utc, offset: Cell::new(0.0), fixed: Cell::new(None) }
    }
    pub fn advance(&self, seconds: f64) {
        self.offset.set(self.offset.get() + seconds);
    }
    pub fn set_local(&self, value: NaiveDateTime) {
        self.local.set(value);
    }
    pub fn set_epoch(&self, value: Option<f64>) {
        self.fixed.set(value);
    }
}

impl Clock for TestClock {
    fn now_local(&self) -> NaiveDateTime {
        self.local.get()
    }
    fn now_utc(&self) -> DateTime<Utc> {
        self.utc
    }
    fn epoch(&self) -> f64 {
        self.fixed.get().unwrap_or_else(|| SystemClock.epoch() + self.offset.get())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_system_clock_is_sane() {
        let c = SystemClock;
        let (utc, local) = (c.now_utc().naive_utc(), c.now_local());
        assert!((local - utc).num_hours().abs() <= 14, "{utc} {local}");
        assert!(utc.and_utc().timestamp() > 1_700_000_000);
        assert!((c.epoch() - utc.and_utc().timestamp() as f64).abs() < 5.0);
    }
}
