use std::sync::Arc;

use tokio::sync::mpsc::Sender;
use tokio::sync::{Notify, RwLock};

use common::output_event::Stream as OutputStream;
use common::OutputEvent;

struct State {
    history: Vec<Vec<u8>>,
    completed: bool,
}

pub struct OutputHandler {
    stream_type: OutputStream,
    state: RwLock<State>,
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

    pub async fn push(&self, data: Vec<u8>) {
        let mut state = self.state.write().await;
        state.history.push(data);
        self.notify.notify_waiters();
    }

    pub async fn complete(&self) {
        let mut state = self.state.write().await;
        state.completed = true;
        self.notify.notify_waiters();
    }
}

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
