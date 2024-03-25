// SPDX-License-Identifier: Apache-2.0
// This file is part of Frontier.
//
// Copyright (c) 2020-2023 Parity Technologies (UK) Ltd.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![cfg_attr(not(feature = "std"), no_std)]

use scale_codec::{Decode, Encode};
use scale_info::TypeInfo;
// Substrate
use sp_core::{ecdsa, RuntimeDebug, H160, H256};
use sp_io::hashing::keccak_256;

#[derive(Eq, PartialEq, Clone, RuntimeDebug, Encode, Decode, TypeInfo)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EthereumSignature(ecdsa::Signature);

impl EthereumSignature {
	pub fn new(s: ecdsa::Signature) -> Self {
		EthereumSignature(s)
	}

	pub fn recover_signer<L: sp_runtime::traits::Lazy<[u8]>>(
		&self,
		mut msg: L,
	) -> Option<sp_core::H160> {
		let m = keccak_256(msg.get());
		match sp_io::crypto::secp256k1_ecdsa_recover(self.0.as_ref(), &m) {
			Ok(pubkey) => Some(H160::from(H256::from(keccak_256(&pubkey)))),
			Err(sp_io::EcdsaVerifyError::BadRS) => {
				log::error!(target: "evm", "Error recovering: Incorrect value of R or S");
				None
			}
			Err(sp_io::EcdsaVerifyError::BadV) => {
				log::error!(target: "evm", "Error recovering: Incorrect value of V");
				None
			}
			Err(sp_io::EcdsaVerifyError::BadSignature) => {
				log::error!(target: "evm", "Error recovering: Invalid signature");
				None
			}
		}
	}
}
