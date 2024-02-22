use ethereum_types::{Bloom as H2048, H160, H256, U256, U64};
use serde::Serialize;

use super::log::Log;
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Receipt {
	/// Transaction Hash
	pub transaction_hash: Option<H256>,
	/// Transaction index
	pub transaction_index: Option<U256>,
	/// Block hash
	pub block_hash: Option<H256>,
	/// Sender
	pub from: Option<H160>,
	/// Recipient
	pub to: Option<H160>,
	/// Block number
	pub block_number: Option<U256>,
	/// Cumulative gas used
	pub cumulative_gas_used: U256,
	/// Gas used
	pub gas_used: Option<U256>,
	/// Contract address
	pub contract_address: Option<H160>,
	/// Logs
	pub logs: Vec<Log>,
	/// State Root
	// NOTE(niklasad1): EIP98 makes this optional field, if it's missing then skip serializing it
	#[serde(skip_serializing_if = "Option::is_none", rename = "root")]
	pub state_root: Option<H256>,
	/// Logs bloom
	pub logs_bloom: H2048,
	/// Status code
	// NOTE(niklasad1): Unknown after EIP98 rules, if it's missing then skip serializing it
	#[serde(skip_serializing_if = "Option::is_none", rename = "status")]
	pub status_code: Option<U64>,
	/// Effective gas price. Pre-eip1559 this is just the gasprice. Post-eip1559 this is base fee + priority fee.
	pub effective_gas_price: U256,
	/// EIP-2718 type
	#[serde(rename = "type")]
	pub transaction_type: U256,
}
