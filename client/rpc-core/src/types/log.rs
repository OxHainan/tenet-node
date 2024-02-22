use super::Bytes;
use ethereum_types::{H160, H256, U256};
use serde::Serialize;

/// Log
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Log {
	/// H160
	pub address: H160,
	/// Topics
	pub topics: Vec<H256>,
	/// Data
	pub data: Bytes,
	/// Block Hash
	pub block_hash: Option<H256>,
	/// Block Number
	pub block_number: Option<U256>,
	/// Transaction Hash
	pub transaction_hash: Option<H256>,
	/// Transaction Index
	pub transaction_index: Option<U256>,
	/// Log Index in Block
	pub log_index: Option<U256>,
	/// Log Index in Transaction
	pub transaction_log_index: Option<U256>,
	/// Whether Log Type is Removed (Geth Compatibility Field)
	#[serde(default)]
	pub removed: bool,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub proof: Option<Vec<Bytes>>,
}
