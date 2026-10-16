//! Events only wake a single foreground reconciliation owner. Paths and event
//! sequences never become analysis inputs; the canonical scanner remains authority.
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
    time::Duration,
};
const DIRTY: u8 = 1;
const RESYNC: u8 = 2;
const ERROR: u8 = 4;
#[derive(Clone)]
struct Hints {
    sender: SyncSender<()>,
    flags: Arc<AtomicU8>,
}
impl Hints {
    fn signal(&self, error: bool) {
        self.flags.fetch_or(
            DIRTY | if error { RESYNC | ERROR } else { 0 },
            Ordering::SeqCst,
        );
        if matches!(self.sender.try_send(()), Err(TrySendError::Full(()))) {
            self.flags.fetch_or(RESYNC, Ordering::SeqCst);
        }
    }
    fn event(&self, event: notify::Result<Event>) {
        // Analysis reads can produce access notifications. They do not change
        // analysis inputs and must not trigger a self-sustaining reconciliation.
        match event {
            Ok(event) if matches!(event.kind, EventKind::Access(_)) && !event.need_rescan() => (),
            Ok(event) => {
                if event.need_rescan() {
                    self.flags.fetch_or(RESYNC, Ordering::SeqCst);
                }
                self.signal(false);
            }
            Err(_) => self.signal(true),
        }
    }
}
#[derive(Debug)]
pub struct Activity {
    pub resync: bool,
    pub watcher_error: bool,
}
pub struct Watch {
    _watcher: RecommendedWatcher,
    receiver: Receiver<()>,
    hints: Hints,
}
fn channel() -> (Hints, Receiver<()>) {
    let (sender, receiver) = mpsc::sync_channel(1);
    (
        Hints {
            sender,
            flags: Arc::new(AtomicU8::new(0)),
        },
        receiver,
    )
}
fn take_activity(hints: &Hints, receiver: &Receiver<()>) -> Option<Activity> {
    // Drain before swapping flags. A racing callback either contributes flags to
    // this batch or leaves a flag/signal for the next batch.
    while receiver.try_recv().is_ok() {}
    let flags = hints.flags.swap(0, Ordering::SeqCst);
    (flags & DIRTY != 0).then_some(Activity {
        resync: flags & RESYNC != 0,
        watcher_error: flags & ERROR != 0,
    })
}
impl Watch {
    pub fn new(root: &Path) -> notify::Result<Self> {
        let (hints, receiver) = channel();
        let callback = hints.clone();
        let mut watcher = notify::recommended_watcher(move |event| callback.event(event))?;
        watcher.watch(root, RecursiveMode::Recursive)?;
        Ok(Self {
            _watcher: watcher,
            receiver,
            hints,
        })
    }
    pub fn next(&self) -> Option<Activity> {
        // Bounded coalescing window: continuous event traffic cannot postpone work
        // forever. The timeout also observes an overflow/error flag without a send.
        let _ = self.receiver.recv_timeout(Duration::from_secs(1));
        if self.hints.flags.load(Ordering::SeqCst) & DIRTY == 0 {
            return None;
        }
        std::thread::sleep(Duration::from_millis(100));
        take_activity(&self.hints, &self.receiver)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn callback_is_bounded_coalesces_and_preserves_overflow_and_errors() {
        let (hints, receiver) = channel();
        hints.event(Ok(
            Event::new(EventKind::Any).add_path("untrusted/outside.py".into())
        ));
        let first = take_activity(&hints, &receiver).unwrap();
        assert!(!first.resync && !first.watcher_error);
        assert!(take_activity(&hints, &receiver).is_none());
        for _ in 0..10_000 {
            hints.signal(false);
        }
        let overflow = take_activity(&hints, &receiver).unwrap();
        assert!(overflow.resync && !overflow.watcher_error);
        hints.event(Err(notify::Error::generic("backend overflow")));
        let error = take_activity(&hints, &receiver).unwrap();
        assert!(error.resync && error.watcher_error);
        hints.event(Ok(Event::new(EventKind::Access(
            notify::event::AccessKind::Any,
        ))));
        assert!(take_activity(&hints, &receiver).is_none());
    }
    #[test]
    fn events_during_owned_work_remain_pending() {
        let (hints, receiver) = channel();
        hints.signal(false);
        assert!(take_activity(&hints, &receiver).is_some());
        // The callback knows neither a repository root nor a cache/parser handle.
        hints.signal(false);
        assert!(take_activity(&hints, &receiver).is_some());
    }

    #[test]
    fn ambiguous_event_paths_trigger_scan_of_actual_final_state() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("old.py"), "def obsolete(): pass\n").unwrap();
        let (hints, receiver) = channel();
        hints.event(Ok(
            Event::new(EventKind::Any).add_path("outside/not-a-source.py".into())
        ));
        std::fs::rename(root.path().join("old.py"), root.path().join("new.py")).unwrap();
        std::fs::write(root.path().join("new.py"), "def final_state(): pass\n").unwrap();
        hints.signal(true);
        let activity = take_activity(&hints, &receiver).unwrap();
        assert!(activity.resync);
        let scan = crate::scan::scan_repository(&crate::model::ScanOptions {
            root: root.path().into(),
            count_lines: false,
            max_line_count_file_bytes: 0,
            probe_git: false,
        })
        .unwrap();
        let (index, _) = crate::index::analyze_scan(scan, 1024, true).unwrap();
        assert_eq!(index.files.len(), 1);
        assert_eq!(index.files[0].relative_path, Path::new("new.py"));
        assert_eq!(index.symbols[0].name, "final_state");
    }
}
