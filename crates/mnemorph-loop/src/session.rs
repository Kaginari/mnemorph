//! A session: one body's run, and the lines the human typed while it worked.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, PoisonError};

use mnemorph_provider::Event;

use crate::run::{Ending, Interrupt, Run};

/// Lines queued while a turn runs; the interface pushes, the session pops.
pub type Queue = Arc<Mutex<VecDeque<String>>>;

/// A run and its queue lane.
pub struct Session {
    run: Run,
    queue: Queue,
}

impl Session {
    /// A session over `run`, its queue empty.
    #[must_use]
    pub fn new(run: Run) -> Self {
        Self {
            run,
            queue: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    /// The queue the interface pushes a line into when Enter is pressed during a turn.
    #[must_use]
    pub fn queue(&self) -> Queue {
        Arc::clone(&self.queue)
    }

    /// The handle the interface fires on Esc.
    #[must_use]
    pub fn interrupt(&self) -> Arc<Interrupt> {
        self.run.interrupt()
    }

    /// The send lane: a line pushed here reaches the running body at its next tool step.
    #[must_use]
    pub fn steer(&self) -> crate::run::Steer {
        self.run.steer()
    }

    /// The run.
    pub fn run(&mut self) -> &mut Run {
        &mut self.run
    }

    /// Ask `line`, then every queued line in turn, until the queue is empty. An interrupted turn
    /// pivots to the next queued line, as a finished one does. After each turn the wake runs the
    /// shared work queue's held orders that their bodies can carry out.
    pub async fn drive(&mut self, line: &str, on: &mut (dyn FnMut(Event) + Send)) -> Vec<Ending> {
        let mut endings = Vec::new();
        let mut next = Some(line.to_owned());
        while let Some(line) = next {
            let ending = self.run.ask(&line, on).await;
            self.run.file_triage(&ending);
            Box::pin(self.run.wake(&ending)).await;
            endings.push(ending);
            next = self
                .queue
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .pop_front();
        }
        endings
    }
}
