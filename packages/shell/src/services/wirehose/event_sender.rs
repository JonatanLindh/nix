use std::cell::RefCell;

use pipewire::main_loop::MainLoopWeak;

use super::{Event, ObjectId, StateEvent};

/// Trait for handling [`Event`]s.
pub trait EventHandler: Send + 'static {
    /// Returns `true` if the event was handled successfully, `false` if the
    /// wirehose thread should shut down.
    fn handle_event(&mut self, event: Event) -> bool;
}

impl<F> EventHandler for F
where
    F: FnMut(Event) -> bool + Send + 'static,
{
    fn handle_event(&mut self, event: Event) -> bool {
        self(event)
    }
}

pub struct EventSender {
    handler: RefCell<Box<dyn EventHandler>>,
    main_loop_weak: MainLoopWeak,
}

impl EventSender {
    pub fn new<F: EventHandler>(handler: F, main_loop_weak: MainLoopWeak) -> Self {
        Self {
            handler: RefCell::new(Box::new(handler)),
            main_loop_weak,
        }
    }

    pub fn send(&self, event: StateEvent) {
        if !self.handler.borrow_mut().handle_event(Event::State(event))
            && let Some(main_loop) = self.main_loop_weak.upgrade()
        {
            main_loop.quit();
        }
    }

    pub fn send_ready(&self) {
        if !self.handler.borrow_mut().handle_event(Event::Ready)
            && let Some(main_loop) = self.main_loop_weak.upgrade()
        {
            main_loop.quit();
        }
    }

    pub fn send_error(&self, object_id: Option<ObjectId>, message: String) {
        let event = Event::Error { object_id, message };
        if !self.handler.borrow_mut().handle_event(event)
            && let Some(main_loop) = self.main_loop_weak.upgrade()
        {
            main_loop.quit();
        }
    }
}
