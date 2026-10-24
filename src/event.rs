//! Event sources merged into one channel: ticks, terminal input, backend
//! events and termination signals (01 §3, VOL-ARCH-012, VOL-ARCH-062).

use std::time::{Duration, Instant};

use color_eyre::eyre::OptionExt;
use futures::StreamExt;
use ratatui::crossterm::event::{Event as CrosstermEvent, EventStream};
use tokio::signal::unix::{SignalKind, signal};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::backend::BackendEvent;

/// The frequency at which tick events are emitted (maximum draw rate).
pub const TICK_FPS: f64 = 30.0;

/// Everything the app loop reacts to.
#[derive(Clone, Debug)]
pub enum Event {
    /// Emitted at [`TICK_FPS`] with the current time; the app draws only on
    /// ticks and takes its clock from them.
    Tick(Instant),
    /// Terminal input (keys, resize, …).
    Crossterm(CrosstermEvent),
    /// A message from the audio backend.
    Backend(BackendEvent),
    /// `SIGTERM`, `SIGHUP` or `SIGINT` arrived: quit like `q`.
    Quit,
}

/// Owns the receiving end of the merged event channel. Dropping it stops
/// every task it spawned (each one exits once the channel is closed).
#[derive(Debug)]
pub struct EventHandler {
    receiver: UnboundedReceiver<Event>,
}

impl EventHandler {
    /// Spawns the tick, terminal, backend-forwarding and signal tasks.
    /// Must be called inside a tokio runtime.
    ///
    /// # Errors
    ///
    /// Fails if a signal handler cannot be registered.
    pub fn new(backend: UnboundedReceiver<BackendEvent>) -> std::io::Result<Self> {
        let (sender, receiver) = unbounded_channel();
        let signals = [
            signal(SignalKind::terminate())?,
            signal(SignalKind::hangup())?,
            signal(SignalKind::interrupt())?,
        ];
        tokio::spawn(ticks(sender.clone()));
        tokio::spawn(terminal(sender.clone()));
        tokio::spawn(forward_backend(sender.clone(), backend));
        tokio::spawn(forward_signals(sender, signals));
        Ok(Self { receiver })
    }

    /// Waits for the next event.
    ///
    /// # Errors
    ///
    /// Fails if every event task has stopped.
    pub async fn next(&mut self) -> color_eyre::Result<Event> {
        self.receiver
            .recv()
            .await
            .ok_or_eyre("all event sources stopped")
    }
}

/// Emits [`Event::Tick`] at [`TICK_FPS`].
async fn ticks(sender: UnboundedSender<Event>) {
    let mut interval = tokio::time::interval(Duration::from_secs_f64(1.0 / TICK_FPS));
    loop {
        tokio::select! {
            () = sender.closed() => break,
            at = interval.tick() => {
                if sender.send(Event::Tick(at.into_std())).is_err() {
                    break;
                }
            }
        }
    }
}

/// Forwards terminal input until the terminal stream ends or fails.
async fn terminal(sender: UnboundedSender<Event>) {
    let mut reader = EventStream::new();
    loop {
        tokio::select! {
            () = sender.closed() => break,
            item = reader.next() => match item {
                Some(Ok(event)) => {
                    if sender.send(Event::Crossterm(event)).is_err() {
                        break;
                    }
                }
                Some(Err(_)) | None => break,
            },
        }
    }
}

/// Forwards each backend event as [`Event::Backend`] (VOL-ARCH-012).
async fn forward_backend(
    sender: UnboundedSender<Event>,
    mut backend: UnboundedReceiver<BackendEvent>,
) {
    loop {
        tokio::select! {
            () = sender.closed() => break,
            item = backend.recv() => match item {
                Some(event) => {
                    if sender.send(Event::Backend(event)).is_err() {
                        break;
                    }
                }
                None => break,
            },
        }
    }
}

/// Turns `SIGTERM`/`SIGHUP`/`SIGINT` into [`Event::Quit`] (VOL-ARCH-062).
async fn forward_signals(
    sender: UnboundedSender<Event>,
    [mut term, mut hup, mut int]: [tokio::signal::unix::Signal; 3],
) {
    loop {
        tokio::select! {
            () = sender.closed() => break,
            _ = term.recv() => {}
            _ = hup.recv() => {}
            _ = int.recv() => {}
        }
        if sender.send(Event::Quit).is_err() {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::mock::{empty_connected, spawn_mock};

    const TIMEOUT: Duration = Duration::from_secs(2);

    /// Next non-tick event (terminal input cannot occur in tests).
    async fn next_non_tick(events: &mut EventHandler) -> Event {
        loop {
            let event = tokio::time::timeout(TIMEOUT, events.next())
                .await
                .expect("timed out waiting for an event")
                .unwrap();
            if !matches!(event, Event::Tick(_)) {
                return event;
            }
        }
    }

    #[tokio::test]
    async fn backend_events_are_forwarded_as_event_backend() {
        let (handle, control) = spawn_mock(empty_connected());
        let mut events = EventHandler::new(handle.events).unwrap();
        let initial = empty_connected().initial;
        match next_non_tick(&mut events).await {
            Event::Backend(event) => assert_eq!(event, initial[0]),
            other => panic!("expected a backend event, got {other:?}"),
        }
        let pushed = BackendEvent::Batch(Vec::new());
        control.push(pushed.clone()).unwrap();
        match next_non_tick(&mut events).await {
            Event::Backend(event) => assert_eq!(event, pushed),
            other => panic!("expected a backend event, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn dropping_receiver_stops_tasks() {
        let (handle, control) = spawn_mock(empty_connected());
        let events = EventHandler::new(handle.events).unwrap();
        drop(events);
        // The forwarder owns the backend receiver; once it exits, pushing fails.
        tokio::time::timeout(TIMEOUT, async {
            while control.push(BackendEvent::Batch(Vec::new())).is_ok() {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("backend forwarder still running after EventHandler was dropped");
    }
}
