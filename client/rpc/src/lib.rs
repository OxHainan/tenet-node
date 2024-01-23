// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0
// This file is part of Frontier.
//
// Copyright (c) 2020-2022 Parity Technologies (UK) Ltd.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

#![allow(
	clippy::too_many_arguments,
	clippy::large_enum_variant,
	clippy::manual_range_contains,
	clippy::explicit_counter_loop,
	clippy::len_zero,
	clippy::new_without_default
)]
#![warn(unused_crate_dependencies)]

mod cache;
mod eth;
mod eth_pubsub;
mod net;
mod signer;
#[cfg(feature = "txpool")]
mod txpool;
mod web3;

#[cfg(feature = "txpool")]
pub use self::txpool::TxPool;
pub use self::{
	cache::{EthBlockDataCacheTask, EthTask},
	eth::{format, pending, EstimateGasAdapter, Eth, EthConfig, EthFilter},
	eth_pubsub::{EthPubSub, EthereumSubIdProvider},
	net::Net,
	signer::{EthDevSigner, EthSigner},
	web3::Web3,
};
#[cfg(feature = "txpool")]
pub use tc_rpc_core::TxPoolApiServer;
pub use tc_rpc_core::{
	EthApiServer, EthFilterApiServer, EthPubSubApiServer, NetApiServer, Web3ApiServer,
};
pub use tc_storage::{
	OverrideHandle, RuntimeApiStorageOverride, SchemaV3Override, StorageOverride,
};
pub use tp_ethereum::TransactionV2 as EthereumTransaction;

pub mod frontier_backend_client {
	use super::internal_err;

	use ethereum_types::{H160, H256, U256};
	use jsonrpsee::core::RpcResult;
	use scale_codec::Encode;
	// Substrate
	use sc_client_api::{
		backend::{Backend, StorageProvider},
		StorageKey,
	};
	use sp_blockchain::HeaderBackend;
	use sp_io::hashing::{blake2_128, twox_128};
	use sp_runtime::{
		generic::BlockId,
		traits::{Block as BlockT, HashingFor, UniqueSaturatedInto},
	};
	use sp_state_machine::OverlayedChanges;
	// Frontier
	use tc_rpc_core::types::BlockNumberOrHash;

	/// Implements a default runtime storage override.
	/// It assumes that the balances and nonces are stored in pallet `system.account`, and
	/// have `nonce: Index` = `u32` for  and `free: Balance` = `u128`.
	/// Uses IdentityAddressMapping for the address.
	pub struct SystemAccountId20StorageOverride<B, C, BE>(pub std::marker::PhantomData<(B, C, BE)>);
	impl<B, C, BE> tp_rpc::RuntimeStorageOverride<B, C> for SystemAccountId20StorageOverride<B, C, BE>
	where
		B: BlockT,
		C: StorageProvider<B, BE> + Send + Sync,
		BE: Backend<B>,
	{
		fn is_enabled() -> bool {
			true
		}

		fn set_overlayed_changes(
			client: &C,
			overlayed_changes: &mut OverlayedChanges<HashingFor<B>>,
			block: B::Hash,
			_version: u32,
			address: H160,
			balance: Option<U256>,
			nonce: Option<U256>,
		) {
			let mut key = [twox_128(b"System"), twox_128(b"Account")]
				.concat()
				.to_vec();
			let account_id = Self::into_account_id_bytes(address);
			key.extend(blake2_128(&account_id));
			key.extend(&account_id);

			if let Ok(Some(item)) = client.storage(block, &StorageKey(key.clone())) {
				let mut new_item = item.0;

				if let Some(nonce) = nonce {
					new_item.splice(0..4, nonce.low_u32().encode());
				}

				if let Some(balance) = balance {
					new_item.splice(16..32, balance.low_u128().encode());
				}

				overlayed_changes.set_storage(key, Some(new_item));
			}
		}

		fn into_account_id_bytes(address: H160) -> Vec<u8> {
			use pallet_evm::AddressMapping;
			let address: H160 = pallet_evm::IdentityAddressMapping::into_account_id(address);
			address.as_ref().to_owned()
		}
	}

