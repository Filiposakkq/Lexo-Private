//! The thread that owns the application. `App` is not `Send` (it keeps `Rc` caches), so it lives on one thread and everything else hands it jobs; one job runs at a time,
//! like the lock of the Python server.
use lexo_core::app::App;
use std::sync::mpsc::{channel, Sender};

type Job = Box<dyn FnOnce(&mut App) + Send>;

/// The way to the application thread. Cheap to clone.
#[derive(Clone)]
pub struct Backend {
    tx: Sender<Job>,
}

impl Backend {
    /// Start the thread; `make_app` builds the application on it.
    pub fn spawn(make_app: impl FnOnce() -> App + Send + 'static) -> Backend {
        let (tx, rx) = channel::<Job>();
        std::thread::spawn(move || {
            let mut app = make_app();
            while let Ok(job) = rx.recv() {
                job(&mut app);
            }
            app.close();
        });
        Backend { tx }
    }

    /// Run `f` on the application thread and wait for its result; `Err` carries the text of a panic.
    pub fn run<R: Send + 'static>(&self, f: impl FnOnce(&mut App) -> R + Send + 'static) -> Result<R, String> {
        let (rtx, rrx) = channel();
        let job: Job = Box::new(move |app| {
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(app)));
            let _ = rtx.send(r.map_err(|p| {
                p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_else(|| "panic".to_string())
            }));
        });
        self.tx.send(job).map_err(|_| "the application thread is gone".to_string())?;
        rrx.recv().map_err(|_| "the application thread is gone".to_string())?
    }
}
