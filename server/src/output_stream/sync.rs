use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::{oneshot, Mutex};

struct Waiting {
    until_after: usize,
    tx: oneshot::Sender<()>,
}

/// A shared monotonically increasing generation counter.
pub(super) struct MonotonicCounter {
    /// The generation number itself
    generation: AtomicUsize,
    /// Channels used to wake up waiters
    /// TODO: keep sorted or use min-heap for better performance
    waiting: Mutex<Vec<Waiting>>,
}
impl MonotonicCounter {
    pub fn new() -> Self {
        Self {
            generation: AtomicUsize::new(0),
            waiting: Mutex::new(Vec::new()),
        }
    }

    /// Sets new generation number to at least `latest`
    pub async fn update(&self, latest: usize) {
        // Update value
        let current = self
            .generation
            .fetch_max(latest, Ordering::SeqCst)
            .max(latest);

        // Notify waiting tasks
        let mut waits = self.waiting.lock().await;

        let mut i = 0;
        while i < waits.len() {
            if waits[i].until_after < current {
                let val = waits.remove(i);
                // Ignore receiver drops, they are done on purpose
                let _ = val.tx.send(());
            } else {
                i += 1;
            }
        }
    }

    /// Register a oneshow channel that will be messaged when the generation is updated
    async fn register_wait(&self, until_after: usize, tx: oneshot::Sender<()>) {
        let mut w = self.waiting.lock().await;
        w.push(Waiting { until_after, tx });
    }

    /// Wait until generation number bypasses `last_seen`
    pub async fn wait_until_after(&self, last_seen: usize) {
        // Check if we can return immediately (fast path)
        if last_seen < self.generation.load(Ordering::SeqCst) {
            return;
        }

        // Register for a notification when our value is reached
        let (tx, rx) = oneshot::channel();
        self.register_wait(last_seen, tx).await;

        // Check if the value was reached when registering to avoids the race-condition
        // where the status was reached since the first start, causing us to wait until
        // the next event (that might never come).
        if last_seen < self.generation.load(Ordering::SeqCst) {
            return; // Drops the oneshot channel on purpose
        }

        // Wait until we get a message
        rx.await
            .expect("MonotonicCounter unexpectedly dropped a sender")
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use tokio::sync::oneshot::{self, error::TryRecvError};

    use super::MonotonicCounter;

    #[tokio::test]
    async fn test_single_consumer() {
        let mc = Arc::new(MonotonicCounter::new());

        let (tx, mut rx) = oneshot::channel();
        let inner = mc.clone();
        let handle = tokio::spawn(async move {
            inner.wait_until_after(100).await;
            tx.send(()).expect("Receiver dropped");
        });

        assert_eq!(rx.try_recv(), Err(TryRecvError::Empty));

        mc.update(1).await;
        assert_eq!(rx.try_recv(), Err(TryRecvError::Empty));

        mc.update(100).await;
        assert_eq!(rx.try_recv(), Err(TryRecvError::Empty));

        mc.update(101).await;
        assert_eq!(rx.await, Ok(()));

        handle.await.expect("task error");
    }

    #[tokio::test]
    async fn test_multiple_consumers() {
        let mc = Arc::new(MonotonicCounter::new());

        let (tx1, mut rx1) = oneshot::channel();
        let inner = mc.clone();
        tokio::spawn(async move {
            inner.wait_until_after(100).await;
            tx1.send(()).expect("Receiver dropped");
        });

        let (tx2, mut rx2) = oneshot::channel();
        let inner = mc.clone();
        tokio::spawn(async move {
            inner.wait_until_after(100).await;
            tx2.send(()).expect("Receiver dropped");
        });

        let (tx3, mut rx3) = oneshot::channel();
        let inner = mc.clone();
        tokio::spawn(async move {
            inner.wait_until_after(200).await;
            tx3.send(()).expect("Receiver dropped");
        });

        assert_eq!(rx1.try_recv(), Err(TryRecvError::Empty));
        assert_eq!(rx2.try_recv(), Err(TryRecvError::Empty));
        assert_eq!(rx3.try_recv(), Err(TryRecvError::Empty));

        mc.update(50).await;
        assert_eq!(rx1.try_recv(), Err(TryRecvError::Empty));
        assert_eq!(rx2.try_recv(), Err(TryRecvError::Empty));
        assert_eq!(rx3.try_recv(), Err(TryRecvError::Empty));

        mc.update(150).await;
        assert_eq!(rx1.await, Ok(()));
        assert_eq!(rx2.await, Ok(()));
        assert_eq!(rx3.try_recv(), Err(TryRecvError::Empty));

        mc.update(250).await;
        assert_eq!(rx3.await, Ok(()));
    }
}
