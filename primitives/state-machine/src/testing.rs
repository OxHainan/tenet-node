// This file is part of Substrate.

// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

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

//! Test implementation for Externalities.

use std::{
	any::{Any, TypeId},
	panic::{AssertUnwindSafe, UnwindSafe},
};

use crate::{
	backend::Backend, ext::Ext, InMemoryBackend, OverlayedChanges, StorageKey, StorageValue,
	TrieBackendBuilder,
};

use hash_db::{HashDB, Hasher};
use sp_core::{
	offchain::testing::TestPersistentOffchainDB,
	storage::{
		well_known_keys::{is_child_storage_key, CODE},
		StateVersion, Storage,
	},
};
use sp_externalities::{Extension, ExtensionStore, Extensions};
use sp_trie::{PrefixedMemoryDB, StorageProof};

/// Simple HashMap-based Externalities impl.
pub struct TestExternalities<H>
where
	H: Hasher + 'static,
	H::Out: codec::Codec + Ord,
{
	/// The overlay changed storage.
	overlay: OverlayedChanges<H>,
	offchain_db: TestPersistentOffchainDB,
	/// Storage backend.
	pub backend: InMemoryBackend<H>,
	/// Extensions.
	pub extensions: Extensions,
	/// State version to use during tests.
	pub state_version: StateVersion,
}

