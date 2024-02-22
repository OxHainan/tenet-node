use fp_storage::EthereumStorageSchema;
use sc_client_api::{backend::Backend, StorageProvider};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::traits::Block as BlockT;
use std::{collections::BTreeMap, sync::Arc};
use tp_rpc::EthereumRuntimeRPCApi;

mod overrides;
pub use fc_storage::onchain_storage_schema;
pub use overrides::*;

pub fn overrides_handle<B, C, BE>(client: Arc<C>) -> Arc<OverrideHandle<B>>
where
	B: BlockT,
	C: ProvideRuntimeApi<B>,
	C::Api: EthereumRuntimeRPCApi<B>,
	C: HeaderBackend<B> + StorageProvider<B, BE> + 'static,
	BE: Backend<B> + 'static,
{
	let mut overrides_map = BTreeMap::new();
	overrides_map.insert(
		EthereumStorageSchema::V3,
		Box::new(SchemaV3Override::new(client.clone())) as Box<dyn StorageOverride<_>>,
	);

	Arc::new(OverrideHandle {
		schemas: overrides_map,
		fallback: Box::new(RuntimeApiStorageOverride::<B, C>::new(client)),
	})
}
