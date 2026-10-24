//! Shared worker transport. Feature owners decide when a result may be committed.
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::thread::JoinHandle;

pub(crate) struct BackgroundJob<T> {
    receiver: mpsc::Receiver<Result<T, String>>,
    cancelled: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}
impl<T: Send + 'static> BackgroundJob<T> {
    pub fn start(
        name: &str,
        run: impl FnOnce(&dyn Fn() -> bool) -> Result<T, String> + Send + 'static,
    ) -> Result<Self, String> {
        let (sender, receiver) = mpsc::channel();
        let cancelled = Arc::new(AtomicBool::new(false));
        let flag = cancelled.clone();
        let worker = std::thread::Builder::new()
            .name(name.into())
            .spawn(move || {
                let check = || flag.load(Ordering::Relaxed);
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    if check() {
                        return Err("Operation cancelled".into());
                    }
                    let result = run(&check);
                    if check() {
                        Err("Operation cancelled".into())
                    } else {
                        result
                    }
                }))
                .unwrap_or_else(|_| {
                    Err("Background operation failed; original document kept".into())
                });
                let _ = sender.send(result);
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            receiver,
            cancelled,
            worker: Some(worker),
        })
    }
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
    pub fn poll(&mut self) -> Option<Result<T, String>> {
        let result = match self.receiver.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return None,
            Err(mpsc::TryRecvError::Disconnected) => {
                Err("Background worker stopped unexpectedly".into())
            }
        };
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        Some(if self.is_cancelled() {
            Err("Operation cancelled".into())
        } else {
            result
        })
    }
}
impl<T> Drop for BackgroundJob<T> {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn finish<T: Send + 'static>(job: &mut BackgroundJob<T>) -> Result<T, String> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            if let Some(result) = job.poll() {
                return result;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "worker did not finish"
            );
            std::thread::yield_now();
        }
    }
    #[test]
    fn worker_reports_success_panics_and_cancellation_and_joins() {
        let mut success = BackgroundJob::start("test-success", |_| Ok(42)).unwrap();
        assert_eq!(finish(&mut success).unwrap(), 42);
        assert!(success.worker.is_none());
        let mut panic = BackgroundJob::<()>::start("test-panic", |_| panic!("test panic")).unwrap();
        assert!(finish(&mut panic).is_err());
        let (started, ready) = mpsc::channel();
        let mut cancelled = BackgroundJob::start("test-cancel", move |cancel| {
            started.send(()).unwrap();
            while !cancel() {
                std::thread::yield_now();
            }
            Ok(42)
        })
        .unwrap();
        ready
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        cancelled.cancel();
        assert!(finish(&mut cancelled).unwrap_err().contains("cancelled"));
        assert!(cancelled.worker.is_none());
    }
}
