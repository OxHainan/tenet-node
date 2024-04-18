#![cfg_attr(not(feature = "std"), no_std)]

use rlp::{Encodable, RlpStream};
use sp_core::{Bytes, H256, U256};
use sp_std::vec::Vec;
use tp_ethereum::{AccessListItem, Log, Receipt, TransactionV2 as Transaction};
extern crate alloc;

pub struct TxInput {
	chain_id: u64,
	nonce: U256,
	max_priority_fee_per_gas: U256,
	max_fee_per_gas: U256,
	value: U256,
	input: Bytes,
	access_list: AccessListItem,
	r: H256,
	s: H256,
}
impl TxInput {
	pub fn hash(&self) -> H256 {
		let encoded = rlp::encode(self);
		let mut out = alloc::vec![0; 1 + encoded.len()];
		out[0] = 2;
		out[1..].copy_from_slice(&encoded);
		H256::from(sp_io::hashing::keccak_256(&out))
	}
}
impl Encodable for TxInput {
	fn rlp_append(&self, s: &mut RlpStream) {
		s.begin_list(9)
			.append(&self.chain_id)
			.append(&self.nonce)
			.append(&self.max_priority_fee_per_gas)
			.append(&self.max_fee_per_gas)
			.append(&self.value)
			.append(&self.input.to_vec())
			.append(&self.access_list)
			.append(&U256::from_big_endian(&self.r[..]))
			.append(&U256::from_big_endian(&self.s[..]));
	}
}

pub struct TxOutput {
	pub status_code: u8,
	pub used_gas: U256,
	pub logs: Vec<Log>,
}
impl TxOutput {
	pub fn hash(&self) -> H256 {
		let encoded = rlp::encode(self);
		let mut out = alloc::vec![0; 1 + encoded.len()];
		out[0] = 2;
		out[1..].copy_from_slice(&encoded);
		H256::from(sp_io::hashing::keccak_256(&out))
	}
}
impl Encodable for TxOutput {
	fn rlp_append(&self, s: &mut RlpStream) {
		s.begin_list(2)
			.append(&self.status_code)
			.append(&self.used_gas)
			.append_list(&self.logs);
	}
}
pub struct TenetApi {}
impl TenetApi {
	pub fn generate_input_hash(transaction: &Transaction) -> H256 {
		transaction.hash()
	}

	pub fn generate_output_hash(receipt: &Receipt) -> H256 {
		let tenet_app_output = match receipt {
			Receipt::Legacy(t) => TxOutput {
				status_code: t.status_code,
				used_gas: t.used_gas,
				logs: t.logs.clone(),
			},
			Receipt::EIP2930(t) => TxOutput {
				status_code: t.status_code,
				used_gas: t.used_gas,
				logs: t.logs.clone(),
			},
			Receipt::EIP1559(t) => TxOutput {
				status_code: t.status_code,
				used_gas: t.used_gas,
				logs: t.logs.clone(),
			},
		};
		tenet_app_output.hash()
	}
}
