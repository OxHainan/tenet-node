#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::too_many_arguments)]

pub use fp_rpc::{ConvertTransaction, NoTransactionConverter, RuntimeStorageOverride};
use scale_codec::{Decode, Encode};
use scale_info::TypeInfo;
use sp_std::vec::Vec;
// Substrate
use ethereum_types::Bloom;
use sp_core::{H160, H256, U256};
use sp_runtime::{traits::Block as BlockT, Permill, RuntimeDebug};
use tp_ethereum::Log;

#[derive(Clone, Eq, PartialEq, Default, RuntimeDebug, Encode, Decode, TypeInfo)]
pub struct TransactionStatus {
	pub transaction_hash: H256,
	pub transaction_index: u32,
	pub from: H160,
	pub to: Option<H160>,
	pub contract_address: Option<H160>,
	pub logs: Vec<Log>,
	pub logs_bloom: Bloom,
}

sp_api::decl_runtime_apis! {
	/// API necessary for Ethereum-compatibility layer.
	#[api_version(5)]
	pub trait EthereumRuntimeRPCApi {
		/// Returns runtime defined pallet_evm::ChainId.
		fn chain_id() -> u64;
		/// Returns pallet_evm::Accounts by address.
		fn account_basic(address: H160) -> fp_evm::Account;
		/// Returns FixedGasPrice::min_gas_price
		fn gas_price() -> U256;
		/// For a given account address, returns pallet_evm::AccountCodes.
		fn account_code_at(address: H160) -> Vec<u8>;
		fn has_account_public_key(address: H160) -> bool;
		fn account_public(address: H160) -> Vec<u8>;
		/// Returns the converted FindAuthor::find_author authority id.
		fn author() -> H160;

		/// Returns a frame_ethereum::call response. If `estimate` is true,
		#[changed_in(2)]
		fn call(
			from: H160,
			to: H160,
			data: Vec<u8>,
			value: U256,
			gas_limit: U256,
			gas_price: Option<U256>,
			nonce: Option<U256>,
			estimate: bool,
		) -> Result<fp_evm::ExecutionInfo::<Vec<u8>>, sp_runtime::DispatchError>;
		#[changed_in(4)]
		fn call(
			from: H160,
			to: H160,
			data: Vec<u8>,
			value: U256,
			gas_limit: U256,
			max_fee_per_gas: Option<U256>,
			max_priority_fee_per_gas: Option<U256>,
			nonce: Option<U256>,
			estimate: bool,
		) -> Result<fp_evm::ExecutionInfo::<Vec<u8>>, sp_runtime::DispatchError>;
		#[changed_in(5)]
		fn call(
			from: H160,
			to: H160,
			data: Vec<u8>,
			value: U256,
			gas_limit: U256,
			max_fee_per_gas: Option<U256>,
			max_priority_fee_per_gas: Option<U256>,
			nonce: Option<U256>,
			estimate: bool,
			access_list: Option<Vec<(H160, Vec<H256>)>>,
		) -> Result<fp_evm::ExecutionInfo::<Vec<u8>>, sp_runtime::DispatchError>;
		fn call(
			from: H160,
			to: H160,
			data: Vec<u8>,
			value: U256,
			gas_limit: U256,
			max_fee_per_gas: Option<U256>,
			max_priority_fee_per_gas: Option<U256>,
			nonce: Option<U256>,
			estimate: bool,
			access_list: Option<Vec<(H160, Vec<H256>)>>,
		) -> Result<fp_evm::ExecutionInfoV2::<Vec<u8>>, sp_runtime::DispatchError>;
		/// Returns a frame_ethereum::create response.
		#[changed_in(2)]
		fn create(
			from: H160,
			data: Vec<u8>,
			value: U256,
			gas_limit: U256,
			gas_price: Option<U256>,
			nonce: Option<U256>,
			estimate: bool,
		) -> Result<fp_evm::ExecutionInfo::<H160>, sp_runtime::DispatchError>;
		#[changed_in(4)]
		fn create(
			from: H160,
			data: Vec<u8>,
			value: U256,
			gas_limit: U256,
			max_fee_per_gas: Option<U256>,
			max_priority_fee_per_gas: Option<U256>,
			nonce: Option<U256>,
			estimate: bool,
		) -> Result<fp_evm::ExecutionInfo::<H160>, sp_runtime::DispatchError>;
		#[changed_in(5)]
		fn create(
			from: H160,
			data: Vec<u8>,
			value: U256,
			gas_limit: U256,
			max_fee_per_gas: Option<U256>,
			max_priority_fee_per_gas: Option<U256>,
			nonce: Option<U256>,
			estimate: bool,
			access_list: Option<Vec<(H160, Vec<H256>)>>,
		) -> Result<fp_evm::ExecutionInfo::<H160>, sp_runtime::DispatchError>;
		fn create(
			from: H160,
			data: Vec<u8>,
			value: U256,
			gas_limit: U256,
			max_fee_per_gas: Option<U256>,
			max_priority_fee_per_gas: Option<U256>,
			nonce: Option<U256>,
			estimate: bool,
			access_list: Option<Vec<(H160, Vec<H256>)>>,
		) -> Result<fp_evm::ExecutionInfoV2::<H160>, sp_runtime::DispatchError>;
		/// Return the current block. Legacy.
		#[changed_in(2)]
		fn current_block() -> Option<tp_ethereum::BlockV0>;
		/// Return the current block.
		fn current_block() -> Option<tp_ethereum::BlockV2>;
		/// Return the current receipt.
		fn current_receipts() -> Option<Vec<tp_ethereum::Receipt>>;
		/// Return the current transaction status.
		fn current_transaction_statuses() -> Option<Vec<TransactionStatus>>;
		/// Return all the current data for a block in a single runtime call. Legacy.
		#[changed_in(2)]
		fn current_all() -> (
			Option<tp_ethereum::BlockV0>,
			Option<Vec<TransactionStatus>>
		);
		/// Return all the current data for a block in a single runtime call.
		#[changed_in(4)]
		fn current_all() -> (
			Option<tp_ethereum::BlockV2>,
			Option<Vec<tp_ethereum::Receipt>>,
			Option<Vec<TransactionStatus>>
		);
		fn current_all() -> (
			Option<tp_ethereum::BlockV2>,
			Option<Vec<tp_ethereum::Receipt>>,
			Option<Vec<TransactionStatus>>
		);
		/// Receives a `Vec<OpaqueExtrinsic>` and filters all the ethereum transactions. Legacy.
		#[changed_in(2)]
		fn extrinsic_filter(
			xts: Vec<<Block as BlockT>::Extrinsic>,
		) -> Vec<tp_ethereum::TransactionV0>;
		/// Receives a `Vec<OpaqueExtrinsic>` and filters all the ethereum transactions.
		fn extrinsic_filter(
			xts: Vec<<Block as BlockT>::Extrinsic>,
		) -> Vec<tp_ethereum::TransactionV2>;
		/// Return the elasticity multiplier.
		fn elasticity() -> Option<Permill>;
		/// Used to determine if gas limit multiplier for non-transactional calls (eth_call/estimateGas)
		/// is supported.
		fn gas_limit_multiplier_support();
		/// Return the pending block.
		fn pending_block(
			xts: Vec<<Block as BlockT>::Extrinsic>,
		) -> (Option<tp_ethereum::BlockV2>, Option<Vec<TransactionStatus>>);
	}

	#[api_version(2)]
	pub trait ConvertTransactionRuntimeApi {
		fn convert_transaction(transaction: tp_ethereum::TransactionV2) -> <Block as BlockT>::Extrinsic;
		#[changed_in(2)]
		fn convert_transaction(transaction: tp_ethereum::TransactionV0) -> <Block as BlockT>::Extrinsic;
	}
}
