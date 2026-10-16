//! Reads events from the macOS Calendar database (EventKit). Read-only.

use chrono::{DateTime, Utc};

#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    pub title: String,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub all_day: bool,
    pub calendar_id: String,
    pub calendar_title: String,
    pub color: [u8; 3],
    pub location: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CalendarInfo {
    pub id: String,
    pub title: String,
    pub color: [u8; 3],
}

#[derive(Debug, Clone, PartialEq)]
pub enum Access {
    Granted,
    /// We haven't asked yet (or the request is waiting for an answer).
    NotDetermined,
    Denied,
    /// Not possible here; the string explains why.
    Unavailable(String),
}

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use block2::RcBlock;
    use objc2::rc::Retained;
    use objc2::runtime::{Bool, NSObjectProtocol};
    use objc2::sel;
    use objc2_app_kit::{NSColor, NSColorSpace};
    use objc2_event_kit::{EKAuthorizationStatus, EKCalendar, EKEntityType, EKEventStore};
    use objc2_foundation::{NSArray, NSDate, NSError};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    pub struct EventSource {
        store: Option<Retained<EKEventStore>>,
        requested: bool,
        answered: Arc<AtomicBool>,
    }

    /// Asking macOS for calendar access crashes an app that has no usage description in
    /// its Info.plist, so only ask when running from the TimeStrip.app bundle.
    fn in_app_bundle() -> bool {
        std::env::current_exe()
            .map(|p| p.to_string_lossy().contains(".app/Contents/MacOS/"))
            .unwrap_or(false)
    }

    fn status() -> EKAuthorizationStatus {
        #[allow(unused_unsafe)]
        unsafe {
            EKEventStore::authorizationStatusForEntityType(EKEntityType::Event)
        }
    }

    fn to_utc(date: &NSDate) -> DateTime<Utc> {
        #[allow(unused_unsafe)]
        let secs = unsafe { date.timeIntervalSince1970() };
        DateTime::from_timestamp_millis((secs * 1000.0) as i64).unwrap_or_default()
    }

    fn to_nsdate(t: DateTime<Utc>) -> Retained<NSDate> {
        #[allow(unused_unsafe)]
        unsafe {
            NSDate::dateWithTimeIntervalSince1970(t.timestamp_millis() as f64 / 1000.0)
        }
    }

    fn calendar_color(cal: &EKCalendar) -> [u8; 3] {
        #[allow(unused_unsafe)]
        unsafe {
            let color: Retained<NSColor> = cal.color();
            match color.colorUsingColorSpace(&NSColorSpace::sRGBColorSpace()) {
                Some(c) => [c.redComponent(), c.greenComponent(), c.blueComponent()]
                    .map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8),
                None => [120, 120, 128],
            }
        }
    }

    impl EventSource {
        pub fn new() -> Self {
            EventSource { store: None, requested: false, answered: Arc::new(AtomicBool::new(false)) }
        }

        fn store(&mut self) -> &EKEventStore {
            #[allow(unused_unsafe)]
            self.store.get_or_insert_with(|| unsafe { EKEventStore::new() })
        }

        pub fn access(&mut self) -> Access {
            let s = status();
            if s == EKAuthorizationStatus::FullAccess {
                Access::Granted
            } else if s == EKAuthorizationStatus::NotDetermined {
                if !in_app_bundle() {
                    Access::Unavailable(
                        "Calendar events need the TimeStrip.app bundle (see README → Build it yourself).".into(),
                    )
                } else if self.requested && self.answered.load(Ordering::Relaxed) {
                    // Answered, but the status hasn't caught up yet; check again next frame.
                    Access::NotDetermined
                } else {
                    Access::NotDetermined
                }
            } else if s == EKAuthorizationStatus::Denied || s == EKAuthorizationStatus::Restricted {
                Access::Denied
            } else {
                // Write-only access can't read events.
                Access::Denied
            }
        }

        /// Shows the system permission prompt (once per run).
        pub fn request_access(&mut self) {
            if self.requested || !in_app_bundle() {
                return;
            }
            self.requested = true;
            let answered = self.answered.clone();
            let block = RcBlock::new(move |_granted: Bool, _error: *mut NSError| {
                answered.store(true, Ordering::Relaxed);
            });
            let store = self.store();
            #[allow(unused_unsafe, deprecated)]
            unsafe {
                if store.respondsToSelector(sel!(requestFullAccessToEventsWithCompletion:)) {
                    // macOS 14 and later
                    store.requestFullAccessToEventsWithCompletion(RcBlock::as_ptr(&block));
                } else {
                    store.requestAccessToEntityType_completion(EKEntityType::Event, RcBlock::as_ptr(&block));
                }
            }
        }

        /// Whether a permission answer has arrived since the last call (so the UI can refresh).
        pub fn take_answer(&mut self) -> bool {
            self.answered.swap(false, Ordering::Relaxed)
        }

        pub fn calendars(&mut self) -> Vec<CalendarInfo> {
            if status() != EKAuthorizationStatus::FullAccess {
                return Vec::new();
            }
            let store = self.store();
            #[allow(unused_unsafe)]
            let cals: Retained<NSArray<EKCalendar>> = unsafe { store.calendarsForEntityType(EKEntityType::Event) };
            let mut out: Vec<CalendarInfo> = cals
                .iter()
                .map(|cal| {
                    #[allow(unused_unsafe)]
                    let (id, title) = unsafe { (cal.calendarIdentifier().to_string(), cal.title().to_string()) };
                    CalendarInfo { id, title, color: calendar_color(&cal) }
                })
                .collect();
            out.sort_by_key(|c| c.title.to_lowercase());
            out
        }

        /// Events overlapping `[start, end)`, skipping hidden calendars, sorted by start time.
        pub fn events(&mut self, start: DateTime<Utc>, end: DateTime<Utc>, hidden: &[String]) -> Vec<Event> {
            if status() != EKAuthorizationStatus::FullAccess {
                return Vec::new();
            }
            let (s, e) = (to_nsdate(start), to_nsdate(end));
            let store = self.store();
            #[allow(unused_unsafe)]
            let found = unsafe {
                let predicate = store.predicateForEventsWithStartDate_endDate_calendars(&s, &e, None);
                store.eventsMatchingPredicate(&predicate)
            };
            let mut out = Vec::new();
            for ev in found.iter() {
                #[allow(unused_unsafe)]
                unsafe {
                    let Some(cal) = ev.calendar() else { continue };
                    let calendar_id = cal.calendarIdentifier().to_string();
                    if hidden.contains(&calendar_id) {
                        continue;
                    }
                    out.push(Event {
                        title: ev.title().to_string(),
                        start: to_utc(&ev.startDate()),
                        end: to_utc(&ev.endDate()),
                        all_day: ev.isAllDay(),
                        calendar_title: cal.title().to_string(),
                        color: calendar_color(&cal),
                        location: ev.location().map(|l| l.to_string()).filter(|l| !l.trim().is_empty()),
                        calendar_id,
                    });
                }
            }
            out.sort_by(|a, b| (!a.all_day, a.start).cmp(&(!b.all_day, b.start)));
            out
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::*;

    pub struct EventSource;

    impl EventSource {
        pub fn new() -> Self {
            EventSource
        }
        pub fn access(&mut self) -> Access {
            Access::Unavailable("Calendar events are only supported on macOS.".into())
        }
        pub fn request_access(&mut self) {}
        pub fn take_answer(&mut self) -> bool {
            false
        }
        pub fn calendars(&mut self) -> Vec<CalendarInfo> {
            Vec::new()
        }
        pub fn events(&mut self, _start: DateTime<Utc>, _end: DateTime<Utc>, _hidden: &[String]) -> Vec<Event> {
            Vec::new()
        }
    }
}

pub use imp::EventSource;

/// The part of an event that falls on `day` in time zone `tz` — used to bucket events
/// into calendar days for whichever region the calendar is showing.
pub fn on_day(ev: &Event, day: chrono::NaiveDate, tz: chrono_tz::Tz, local: chrono_tz::Tz) -> bool {
    use chrono::TimeZone;
    if ev.all_day {
        // All-day events are "floating": they belong to the same dates everywhere.
        let first = ev.start.with_timezone(&local).date_naive();
        let last = (ev.end - chrono::Duration::seconds(1)).with_timezone(&local).date_naive();
        return day >= first && day <= last.max(first);
    }
    let Some(start) = tz.from_local_datetime(&day.and_hms_opt(0, 0, 0).unwrap()).earliest() else {
        return false;
    };
    let next = day.succ_opt().unwrap_or(day);
    let Some(end) = tz.from_local_datetime(&next.and_hms_opt(0, 0, 0).unwrap()).earliest() else {
        return false;
    };
    let (start, end) = (start.with_timezone(&Utc), end.with_timezone(&Utc));
    ev.start < end && (ev.end > start || (ev.end == ev.start && ev.start >= start))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, TimeZone};

    fn ev(start: DateTime<Utc>, end: DateTime<Utc>, all_day: bool) -> Event {
        Event {
            title: "x".into(),
            start,
            end,
            all_day,
            calendar_id: "c".into(),
            calendar_title: "Work".into(),
            color: [0, 0, 0],
            location: None,
        }
    }

    #[test]
    fn timed_event_lands_on_each_regions_own_day() {
        // 5 PM–6 PM Thursday Oct 15 in Los Angeles = Friday morning in Tokyo.
        let la = chrono_tz::America::Los_Angeles;
        let tokyo = chrono_tz::Asia::Tokyo;
        let start = la.with_ymd_and_hms(2026, 10, 15, 17, 0, 0).unwrap().with_timezone(&Utc);
        let e = ev(start, start + chrono::Duration::hours(1), false);
        let thu = NaiveDate::from_ymd_opt(2026, 10, 15).unwrap();
        let fri = NaiveDate::from_ymd_opt(2026, 10, 16).unwrap();
        assert!(on_day(&e, thu, la, la));
        assert!(!on_day(&e, fri, la, la));
        assert!(on_day(&e, fri, tokyo, la));
        assert!(!on_day(&e, thu, tokyo, la));
    }

    #[test]
    fn all_day_event_keeps_its_date() {
        let la = chrono_tz::America::Los_Angeles;
        let start = la.with_ymd_and_hms(2026, 10, 15, 0, 0, 0).unwrap().with_timezone(&Utc);
        let e = ev(start, start + chrono::Duration::days(1), true);
        let thu = NaiveDate::from_ymd_opt(2026, 10, 15).unwrap();
        let fri = NaiveDate::from_ymd_opt(2026, 10, 16).unwrap();
        assert!(on_day(&e, thu, chrono_tz::Asia::Tokyo, la));
        assert!(!on_day(&e, fri, chrono_tz::Asia::Tokyo, la));
    }
}
