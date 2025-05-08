#![cfg_attr(not(feature = "std"), no_std)]
use rlp::{Encodable, RlpEncodable, RlpStream};
use secp256k1::{ecdsa::Signature, Message, Secp256k1, SecretKey};
use sp_core::H256;
use sp_std::{collections::vec_deque::VecDeque, vec::Vec};
#[derive(Clone)]
pub struct PoC {
	io_hash_list: Vec<IOHash>,
	pub sign: Signature,
}
impl Encodable for PoC {
	fn rlp_append(&self, s: &mut RlpStream) {
		s.append_list(&self.io_hash_list);
		// s.append(&self.sign.to_string().as_bytes());
	}
}

#[derive(Clone, RlpEncodable)]
pub struct IOHash {
	pub input_hash: H256,
	pub output_hash: H256,
}

pub fn generate_poc(private_key: H256, io_list: &Vec<IOHash>) -> PoC {
	let root = generate_root(io_list);

	PoC {
		io_hash_list: io_list.clone(),
		sign: Secp256k1::new().sign_ecdsa(
			&Message::from_digest(*root.as_fixed_bytes()),
			&SecretKey::from_slice(private_key.as_bytes()).unwrap(),
		),
	}
}

fn generate_root(io_list: &Vec<IOHash>) -> H256 {
	if io_list.is_empty() {
		return H256::zero();
	}
	let mut hash_queue = VecDeque::new();
	for io in io_list {
		hash_queue.push_back(io.input_hash);
		hash_queue.push_back(io.output_hash);
	}
	while hash_queue.len() > 1 {
		let hash1 = hash_queue.pop_front().unwrap();
		let hash2 = hash_queue.pop_front().unwrap();
		let combined_hash: Vec<u8> = [
			&hash1.to_fixed_bytes().to_vec()[..],
			&hash2.to_fixed_bytes().to_vec()[..],
		]
		.concat();
		hash_queue.push_back(H256::from(sp_io::hashing::keccak_256(&combined_hash)));
	}
	hash_queue.pop_front().unwrap()
}
