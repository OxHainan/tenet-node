use std::{collections::BTreeMap, marker::PhantomData, sync::Arc};

use ethereum_types::H160;
use sp_api::{ApiExt, ProvideRuntimeApi};
use sp_io::hashing::{blake2_128, twox_128};
use sp_runtime::{traits::Block as BlockT, Permill};
use tp_ethereum::BlockV2 as EthereumBlock;

use fp_storage::EthereumStorageSchema;
use tp_rpc::{EthereumRuntimeRPCApi, TransactionStatus};

mod schema_v3_override;

pub use schema_v3_override::SchemaV3Override;

pub struct OverrideHandle<Block: BlockT> {
	pub schemas: BTreeMap<EthereumStorageSchema, Box<dyn StorageOverride<Block>>>,
	pub fallback: Box<dyn StorageOverride<Block>>,
}

pub trait StorageOverride<Block: BlockT>: Send + Sync {
	/// For a given account address, returns pallet_evm::AccountCodes.
	fn account_code_at(&self, block_hash: Block::Hash, address: H160) -> Option<Vec<u8>>;

	/// Return the current block.
	fn current_block(&self, block_hash: Block::Hash) -> Option<EthereumBlock>;
	/// Return the current receipt.
	fn current_receipts(&self, block_hash: Block::Hash) -> Option<Vec<tp_ethereum::Receipt>>;
	/// Return the current transaction status.
	fn current_transaction_statuses(
		&self,
		block_hash: Block::Hash,
	) -> Option<Vec<TransactionStatus>>;
	/// Return the base fee at the given height.
	fn elasticity(&self, block_hash: Block::Hash) -> Option<Permill>;
	/// Return `true` if the request BlockId is post-eip1559.
	fn is_eip1559(&self, block_hash: Block::Hash) -> bool;
}

fn storage_prefix_build(module: &[u8], storage: &[u8]) -> Vec<u8> {
	[twox_128(module), twox_128(storage)].concat().to_vec()
}

fn blake2_128_extend(bytes: &[u8]) -> Vec<u8> {
	let mut ext: Vec<u8> = blake2_128(bytes).to_vec();
	ext.extend_from_slice(bytes);
	ext
}

/// A wrapper type for the Runtime API. This type implements `StorageOverride`, so it can be used
/// when calling the runtime API is desired but a `dyn StorageOverride` is required.
pub struct RuntimeApiStorageOverride<B: BlockT, C> {
	client: Arc<C>,
	_marker: PhantomData<B>,
}

impl<B: BlockT, C> RuntimeApiStorageOverride<B, C> {
	pub fn new(client: Arc<C>) -> Self {
		Self {
			client,
			_marker: PhantomData,
		}
	}
}

impl<Block, C> StorageOverride<Block> for RuntimeApiStorageOverride<Block, C>
where
	Block: BlockT,
	C: ProvideRuntimeApi<Block> + Send + Sync,
	C::Api: EthereumRuntimeRPCApi<Block>,
{
	/// For a given account address, returns pallet_evm::AccountCodes.
	fn account_code_at(&self, block_hash: Block::Hash, address: H160) -> Option<Vec<u8>> {
		self.client
			.runtime_api()
			.account_code_at(block_hash, address)
			.ok()
	}

	/// Return the current block.
	fn current_block(&self, block_hash: Block::Hash) -> Option<tp_ethereum::BlockV2> {
		let api = self.client.runtime_api();

		let api_version = if let Ok(Some(api_version)) =
			api.api_version::<dyn EthereumRuntimeRPCApi<Block>>(block_hash)
		{
			api_version
		} else {
			return None;
		};
		if api_version == 1 {
			#[allow(deprecated)]
			let old_block = api.current_block_before_version_2(block_hash).ok()?;
			old_block.map(|block| block.into())
		} else {
			api.current_block(block_hash).ok()?
		}
	}

	/// Return the current receipt.
	fn current_receipts(&self, block_hash: Block::Hash) -> Option<Vec<tp_ethereum::Receipt>> {
		self.client
			.runtime_api()
			.current_receipts(block_hash)
			.ok()?
	}

	/// Return the current transaction status.
	fn current_transaction_statuses(
		&self,
		block_hash: Block::Hash,
	) -> Option<Vec<TransactionStatus>> {
		self.client
			.runtime_api()
			.current_transaction_statuses(block_hash)
			.ok()?
	}

	/// Return the elasticity multiplier at the give post-eip1559 height.
	fn elasticity(&self, block_hash: Block::Hash) -> Option<Permill> {
		if self.is_eip1559(block_hash) {
			self.client.runtime_api().elasticity(block_hash).ok()?
		} else {
			None
		}
	}

	fn is_eip1559(&self, block_hash: Block::Hash) -> bool {
		if let Ok(Some(api_version)) = self
			.client
			.runtime_api()
			.api_version::<dyn EthereumRuntimeRPCApi<Block>>(block_hash)
		{
			return api_version >= 2;
		}
		false
	}
}
