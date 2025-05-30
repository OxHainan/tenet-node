use std::{collections::BTreeMap, sync::Arc};

use jsonrpsee::RpcModule;
// Substrate
use sc_client_api::{
	backend::{Backend, StorageProvider},
	client::BlockchainEvents,
	AuxStore, UsageProvider,
};
use sc_network::NetworkService;
use sc_network_sync::SyncingService;
use sc_rpc::SubscriptionTaskExecutor;
use sc_transaction_pool::{ChainApi, Pool};
use sc_transaction_pool_api::TransactionPool;
use sp_api::{CallApiAt, ProvideRuntimeApi};
use sp_block_builder::BlockBuilder as BlockBuilderApi;
use sp_blockchain::{Error as BlockChainError, HeaderBackend, HeaderMetadata};
use sp_consensus_aura::{sr25519::AuthorityId as AuraId, AuraApi};
use sp_core::H256;
use sp_inherents::CreateInherentDataProviders;
use sp_runtime::traits::Block as BlockT;

// Frontier
pub use tc_rpc::{EthBlockDataCacheTask, EthConfig, OverrideHandle};
pub use tc_rpc_core::types::{FeeHistoryCache, FeeHistoryCacheLimit, FilterPool};
pub use tc_storage::overrides_handle;
use tp_rpc::{ConvertTransaction, ConvertTransactionRuntimeApi, EthereumRuntimeRPCApi};

/// Extra dependencies for Ethereum compatibility.
pub struct EthDeps<B: BlockT, C, P, A: ChainApi, CT, CIDP> {
	/// The client instance to use.
	pub client: Arc<C>,
	/// Transaction pool instance.
	pub pool: Arc<P>,
	/// Graph pool instance.
	pub graph: Arc<Pool<A>>,
	/// Ethereum transaction converter.
	pub converter: Option<CT>,
	/// The Node authority flag
	pub is_authority: bool,
	/// Whether to enable dev signer
	pub enable_dev_signer: bool,
	/// Network service
	pub network: Arc<NetworkService<B, B::Hash>>,
	/// Chain syncing service
	pub sync: Arc<SyncingService<B>>,
	/// Frontier Backend.
	pub frontier_backend: Arc<dyn fc_api::Backend<B>>,
	/// Ethereum data access overrides.
	pub overrides: Arc<OverrideHandle<B>>,
	/// Cache for Ethereum block data.
	pub block_data_cache: Arc<EthBlockDataCacheTask<B>>,
	/// EthFilterApi pool.
	pub filter_pool: Option<FilterPool>,
	/// Maximum number of logs in a query.
	pub max_past_logs: u32,
	/// Fee history cache.
	pub fee_history_cache: FeeHistoryCache,
	/// Maximum fee history cache size.
	pub fee_history_cache_limit: FeeHistoryCacheLimit,
	/// Maximum allowed gas limit will be ` block.gas_limit * execute_gas_limit_multiplier` when
	/// using eth_call/eth_estimateGas.
	pub execute_gas_limit_multiplier: u64,
	/// Mandated parent hashes for a given block hash.
	pub forced_parent_hashes: Option<BTreeMap<H256, H256>>,
	/// Something that can create the inherent data providers for pending state
	pub pending_create_inherent_data_providers: CIDP,
}

/// Instantiate Ethereum-compatible RPC extensions.
pub fn create_eth<B, C, BE, P, A, CT, CIDP, EC>(
	mut io: RpcModule<()>,
	deps: EthDeps<B, C, P, A, CT, CIDP>,
	subscription_task_executor: SubscriptionTaskExecutor,
	pubsub_notification_sinks: Arc<
		tc_mapping_sync::EthereumBlockNotificationSinks<
			tc_mapping_sync::EthereumBlockNotification<B>,
		>,
	>,
) -> Result<RpcModule<()>, Box<dyn std::error::Error + Send + Sync>>
where
	B: BlockT<Hash = H256>,
	C: CallApiAt<B> + ProvideRuntimeApi<B>,
	C::Api: AuraApi<B, AuraId>
		+ BlockBuilderApi<B>
		+ ConvertTransactionRuntimeApi<B>
		+ EthereumRuntimeRPCApi<B>,
	C: HeaderBackend<B> + HeaderMetadata<B, Error = BlockChainError>,
	C: BlockchainEvents<B> + AuxStore + UsageProvider<B> + StorageProvider<B, BE> + 'static,
	BE: Backend<B> + 'static,
	P: TransactionPool<Block = B> + 'static,
	A: ChainApi<Block = B> + 'static,
	CT: ConvertTransaction<<B as BlockT>::Extrinsic> + Send + Sync + 'static,
	CIDP: CreateInherentDataProviders<B, ()> + Send + 'static,
	EC: EthConfig<B, C>,
{
	use tc_rpc::{
		pending::AuraConsensusDataProvider, Eth, EthApiServer, EthDevSigner, EthFilter,
		EthFilterApiServer, EthPubSub, EthPubSubApiServer, EthSigner, Net, NetApiServer, Web3,
		Web3ApiServer,
	};
	#[cfg(feature = "txpool")]
	use tc_rpc::{TxPool, TxPoolApiServer};

	let EthDeps {
		client,
		pool,
		graph,
		converter,
		is_authority,
		enable_dev_signer,
		network,
		sync,
		frontier_backend,
		overrides,
		block_data_cache,
		filter_pool,
		max_past_logs,
		fee_history_cache,
		fee_history_cache_limit,
		execute_gas_limit_multiplier,
		forced_parent_hashes,
		pending_create_inherent_data_providers,
	} = deps;

	let mut signers = Vec::new();
	if enable_dev_signer {
		signers.push(Box::new(EthDevSigner::new()) as Box<dyn EthSigner>);
	}

	io.merge(
		Eth::<B, C, P, CT, BE, A, CIDP, EC>::new(
			client.clone(),
			pool.clone(),
			graph.clone(),
			converter,
			sync.clone(),
			signers,
			overrides.clone(),
			frontier_backend.clone(),
			is_authority,
			block_data_cache.clone(),
			fee_history_cache,
			fee_history_cache_limit,
			execute_gas_limit_multiplier,
			forced_parent_hashes,
			pending_create_inherent_data_providers,
			Some(Box::new(AuraConsensusDataProvider::new(client.clone()))),
		)
		.replace_config::<EC>()
		.into_rpc(),
	)?;

	if let Some(filter_pool) = filter_pool {
		io.merge(
			EthFilter::new(
				client.clone(),
				frontier_backend,
				graph.clone(),
				filter_pool,
				500_usize, // max stored filters
				max_past_logs,
				block_data_cache,
			)
			.into_rpc(),
		)?;
	}

	io.merge(
		EthPubSub::new(
			pool,
			client.clone(),
			sync,
			subscription_task_executor,
			overrides,
			pubsub_notification_sinks,
		)
		.into_rpc(),
	)?;

	io.merge(
		Net::new(
			client.clone(),
			network,
			// Whether to format the `peer_count` response as Hex (default) or not.
			true,
		)
		.into_rpc(),
	)?;

	io.merge(Web3::new(client.clone()).into_rpc())?;

	#[cfg(feature = "txpool")]
	io.merge(TxPool::new(client, graph).into_rpc())?;

	Ok(io)
}
