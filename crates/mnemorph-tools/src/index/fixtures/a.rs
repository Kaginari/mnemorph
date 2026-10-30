//! A crate.

use std::fmt;
use std::io::{self, Read};

/// The answer.
pub const ANSWER: u32 = 42;

/// A stack.
#[derive(Debug)]
pub struct Stack<T> {
    items: Vec<T>,
}

impl<T> Stack<T> {
    /// A new stack.
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn push(&mut self, t: T) {
        self.items.push(t);
    }
}

impl fmt::Display for Stack<u8> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "stack")
    }
}

pub trait Shape {
    fn area(&self) -> f64;
}

pub enum Kind {
    A,
    B,
}

mod inner {
    pub fn helper() {}
}

macro_rules! twice {
    ($e:expr) => {
        $e + $e
    };
}
