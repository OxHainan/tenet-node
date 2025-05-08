use tp_ethereum::{AccessListItem, TransactionAction, TransactionV2 as EthereumTransaction};

use ethereum_types::{H160, H256, H512, U256, U64};
use serde::Serialize;

use crate::types::{BuildFrom, Bytes};

/// Transaction
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Transaction {
	/// EIP-2718 transaction type
	#[serde(rename = "type")]
	pub transaction_type: U256,
	/// Hash
	pub hash: H256,
	/// Nonce
	pub nonce: U256,
	/// Block hash
	pub block_hash: Option<H256>,
	/// Block number
	pub block_number: Option<U256>,
	/// Transaction Index
	pub transaction_index: Option<U256>,
	/// Sender
	pub from: H160,
	/// Recipient
	pub to: Option<H160>,
	/// Transferred value
	pub value: U256,
	/// Gas
	pub gas: U256,
	/// Gas Price
	#[serde(skip_serializing_if = "Option::is_none")]
	pub gas_price: Option<U256>,
	/// Max BaseFeePerGas the user is willing to pay.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub max_fee_per_gas: Option<U256>,
	/// The miner's tip.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub max_priority_fee_per_gas: Option<U256>,
	/// Data
	pub input: Bytes,
	/// Creates contract
	pub creates: Option<H160>,
	/// Raw transaction data
	pub raw: Bytes,
	/// Public key of the signer.
	pub public_key: Option<H512>,
	/// The network id of the transaction, if any.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub chain_id: Option<U64>,
	/// Pre-pay to warm storage access.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub access_list: Option<Vec<AccessListItem>>,
	/// The parity (0 for even, 1 for odd) of the y-value of the secp256k1 signature.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub y_parity: Option<U256>,
	/// The standardised V field of the signature.
	///
	/// For backwards compatibility, `v` is optionally provided as an alternative to `yParity`.
	/// This field is DEPRECATED and all use of it should migrate to `yParity`.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub v: Option<U256>,
	/// The R field of the signature.
	pub r: U256,
	/// The S field of the signature.
	pub s: U256,
}

impl BuildFrom for Transaction {
	fn build_from(from: H160, transaction: &EthereumTransaction) -> Self {
		let serialized = ethereum::EnvelopedEncodable::encode(transaction);
		let hash = transaction.hash();
		let raw = Bytes(serialized.to_vec());
		match transaction {
			EthereumTransaction::Legacy(t) => Self {
				transaction_type: U256::from(0),
				hash,
				nonce: t.nonce,
				block_hash: None,
				block_number: None,
				transaction_index: None,
				from,
				to: match t.action {
					TransactionAction::Call(to) => Some(to),
					TransactionAction::Create => None,
				},
				value: t.value,
				gas: t.gas_limit,
				gas_price: Some(t.gas_price),
				max_fee_per_gas: None,
				max_priority_fee_per_gas: None,
				input: Bytes(t.input.clone()),
				creates: None,
				raw,
				public_key: None,
				chain_id: t.signature.chain_id().map(U64::from),
				access_list: None,
				y_parity: None,
				v: Some(U256::from(t.signature.v())),
				r: U256::from(t.signature.r().as_bytes()),
				s: U256::from(t.signature.s().as_bytes()),
			},
			EthereumTransaction::EIP2930(t) => Self {
				transaction_type: U256::from(1),
				hash,
				nonce: t.nonce,
				block_hash: None,
				block_number: None,
				transaction_index: None,
				from,
				to: match t.action {
					TransactionAction::Call(to) => Some(to),
					TransactionAction::Create => None,
				},
				value: t.value,
				gas: t.gas_limit,
				gas_price: Some(t.gas_price),
				max_fee_per_gas: None,
				max_priority_fee_per_gas: None,
				input: Bytes(t.input.clone()),
				creates: None,
				raw,
				public_key: None,
				chain_id: Some(U64::from(t.chain_id)),
				access_list: Some(t.access_list.clone()),
				y_parity: Some(U256::from(t.odd_y_parity as u8)),
				v: Some(U256::from(t.odd_y_parity as u8)),
				r: U256::from(t.r.as_bytes()),
				s: U256::from(t.s.as_bytes()),
			},
			EthereumTransaction::EIP1559(t) => {
				let (
					value,
					max_fee_per_gas,
					max_priority_fee_per_gas,
					gas_limit,
					access_list,
					input,
				) = match &t.method {
					ethereum::TransactionMethod::Confidential(con) => (
						None,
						Some(con.max_fee_per_gas),
						Some(con.max_priority_fee_per_gas),
						con.gas_limit,
						None,
						None,
					),
					ethereum::TransactionMethod::Universal(uni) => (
						Some(uni.value),
						Some(uni.max_fee_per_gas),
						Some(uni.max_priority_fee_per_gas),
						uni.gas_limit,
						Some(uni.access_list.clone()),
						Some(uni.input.clone()),
					),
				};
				Self {
					transaction_type: U256::from(2),
					hash,
					nonce: t.nonce,
					block_hash: None,
					block_number: None,
					transaction_index: None,
					from: H160::default(),
					to: None,
					value: value.unwrap_or_default(),
					gas_price: None,
					max_fee_per_gas,
					max_priority_fee_per_gas,
					gas: gas_limit,
					input: Bytes(input.unwrap_or_default()),
					creates: None,
					raw,
					public_key: None,
					chain_id: Some(U64::from(t.chain_id)),
					y_parity: Some(U256::from(t.odd_y_parity as u8)),
					v: Some(U256::from(t.odd_y_parity as u8)),
					r: U256::from(t.r.as_bytes()),
					s: U256::from(t.s.as_bytes()),
					access_list,
				}
			}
		}
	}
}

/// Geth-compatible output for eth_signTransaction method
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct RichRawTransaction {
	/// Raw transaction RLP
	pub raw: Bytes,
	/// Transaction details
	#[serde(rename = "tx")]
	pub transaction: Transaction,
}