	/// Implements a runtime storage override.
	/// It assumes that the balances and nonces are stored in pallet `system.account`, and
	/// have `nonce: Index` = `u32` for  and `free: Balance` = `u128`.
	/// USes HashedAddressMapping for the address.
	pub struct SystemAccountId32StorageOverride<B, C, BE>(pub std::marker::PhantomData<(B, C, BE)>);
	impl<B, C, BE> tp_rpc::RuntimeStorageOverride<B, C> for SystemAccountId32StorageOverride<B, C, BE>
	where
		B: BlockT,
		C: StorageProvider<B, BE> + Send + Sync,
		BE: Backend<B>,
	{
		fn is_enabled() -> bool {
			true
		}

		fn set_overlayed_changes(
			client: &C,
			overlayed_changes: &mut OverlayedChanges<HashingFor<B>>,
			block: B::Hash,
			_version: u32,
			address: H160,
			balance: Option<U256>,
			nonce: Option<U256>,
		) {
			let mut key = [twox_128(b"System"), twox_128(b"Account")]
				.concat()
				.to_vec();
			let account_id = Self::into_account_id_bytes(address);
			key.extend(blake2_128(&account_id));
			key.extend(&account_id);

			if let Ok(Some(item)) = client.storage(block, &StorageKey(key.clone())) {
				let mut new_item = item.0;

				if let Some(nonce) = nonce {
					new_item.splice(0..4, nonce.low_u32().encode());
				}

				if let Some(balance) = balance {
					new_item.splice(16..32, balance.low_u128().encode());
				}

				overlayed_changes.set_storage(key, Some(new_item));
			}
		}

		fn into_account_id_bytes(address: H160) -> Vec<u8> {
			use pallet_evm::AddressMapping;
			use sp_core::crypto::ByteArray;
			use sp_runtime::traits::BlakeTwo256;

			pallet_evm::HashedAddressMapping::<BlakeTwo256>::into_account_id(address)
				.as_slice()
				.to_owned()
		}
	}

	pub async fn native_block_id<B: BlockT, C>(
		client: &C,
		backend: &dyn fc_api::Backend<B>,
		number: Option<BlockNumberOrHash>,
	) -> RpcResult<Option<BlockId<B>>>
	where
		B: BlockT,
		C: HeaderBackend<B> + 'static,
	{
		Ok(match number.unwrap_or(BlockNumberOrHash::Latest) {
			BlockNumberOrHash::Hash { hash, .. } => {
				if let Ok(Some(hash)) = load_hash::<B, C>(client, backend, hash).await {
					Some(BlockId::Hash(hash))
				} else {
					None
				}
			}
			BlockNumberOrHash::Num(number) => Some(BlockId::Number(number.unique_saturated_into())),
			BlockNumberOrHash::Latest => Some(BlockId::Hash(client.info().best_hash)),
			BlockNumberOrHash::Earliest => Some(BlockId::Hash(client.info().genesis_hash)),
			BlockNumberOrHash::Pending => None,
			BlockNumberOrHash::Safe => Some(BlockId::Hash(client.info().finalized_hash)),
			BlockNumberOrHash::Finalized => Some(BlockId::Hash(client.info().finalized_hash)),
		})
	}

	pub async fn load_hash<B: BlockT, C>(
		client: &C,
		backend: &dyn fc_api::Backend<B>,
		hash: H256,
	) -> RpcResult<Option<B::Hash>>
	where
		B: BlockT,
		C: HeaderBackend<B> + 'static,
	{
		let substrate_hashes = backend
			.block_hash(&hash)
			.await
			.map_err(|err| internal_err(format!("fetch aux store failed: {:?}", err)))?;

		if let Some(substrate_hashes) = substrate_hashes {
			for substrate_hash in substrate_hashes {
				if is_canon::<B, C>(client, substrate_hash) {
					return Ok(Some(substrate_hash));
				}
			}
		}
		Ok(None)
	}

