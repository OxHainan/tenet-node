// Substrate
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use tc_executor::{NativeElseWasmExecutor, NativeExecutionDispatch, NativeVersion};
// Local
use tenet_runtime::{opaque::Block, AccountId, Balance, Nonce};

use crate::eth::EthCompatRuntimeApiCollection;

/// Full backend.
pub type FullBackend = sc_service::TFullBackend<Block>;
/// Full client.
pub type FullClient<RuntimeApi, Executor> =
	sc_service::TFullClient<Block, RuntimeApi, NativeElseWasmExecutor<Executor>>;

pub type Client = FullClient<tenet_runtime::RuntimeApi, TemplateRuntimeExecutor>;

/// Only enable the benchmarking host functions when we actually want to benchmark.
#[cfg(feature = "runtime-benchmarks")]
pub type HostFunctions = frame_benchmarking::benchmarking::HostFunctions;
/// Otherwise we use empty host functions for ext host functions.
#[cfg(not(feature = "runtime-benchmarks"))]
pub type HostFunctions = ();

pub struct TemplateRuntimeExecutor;
impl NativeExecutionDispatch for TemplateRuntimeExecutor {
	type ExtendHostFunctions = HostFunctions;

	fn dispatch(method: &str, data: &[u8]) -> Option<Vec<u8>> {
		tenet_runtime::api::dispatch(method, data)
	}

	fn native_version() -> NativeVersion {
		tenet_runtime::native_version()
	}
}

/// A set of APIs that every runtimes must implement.
pub trait BaseRuntimeApiCollection:
	sp_api::ApiExt<Block>
	+ sp_api::Metadata<Block>
	+ sp_block_builder::BlockBuilder<Block>
	+ sp_session::SessionKeys<Block>
	+ sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block>
{
}

impl<Api> BaseRuntimeApiCollection for Api where
	Api: sp_api::ApiExt<Block>
		+ sp_api::Metadata<Block>
		+ sp_block_builder::BlockBuilder<Block>
		+ sp_session::SessionKeys<Block>
		+ sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block>
{
}

/// A set of APIs that template runtime must implement.
pub trait RuntimeApiCollection:
	BaseRuntimeApiCollection
	+ EthCompatRuntimeApiCollection
	+ sp_consensus_aura::AuraApi<Block, AuraId>
	+ sp_consensus_grandpa::GrandpaApi<Block>
	+ frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Nonce>
	+ pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<Block, Balance>
{
}

impl<Api> RuntimeApiCollection for Api where
	Api: BaseRuntimeApiCollection
		+ EthCompatRuntimeApiCollection
		+ sp_consensus_aura::AuraApi<Block, AuraId>
		+ sp_consensus_grandpa::GrandpaApi<Block>
		+ frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Nonce>
		+ pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<Block, Balance>
{
}
