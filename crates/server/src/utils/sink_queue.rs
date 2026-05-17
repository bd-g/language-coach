//! Non-blocking queue helper for driving sink writes inside `tokio::select!` loops.
//!
//! `SinkQueue` splits `SinkExt::send(...).await` into readiness, `start_send`, and flush steps so
//! a caller can poll sink progress as one branch of a `tokio::select!` loop.
//!
//! ```ignore
//! let mut queue = SinkQueue::default();
//! loop {
//!     enum Event<E> {
//!         SinkReady(Result<(), E>),
//!         Work,
//!     }
//!
//!     let event = {
//!         let mut sink_ready = queue.ready_or_flushed(&mut sink);
//!         tokio::select! {
//!             Some(result) = &mut sink_ready => Event::SinkReady(result),
//!             _ = do_work() => Event::Work,
//!         }
//!     };
//!
//!     match event {
//!         Event::SinkReady(Ok(())) => {
//!             queue.send_or_note_flushed(&mut sink, |item| item)?;
//!         }
//!         Event::SinkReady(Err(error)) => return Err(error),
//!         Event::Work => queue.push("message"),
//!     }
//! }
//! ```

use std::collections::VecDeque;

use futures_util::{
    Sink, SinkExt,
    future::{OptionFuture, poll_fn},
};

/// Queue state for a sink that is advanced without awaiting a full send call.
pub(crate) struct SinkQueue<T> {
    items: VecDeque<T>,
    flushing: bool,
}

impl<T> Default for SinkQueue<T> {
    fn default() -> Self {
        Self {
            items: VecDeque::new(),
            flushing: false,
        }
    }
}

/// Synchronous progress made after a sink readiness future completes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum QueueStep {
    /// An item was started with `Sink::start_send` and now needs flushing.
    StartedItem,
    /// A previously started item was flushed.
    FinishedFlush,
    /// The queue had no pending work.
    Idle,
}

impl<T> SinkQueue<T> {
    /// Adds one item to the back of the queue.
    pub(crate) fn push(&mut self, item: T) {
        self.items.push_back(item);
    }

    /// Adds several items to the back of the queue.
    pub(crate) fn extend<I>(&mut self, items: I)
    where
        I: IntoIterator<Item = T>,
    {
        self.items.extend(items);
    }

    /// Returns true when no item is queued or awaiting flush.
    pub(crate) fn is_idle(&self) -> bool {
        self.items.is_empty() && !self.flushing
    }

    /// Returns a future that resolves when the sink can be advanced, or `None` when idle.
    pub(crate) fn ready_or_flushed<'a, S, Out>(
        &self,
        sink: &'a mut S,
    ) -> OptionFuture<impl std::future::Future<Output = Result<(), S::Error>> + 'a>
    where
        S: Sink<Out> + Unpin,
    {
        OptionFuture::from((!self.is_idle()).then(|| {
            let flushing = self.flushing;
            poll_fn(move |context| {
                if flushing {
                    sink.poll_flush_unpin(context)
                } else {
                    sink.poll_ready_unpin(context)
                }
            })
        }))
    }

    /// Starts the next send or records that the previous send was flushed.
    pub(crate) fn send_or_note_flushed<S, Out, Map>(
        &mut self,
        sink: &mut S,
        map: Map,
    ) -> Result<QueueStep, S::Error>
    where
        S: Sink<Out> + Unpin,
        Map: FnOnce(T) -> Out,
    {
        if self.flushing {
            self.flushing = false;
            return Ok(QueueStep::FinishedFlush);
        }

        if let Some(item) = self.items.pop_front() {
            sink.start_send_unpin(map(item))?;
            self.flushing = true;
            return Ok(QueueStep::StartedItem);
        }

        Ok(QueueStep::Idle)
    }
}