	pub fn is_canon<B: BlockT, C>(client: &C, target_hash: B::Hash) -> bool
	where
		B: BlockT,
		C: HeaderBackend<B> + 'static,
	{
		if let Ok(Some(number)) = client.number(target_hash) {
			if let Ok(Some(hash)) = client.hash(number) {
				return hash == target_hash;
			}
		}
		false
	}

	pub async fn load_transactions<B: BlockT, C>(
		client: &C,
		backend: &dyn fc_api::Backend<B>,
		transaction_hash: H256,
		only_canonical: bool,
	) -> RpcResult<Option<(H256, u32)>>
	where
		B: BlockT,
		C: HeaderBackend<B> + 'static,
	{
		let transaction_metadata = backend
			.transaction_metadata(&transaction_hash)
			.await
			.map_err(|err| internal_err(format!("fetch aux store failed: {:?}", err)))?;

		transaction_metadata
			.iter()
			.find(|meta| is_canon::<B, C>(client, meta.substrate_block_hash))
			.map_or_else(
				|| {
					if !only_canonical && transaction_metadata.len() > 0 {
						Ok(Some((
							transaction_metadata[0].ethereum_block_hash,
							transaction_metadata[0].ethereum_index,
						)))
					} else {
						Ok(None)
					}
				},
				|meta| Ok(Some((meta.ethereum_block_hash, meta.ethereum_index))),
			)
	}
}

pub fn err<T: ToString>(code: i32, message: T, data: Option<&[u8]>) -> jsonrpsee::core::Error {
	jsonrpsee::core::Error::Call(jsonrpsee::types::error::CallError::Custom(
		jsonrpsee::types::error::ErrorObject::owned(
			code,
			message.to_string(),
			data.map(|bytes| {
				jsonrpsee::core::to_json_raw_value(&format!("0x{}", hex::encode(bytes)))
					.expect("fail to serialize data")
			}),
		),
	))
}

pub fn internal_err<T: ToString>(message: T) -> jsonrpsee::core::Error {
	err(jsonrpsee::types::error::INTERNAL_ERROR_CODE, message, None)
}

pub fn internal_err_with_data<T: ToString>(message: T, data: &[u8]) -> jsonrpsee::core::Error {
	err(
		jsonrpsee::types::error::INTERNAL_ERROR_CODE,
		message,
		Some(data),
	)
}

pub fn public_key(transaction: &EthereumTransaction) -> Result<[u8; 64], sp_io::EcdsaVerifyError> {
	let mut sig = [0u8; 65];
	let mut msg = [0u8; 32];
	match transaction {
		EthereumTransaction::Legacy(t) => {
			sig[0..32].copy_from_slice(&t.signature.r()[..]);
			sig[32..64].copy_from_slice(&t.signature.s()[..]);
			sig[64] = t.signature.standard_v();
			msg.copy_from_slice(&tp_ethereum::LegacyTransactionMessage::from(t.clone()).hash()[..]);
		}
		EthereumTransaction::EIP2930(t) => {
			sig[0..32].copy_from_slice(&t.r[..]);
			sig[32..64].copy_from_slice(&t.s[..]);
			sig[64] = t.odd_y_parity as u8;
			msg.copy_from_slice(
				&tp_ethereum::EIP2930TransactionMessage::from(t.clone()).hash()[..],
			);
		}
		EthereumTransaction::EIP1559(t) => {
			sig[0..32].copy_from_slice(&t.r[..]);
			sig[32..64].copy_from_slice(&t.s[..]);
			sig[64] = t.odd_y_parity as u8;
			msg.copy_from_slice(
				&tp_ethereum::EIP1559TransactionMessage::from(t.clone()).hash()[..],
			);
		}
	}
	sp_io::crypto::secp256k1_ecdsa_recover(&sig, &msg)
}
