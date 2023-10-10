#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]

pub extern crate hex;
mod api;
mod socket;
pub mod compat;
pub mod wallet;
extern crate log;