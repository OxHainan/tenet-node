#![warn(unused_crate_dependencies)]
#![allow(clippy::too_many_arguments)]

pub mod kv;
#[cfg(feature = "sql")]
pub mod sql;

use sp_runtime::traits::Block as BlockT;

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum SyncStrategy {
	Normal,
	Parachain,
}

pub type EthereumBlockNotificationSinks<T> =
	parking_lot::Mutex<Vec<sc_utils::mpsc::TracingUnboundedSender<T>>>;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct EthereumBlockNotification<Block: BlockT> {
	pub is_new_best: bool,
	pub hash: Block::Hash,
}
