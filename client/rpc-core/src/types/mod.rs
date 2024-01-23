use ethereum_types::H160;
pub use fc_rpc_core::types::{
	AccountInfo, BlockNumberOrHash, Bytes, CallRequest, CallStateOverride, ChainStatus, EthAccount,
	EthProtocolInfo, ExtAccountInfo, FeeHistory, FeeHistoryCache, FeeHistoryCacheItem,
	FeeHistoryCacheLimit, Filter, FilterAddress, FilterChanges, FilterPool, FilterPoolItem,
	FilterType, FilteredParams, Index, LocalTransactionStatus, Log, PeerCount, PeerInfo,
	PeerNetworkInfo, PeerProtocolsInfo, Peers, PipProtocolInfo, Receipt, RecoveredAccount,
	StorageProof, SyncInfo, SyncStatus, Topic, TransactionStats, VariadicValue, Work,
};

#[cfg(feature = "txpool")]
pub use self::txpool::{Summary, TransactionMap, TxPoolResult};
pub use self::{
	block::{Block, BlockTransactions, Header, Rich, RichBlock, RichHeader},
	transaction::{RichRawTransaction, Transaction},
	transaction_request::{TransactionMessage, TransactionRequest},
};
mod block;
pub mod pubsub;
mod transaction;
mod transaction_request;
#[cfg(feature = "txpool")]
mod txpool;
use serde::{de::Error, Deserialize, Deserializer};
use tp_ethereum::TransactionV2 as EthereumTransaction;

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize)]
pub(crate) struct CallOrInputData {
	data: Option<Bytes>,
	input: Option<Bytes>,
}

/// Function to deserialize `data` and `input`  within `TransactionRequest` and `CallRequest`.
/// It verifies that if both `data` and `input` are provided, they must be identical.
pub(crate) fn deserialize_data_or_input<'d, D: Deserializer<'d>>(
	d: D,
) -> Result<Option<Bytes>, D::Error> {
	let CallOrInputData { data, input } = CallOrInputData::deserialize(d)?;
	match (&data, &input) {
		(Some(data), Some(input)) => {
			if data == input {
				Ok(Some(data.clone()))
			} else {
				Err(D::Error::custom(
					"Ambiguous value for `data` and `input`".to_string(),
				))
			}
		}
		(_, _) => Ok(data.or(input)),
	}
}

/// The trait that used to build types from the `from` address and ethereum `transaction`.
pub trait BuildFrom {
	fn build_from(from: H160, transaction: &EthereumTransaction) -> Self;
}
