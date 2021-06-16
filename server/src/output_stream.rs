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
    // TODO: error handling
    let out_to = to;
    tokio::spawn(async move {
        let mut index = 0;
        loop {
            let h: &OutputHandler = from.borrow();
            let state = h.state.read().await;
            if index < state.history.len() {
                out_to
                    .send(Ok(OutputEvent {
                        stream: h.stream_type as i32,
                        output: state.history[index].clone(),
                    }))
                    .await
                    .unwrap(); // TODO: handle
                index += 1;
            } else if state.completed {
                break;
            } else {
                h.notify.notified().await;
            }
        }
    });
}