impl<H> TestExternalities<H>
where
	H: Hasher + 'static,
	H::Out: Ord + 'static + codec::Codec,
{
	/// Get externalities implementation.
	pub fn ext(&mut self) -> Ext<H, InMemoryBackend<H>> {
		Ext::new(&mut self.overlay, &self.backend, Some(&mut self.extensions))
	}

	/// Create a new instance of `TestExternalities` with storage.
	pub fn new(storage: Storage) -> Self {
		Self::new_with_code_and_state(&[], storage, Default::default())
	}

	/// Create a new instance of `TestExternalities` with storage for a given state version.
	pub fn new_with_state_version(storage: Storage, state_version: StateVersion) -> Self {
		Self::new_with_code_and_state(&[], storage, state_version)
	}

	/// New empty test externalities.
	pub fn new_empty() -> Self {
		Self::new_with_code_and_state(&[], Storage::default(), Default::default())
	}

	/// Create a new instance of `TestExternalities` with code and storage.
	pub fn new_with_code(code: &[u8], storage: Storage) -> Self {
		Self::new_with_code_and_state(code, storage, Default::default())
	}

	/// Create a new instance of `TestExternalities` with code and storage for a given state
	/// version.
	pub fn new_with_code_and_state(
		code: &[u8],
		mut storage: Storage,
		state_version: StateVersion,
	) -> Self {
		assert!(storage.top.keys().all(|key| !is_child_storage_key(key)));

		storage.top.insert(CODE.to_vec(), code.to_vec());

		let offchain_db = TestPersistentOffchainDB::new();

		let backend = (storage, state_version).into();

		TestExternalities {
			overlay: OverlayedChanges::default(),
			offchain_db,
			extensions: Default::default(),
			backend,
			state_version,
		}
	}

	/// Returns the overlayed changes.
	pub fn overlayed_changes(&self) -> &OverlayedChanges<H> {
		&self.overlay
	}

	/// Move offchain changes from overlay to the persistent store.
	pub fn persist_offchain_overlay(&mut self) {
		self.offchain_db
			.apply_offchain_changes(self.overlay.offchain_drain_committed());
	}

	/// A shared reference type around the offchain worker storage.
	pub fn offchain_db(&self) -> TestPersistentOffchainDB {
		self.offchain_db.clone()
	}

	/// Batch insert key/values into backend
	pub fn batch_insert<I>(&mut self, kvs: I)
	where
		I: IntoIterator<Item = (StorageKey, StorageValue)>,
	{
		self.backend.insert(
			Some((None, kvs.into_iter().map(|(k, v)| (k, Some(v))).collect())),
			self.state_version,
		);
	}

	/// Insert key/value into backend
	pub fn insert(&mut self, k: StorageKey, v: StorageValue) {
		self.backend
			.insert(vec![(None, vec![(k, Some(v))])], self.state_version);
	}

	/// Insert key/value into backend.
	///
	/// This only supports inserting keys in child tries.
	pub fn insert_child(&mut self, c: sp_core::storage::ChildInfo, k: StorageKey, v: StorageValue) {
		self.backend
			.insert(vec![(Some(c), vec![(k, Some(v))])], self.state_version);
	}

	/// Registers the given extension for this instance.
	pub fn register_extension<E: Any + Extension>(&mut self, ext: E) {
		self.extensions.register(ext);
	}

	/// Sets raw storage key/values and a root.
	///
	/// This can be used as a fast way to restore the storage state from a backup because the trie
	/// does not need to be computed.
	#[allow(clippy::type_complexity)]
	pub fn from_raw_snapshot(
		raw_storage: Vec<(Vec<u8>, (Vec<u8>, i32))>,
		storage_root: H::Out,
		state_version: StateVersion,
	) -> Self {
		let mut backend = PrefixedMemoryDB::default();

		for (key, (v, ref_count)) in raw_storage {
			let mut hash = H::Out::default();
			let hash_len = hash.as_ref().len();

			if key.len() < hash_len {
				log::warn!("Invalid key in `from_raw_snapshot`: {key:?}");
				continue;
			}

			hash.as_mut()
				.copy_from_slice(&key[(key.len() - hash_len)..]);

			// Each time .emplace is called the internal MemoryDb ref count increments.
			// Repeatedly call emplace to initialise the ref count to the correct value.
			for _ in 0..ref_count {
				backend.emplace(hash, (&key[..(key.len() - hash_len)], None), v.clone());
			}
		}

		Self {
			backend: TrieBackendBuilder::new(backend, storage_root).build(),
			overlay: Default::default(),
			offchain_db: Default::default(),
			extensions: Default::default(),
			state_version,
		}
	}

	/// Drains the underlying raw storage key/values and returns the root hash.
	///
	/// Useful for backing up the storage in a format that can be quickly re-loaded.
	#[allow(clippy::type_complexity)]
	pub fn into_raw_snapshot(mut self) -> (Vec<(Vec<u8>, (Vec<u8>, i32))>, H::Out) {
		let raw_key_values = self
			.backend
			.backend_storage_mut()
			.drain()
			.into_iter()
			.filter(|(_, (_, r))| *r > 0)
			.collect::<Vec<(Vec<u8>, (Vec<u8>, i32))>>();

		(raw_key_values, *self.backend.root())
	}

	/// Return a new backend with all pending changes.
	///
	/// In contrast to [`commit_all`](Self::commit_all) this will not panic if there are open
	/// transactions.
	pub fn as_backend(&self) -> InMemoryBackend<H> {
		let top: Vec<_> = self
			.overlay
			.changes()
			.map(|(k, v)| (k.clone(), v.value().cloned()))
			.collect();
		let mut transaction = vec![(None, top)];

		for (child_changes, child_info) in self.overlay.children() {
			transaction.push((
				Some(child_info.clone()),
				child_changes
					.map(|(k, v)| (k.clone(), v.value().cloned()))
					.collect(),
			))
		}

		self.backend.update(transaction, self.state_version)
	}

	/// Commit all pending changes to the underlying backend.
	///
	/// # Panic
	///
	/// This will panic if there are still open transactions.
	pub fn commit_all(&mut self) -> Result<(), String> {
		let changes = self
			.overlay
			.drain_storage_changes(&self.backend, self.state_version)?;

		self.backend
			.apply_transaction(changes.transaction_storage_root, changes.transaction);
		Ok(())
	}

	/// Execute the given closure while `self` is set as externalities.
	///
	/// Returns the result of the given closure.
	pub fn execute_with<R>(&mut self, execute: impl FnOnce() -> R) -> R {
		let mut ext = self.ext();
		sp_externalities::set_and_run_with_externalities(&mut ext, execute)
	}

	/// Execute the given closure while `self`, with `proving_backend` as backend, is set as
	/// externalities.
	///
	/// This implementation will wipe the proof recorded in between calls. Consecutive calls will
	/// get their own proof from scratch.
	pub fn execute_and_prove<R>(&mut self, execute: impl FnOnce() -> R) -> (R, StorageProof) {
		let proving_backend = TrieBackendBuilder::wrap(&self.backend)
			.with_recorder(Default::default())
			.build();
		let mut proving_ext = Ext::new(
			&mut self.overlay,
			&proving_backend,
			Some(&mut self.extensions),
		);

		let outcome = sp_externalities::set_and_run_with_externalities(&mut proving_ext, execute);
		let proof = proving_backend
			.extract_proof()
			.expect("Failed to extract storage proof");

		(outcome, proof)
	}

	/// Execute the given closure while `self` is set as externalities.
	///
	/// Returns the result of the given closure, if no panics occurred.
	/// Otherwise, returns `Err`.
	pub fn execute_with_safe<R>(
		&mut self,
		f: impl FnOnce() -> R + UnwindSafe,
	) -> Result<R, String> {
		let mut ext = AssertUnwindSafe(self.ext());
		std::panic::catch_unwind(move || {
			sp_externalities::set_and_run_with_externalities(&mut *ext, f)
		})
		.map_err(|e| format!("Closure panicked: {:?}", e))
	}
}

