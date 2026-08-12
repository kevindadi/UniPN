use crate::domain::BindingEnv;

use super::marking::{ColoredMarking, PtMarking};

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct RuntimeState<M, G, T = ()> {
    pub marking: M,
    pub globals: G,
    pub time: T,
}

impl<M, G, T> RuntimeState<M, G, T> {
    pub fn new(marking: M, globals: G, time: T) -> Self {
        Self {
            marking,
            globals,
            time,
        }
    }
}

pub type PtState = RuntimeState<PtMarking, ()>;
pub type ColoredState = RuntimeState<ColoredMarking, BindingEnv>;
