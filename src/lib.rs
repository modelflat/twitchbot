#![feature(test)]

extern crate pest;
#[macro_use]
extern crate pest_derive;

pub mod core;
pub mod irc;
pub mod event;
pub mod commands;
pub mod util;
pub mod message_history;