impl<H: Hasher> std::fmt::Debug for TestExternalities<H>
where
	H::Out: Ord + codec::Codec,
{
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		let pairs: Vec<_> = self
			.backend
			.pairs(Default::default())
			.expect("creating an iterator over all of the pairs doesn't fail in tests")
			.collect();
		write!(f, "overlay: {:?}\nbackend: {:?}", self.overlay, pairs)
	}
}

impl<H: Hasher> PartialEq for TestExternalities<H>
where
	H::Out: Ord + 'static + codec::Codec,
{
	/// This doesn't test if they are in the same state, only if they contains the
	/// same data at this state
	fn eq(&self, other: &TestExternalities<H>) -> bool {
		self.as_backend().eq(&other.as_backend())
	}
}

impl<H: Hasher> Default for TestExternalities<H>
where
	H::Out: Ord + 'static + codec::Codec,
{
	fn default() -> Self {
		// default to default version.
		Self::new_with_state_version(Storage::default(), Default::default())
	}
}

impl<H: Hasher> From<Storage> for TestExternalities<H>
where
	H::Out: Ord + 'static + codec::Codec,
{
	fn from(storage: Storage) -> Self {
		Self::new_with_state_version(storage, Default::default())
	}
}

impl<H: Hasher> From<(Storage, StateVersion)> for TestExternalities<H>
where
	H::Out: Ord + 'static + codec::Codec,
{
	fn from((storage, state_version): (Storage, StateVersion)) -> Self {
		Self::new_with_state_version(storage, state_version)
	}
}

impl<H> sp_externalities::ExtensionStore for TestExternalities<H>
where
	H: Hasher,
	H::Out: Ord + codec::Codec,
{
	fn extension_by_type_id(&mut self, type_id: TypeId) -> Option<&mut dyn Any> {
		self.extensions.get_mut(type_id)
	}

	fn register_extension_with_type_id(
		&mut self,
		type_id: TypeId,
		extension: Box<dyn Extension>,
	) -> Result<(), sp_externalities::Error> {
		self.extensions.register_with_type_id(type_id, extension)
	}

	fn deregister_extension_by_type_id(
		&mut self,
		type_id: TypeId,
	) -> Result<(), sp_externalities::Error> {
		if self.extensions.deregister(type_id) {
			Ok(())
		} else {
			Err(sp_externalities::Error::ExtensionIsNotRegistered(type_id))
		}
	}
}

impl<H> sp_externalities::ExternalitiesExt for TestExternalities<H>
where
	H: Hasher,
	H::Out: Ord + codec::Codec,
{
	fn extension<T: Any + Extension>(&mut self) -> Option<&mut T> {
		self.extension_by_type_id(TypeId::of::<T>())
			.and_then(<dyn Any>::downcast_mut)
	}

	fn register_extension<T: Extension>(&mut self, ext: T) -> Result<(), sp_externalities::Error> {
		self.register_extension_with_type_id(TypeId::of::<T>(), Box::new(ext))
	}

	fn deregister_extension<T: Extension>(&mut self) -> Result<(), sp_externalities::Error> {
		self.deregister_extension_by_type_id(TypeId::of::<T>())
	}
}
