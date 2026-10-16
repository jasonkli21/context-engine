//! Events only wake a single foreground reconciliation owner. Paths and event
//! sequences never become analysis inputs; the canonical scanner remains authority.
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU8, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
    time::{Duration, Instant},
};
const DIRTY: u8 = 1;
const RESYNC: u8 = 2;
const ERROR: u8 = 4;
const RESCAN_INTERVAL: Duration = Duration::from_secs(60);
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
    fn event(
        &self,
        event: notify::Result<Event>,
        root: &Path,
        external_ignores: &BTreeSet<PathBuf>,
    ) {
        // Analysis reads can produce access notifications. They do not change
        // analysis inputs and must not trigger a self-sustaining reconciliation.
        match event {
            Ok(event) if matches!(event.kind, EventKind::Access(_)) && !event.need_rescan() => (),
            Ok(event) => {
                if event.need_rescan() {
                    self.flags.fetch_or(RESYNC, Ordering::SeqCst);
                    self.signal(false);
                    return;
                }
                if !event.affects_watched_input(root, external_ignores) {
                    return;
                }
                self.signal(false);
            }
            Err(_) => self.signal(true),
        }
    }
}
trait WatchedInputEvent {
    fn affects_watched_input(&self, root: &Path, controls: &BTreeSet<PathBuf>) -> bool;
}
impl WatchedInputEvent for Event {
    fn affects_watched_input(&self, root: &Path, controls: &BTreeSet<PathBuf>) -> bool {
        self.paths.is_empty()
            || self.paths.iter().any(|path| {
                !path.is_absolute() || path.starts_with(root) || controls.contains(path)
            })
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
    last_rescan: Mutex<Instant>,
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
        let callback_root = root.to_path_buf();
        let mut external_ignores = BTreeSet::new();
        let mut ancestors = Vec::new();
        let mut parent = root.parent();
        while let Some(directory) = parent {
            external_ignores.insert(directory.join(".gitignore"));
            external_ignores.insert(directory.join(".ignore"));
            ancestors.push(directory.to_path_buf());
            parent = directory.parent();
        }
        let callback_ignores = external_ignores.clone();
        let mut watcher = notify::recommended_watcher(move |event| {
            callback.event(event, &callback_root, &callback_ignores)
        })?;
        watcher.watch(root, RecursiveMode::Recursive)?;
        for directory in ancestors {
            watcher.watch(&directory, RecursiveMode::NonRecursive)?;
        }
        Ok(Self {
            _watcher: watcher,
            receiver,
            hints,
            last_rescan: Mutex::new(Instant::now()),
        })
    }
    pub fn next(&self) -> Option<Activity> {
        self.next_with_interval(RESCAN_INTERVAL)
    }
    fn next_with_interval(&self, interval: Duration) -> Option<Activity> {
        let elapsed = self.last_rescan.lock().unwrap().elapsed();
        if elapsed >= interval {
            *self.last_rescan.lock().unwrap() = Instant::now();
            return Some(Activity {
                resync: true,
                watcher_error: false,
            });
        }
        // The bounded timeout observes backend flags without requiring an event
        // and guarantees a canonical full reconciliation at least once per interval.
        let remaining = interval - elapsed;
        let _ = self
            .receiver
            .recv_timeout(remaining.min(Duration::from_secs(1)));
        if self.hints.flags.load(Ordering::SeqCst) & DIRTY == 0 {
            if self.last_rescan.lock().unwrap().elapsed() >= interval {
                *self.last_rescan.lock().unwrap() = Instant::now();
                return Some(Activity {
                    resync: true,
                    watcher_error: false,
                });
            }
            return None;
        }
        std::thread::sleep(Duration::from_millis(100));
        let activity = take_activity(&self.hints, &self.receiver);
        if activity.is_some() {
            *self.last_rescan.lock().unwrap() = Instant::now();
        }
        activity
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn callback_is_bounded_coalesces_and_preserves_overflow_and_errors() {
        let (hints, receiver) = channel();
        hints.event(
            Ok(Event::new(EventKind::Any).add_path("untrusted/outside.py".into())),
            Path::new("/repo"),
            &BTreeSet::new(),
        );
        let first = take_activity(&hints, &receiver).unwrap();
        assert!(!first.resync && !first.watcher_error);
        assert!(take_activity(&hints, &receiver).is_none());
        for _ in 0..10_000 {
            hints.signal(false);
        }
        let overflow = take_activity(&hints, &receiver).unwrap();
        assert!(overflow.resync && !overflow.watcher_error);
        hints.event(
            Err(notify::Error::generic("backend overflow")),
            Path::new("/repo"),
            &BTreeSet::new(),
        );
        let error = take_activity(&hints, &receiver).unwrap();
        assert!(error.resync && error.watcher_error);
        hints.event(
            Ok(Event::new(EventKind::Access(
                notify::event::AccessKind::Any,
            ))),
            Path::new("/repo"),
            &BTreeSet::new(),
        );
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
        hints.event(
            Ok(Event::new(EventKind::Any).add_path("outside/not-a-source.py".into())),
            Path::new("/repo"),
            &BTreeSet::new(),
        );
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

    #[test]
    fn parent_ignore_controls_wake_but_unrelated_parent_activity_does_not() {
        let (hints, receiver) = channel();
        let ignore = PathBuf::from("/repo/.gitignore");
        let controls = BTreeSet::from([ignore.clone(), PathBuf::from("/repo/.ignore")]);
        hints.event(
            Ok(Event::new(EventKind::Modify(notify::event::ModifyKind::Any)).add_path(ignore)),
            Path::new("/repo/sub"),
            &controls,
        );
        assert!(take_activity(&hints, &receiver).is_some());
        hints.event(
            Ok(
                Event::new(EventKind::Modify(notify::event::ModifyKind::Any))
                    .add_path("/repo/unrelated.txt".into()),
            ),
            Path::new("/repo/sub"),
            &controls,
        );
        assert!(take_activity(&hints, &receiver).is_none());
    }

    #[test]
    fn periodic_reconciliation_fires_even_without_backend_events() {
        let root = tempfile::tempdir().unwrap();
        let watcher = Watch::new(root.path()).unwrap();
        *watcher.last_rescan.lock().unwrap() = Instant::now() - Duration::from_secs(1);
        let activity = watcher
            .next_with_interval(Duration::from_millis(10))
            .unwrap();
        assert!(activity.resync);
        assert!(!activity.watcher_error);
    }

    #[test]
    fn parent_ignore_file_change_wakes_nested_root_watch() {
        let parent = tempfile::tempdir().unwrap();
        let root_path = parent.path().join("sub");
        std::fs::create_dir(&root_path).unwrap();
        let root = std::fs::canonicalize(root_path).unwrap();
        std::fs::write(root.join("a.py"), "def visible(): pass\n").unwrap();
        let watcher = Watch::new(&root).unwrap();
        std::fs::write(root.parent().unwrap().join(".gitignore"), "/sub/a.py\n").unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut woke = false;
        while Instant::now() < deadline {
            if watcher
                .next_with_interval(Duration::from_millis(50))
                .is_some()
            {
                woke = true;
                break;
            }
        }
        assert!(woke);
        let scan = crate::scan::scan_repository(&crate::model::ScanOptions {
            root,
            count_lines: false,
            max_line_count_file_bytes: 0,
            probe_git: false,
        })
        .unwrap();
        assert!(scan.files.is_empty());
    }
}
