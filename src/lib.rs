#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]

mod api;
mod socket;
mod compat;
pub mod wallet;
pub mod async_client;
pub use async_client::MempoolAsync;