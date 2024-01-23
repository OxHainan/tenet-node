use sp_std::sync::Arc;

use eth_trie::{EthTrie, MemoryDB, Trie, TrieError, DB};
use ethereum_types::H256;
use hash_db::Hasher;

pub fn order_verify_proof<H>(
	proof: Vec<Vec<u8>>,
	root_hash: H::Out,
	indexed: usize,
) -> Result<Option<Vec<u8>>, TrieError>
where
	H: Hasher<Out = H256>,
{
	verify_proof::<H>(root_hash, &rlp::encode(&indexed), proof)
}

fn verify_proof<H>(
	root_hash: H::Out,
	key: &[u8],
	proof: Vec<Vec<u8>>,
) -> Result<Option<Vec<u8>>, TrieError>
where
	H: Hasher<Out = H256>,
{
	let proof_db = Arc::new(MemoryDB::new(true));
	for node_encoded in proof.into_iter() {
		let hash = H::hash(&node_encoded);

		if root_hash.eq(&hash) || node_encoded.len() >= 32 {
			proof_db.insert(hash.as_bytes(), node_encoded).unwrap();
		}
	}

	EthTrie::new(proof_db)
		.at_root(root_hash)
		.get(key)
		.or(Err(TrieError::InvalidProof))
}

pub fn order_generate_proof<H, I, V>(
	input: I,
	indexed: usize,
) -> Result<(H256, Vec<Vec<u8>>), eth_trie::TrieError>
where
	H: Hasher,
	I: IntoIterator<Item = V>,
	V: AsRef<[u8]>,
{
	generate_proof::<H, _, _, _>(
		input
			.into_iter()
			.enumerate()
			.map(|(i, v)| (rlp::encode(&i), v)),
		rlp::encode(&indexed),
	)
}

pub fn generate_proof<H, I, A, B>(
	input: I,
	key: A,
) -> Result<(H256, Vec<Vec<u8>>), eth_trie::TrieError>
where
	H: Hasher,
	I: IntoIterator<Item = (A, B)>,
	A: AsRef<[u8]> + Ord,
	B: AsRef<[u8]>,
{
	let mut trie = EthTrie::new(Arc::new(eth_trie::MemoryDB::new(true)));
	for (key, val) in input.into_iter() {
		trie.insert(key.as_ref(), val.as_ref()).unwrap();
	}

	Ok((trie.root_hash()?, trie.get_proof(key.as_ref())?))
}
