//! Substrate Node Template CLI library.

#![warn(missing_docs)]
#![allow(
	clippy::type_complexity,
	clippy::too_many_arguments,
	clippy::large_enum_variant
)]

mod chain_spec;
mod cli;
mod client;
mod command;
mod eth;
mod rpc;
mod service;

pub use command::*;
