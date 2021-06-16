use std::sync::Arc;

use tokio::sync::mpsc::Sender;
use tokio::sync::{Notify, RwLock};

use common::output_event::Stream as OutputStream;
use common::OutputEvent;

/// Internal state of the `OutputHandler`
struct State {
    history: Vec<Vec<u8>>,
    completed: bool,
}

/// Handles a single output stream
pub struct OutputHandler {
    /// Stdout or Stderr
    stream_type: OutputStream,
    /// Internal state
    state: RwLock<State>,
    /// State change notification
    notify: Notify,
}
impl OutputHandler {
    pub fn new(stream_type: OutputStream) -> Self {
        Self {
            stream_type,
            state: RwLock::new(State {
                history: Vec::new(),
                completed: false,
            }),
            notify: Notify::new(),
        }
    }

    /// Sets up a new OutputHandler and the stream into it from any `AsyncRead`-object,
    /// usually either ChildStdout or ChildStderr.
    pub fn setup<R>(stream_type: OutputStream, pipe: R) -> Arc<Self>
    where
        R: tokio::io::AsyncRead + Unpin + Send + 'static,
    {
        use futures_util::StreamExt;

        let arc_self = Arc::new(OutputHandler::new(stream_type));

        let inner = arc_self.clone();
        tokio::spawn(async move {
            let mut out = tokio_util::io::ReaderStream::new(pipe);
            while let Some(value) = out.next().await {
                let x = value.expect("Process output error");
                inner.push(x.to_vec()).await;
            }
            inner.complete().await;
        });

        arc_self
    }

    /// Push new data to the history, notifying all waiting processes
    pub async fn push(&self, data: Vec<u8>) {
        let mut state = self.state.write().await;
        assert!(
            !state.completed,
            "Trying to push more output to a completed stream"
        );
        state.history.push(data);
        self.notify.notify_waiters();
    }

    /// Mark the process as complete. `push` must not be called after this.
    pub async fn complete(&self) {
        let mut state = self.state.write().await;
        state.completed = true;
        self.notify.notify_waiters();
    }
}

/// Start a task that streams from an `OutputHandler` to a mpsc channel.
pub fn stream_to(from: Arc<OutputHandler>, to: Sender<Result<OutputEvent, tonic::Status>>) {
    use std::borrow::Borrow;
    tokio::spawn(async move {
        let mut index = 0;
        loop {
            let h: &OutputHandler = from.borrow();
            let state = h.state.read().await;
            if index < state.history.len() {
                let send_result = to
                    .send(Ok(OutputEvent {
                        stream: h.stream_type as i32,
                        output: state.history[index].clone(),
                    }))
                    .await;

                if send_result.is_err() {
                    // Send failed, meaning that the other end has hung up.
                    // In this case it doesn't make sense to stream any more
                    // output, but this is not an error either.
                    log::debug!("Stream receiver has hung up, ending stream");
                    break;
                }

                index += 1;
            } else if state.completed {
                // All content has been streamed
                break;
            } else {
                // Wait until more output is available
                h.notify.notified().await;
            }
        }
    });
}
