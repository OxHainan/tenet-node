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

//! Trie-based state machine backend.

#[cfg(feature = "std")]
use crate::backend::AsTrieBackend;
use crate::{
	backend::{IterArgs, StorageIterator},
	trie_backend_essence::{RawIter, TrieBackendEssence, TrieBackendStorage},
	Backend, StorageKey, StorageValue,
};

use codec::Codec;
#[cfg(feature = "std")]
use hash_db::HashDB;
use hash_db::Hasher;
use sp_core::storage::{ChildInfo, StateVersion};
#[cfg(feature = "std")]
use sp_trie::{
	cache::{LocalTrieCache, TrieCache},
	recorder::Recorder,
	MemoryDB, StorageProof,
};
#[cfg(not(feature = "std"))]
use sp_trie::{Error, NodeCodec};
use sp_trie::{MerkleValue, PrefixedMemoryDB};
use trie_db::TrieCache as TrieCacheT;
#[cfg(not(feature = "std"))]
use trie_db::{node::NodeOwned, CachedValue};

/// A provider of trie caches that are compatible with [`trie_db::TrieDB`].
pub trait TrieCacheProvider<H: Hasher> {
	/// Cache type that implements [`trie_db::TrieCache`].
	type Cache<'a>: TrieCacheT<sp_trie::NodeCodec<H>> + 'a
	where
		Self: 'a;

	/// Return a [`trie_db::TrieDB`] compatible cache.
	///
	/// The `storage_root` parameter should be the storage root of the used trie.
	fn as_trie_db_cache(&self, storage_root: H::Out) -> Self::Cache<'_>;

	/// Returns a cache that can be used with a [`trie_db::TrieDBMut`].
	///
	/// When finished with the operation on the trie, it is required to call [`Self::merge`] to
	/// merge the cached items for the correct `storage_root`.
	fn as_trie_db_mut_cache(&self) -> Self::Cache<'_>;

	/// Merge the cached data in `other` into the provider using the given `new_root`.
	///
	/// This must be used for the cache returned by [`Self::as_trie_db_mut_cache`] as otherwise the
	/// cached data is just thrown away.
	fn merge<'a>(&'a self, other: Self::Cache<'a>, new_root: H::Out);
}

#[cfg(feature = "std")]
impl<H: Hasher> TrieCacheProvider<H> for LocalTrieCache<H> {
	type Cache<'a> = TrieCache<'a, H> where H: 'a;

	fn as_trie_db_cache(&self, storage_root: H::Out) -> Self::Cache<'_> {
		self.as_trie_db_cache(storage_root)
	}

	fn as_trie_db_mut_cache(&self) -> Self::Cache<'_> {
		self.as_trie_db_mut_cache()
	}

	fn merge<'a>(&'a self, other: Self::Cache<'a>, new_root: H::Out) {
		other.merge_into(self, new_root)
	}
}

#[cfg(feature = "std")]
impl<H: Hasher> TrieCacheProvider<H> for &LocalTrieCache<H> {
	type Cache<'a> = TrieCache<'a, H> where Self: 'a;

	fn as_trie_db_cache(&self, storage_root: H::Out) -> Self::Cache<'_> {
		(*self).as_trie_db_cache(storage_root)
	}

	fn as_trie_db_mut_cache(&self) -> Self::Cache<'_> {
		(*self).as_trie_db_mut_cache()
	}

	fn merge<'a>(&'a self, other: Self::Cache<'a>, new_root: H::Out) {
		other.merge_into(self, new_root)
	}
}

/// Cache provider that allows construction of a [`TrieBackend`] and satisfies the requirements, but
/// can never be instantiated.
#[cfg(not(feature = "std"))]
pub struct UnimplementedCacheProvider<H> {
	// Not strictly necessary, but the H bound allows to use this as a drop-in
	// replacement for the `LocalTrieCache` in no-std contexts.
	_phantom: core::marker::PhantomData<H>,
	// Statically prevents construction.
	_infallible: core::convert::Infallible,
}

#[cfg(not(feature = "std"))]
impl<H: Hasher> trie_db::TrieCache<NodeCodec<H>> for UnimplementedCacheProvider<H> {
	fn lookup_value_for_key(&mut self, _key: &[u8]) -> Option<&CachedValue<H::Out>> {
		unimplemented!()
	}

	fn cache_value_for_key(&mut self, _key: &[u8], _value: CachedValue<H::Out>) {
		unimplemented!()
	}

	fn get_or_insert_node(
		&mut self,
		_hash: H::Out,
		_fetch_node: &mut dyn FnMut() -> trie_db::Result<NodeOwned<H::Out>, H::Out, Error<H::Out>>,
	) -> trie_db::Result<&NodeOwned<H::Out>, H::Out, Error<H::Out>> {
		unimplemented!()
	}

	fn get_node(&mut self, _hash: &H::Out) -> Option<&NodeOwned<H::Out>> {
		unimplemented!()
	}
}

#[cfg(not(feature = "std"))]
impl<H: Hasher> TrieCacheProvider<H> for UnimplementedCacheProvider<H> {
	type Cache<'a> = UnimplementedCacheProvider<H> where H: 'a;

	fn as_trie_db_cache(&self, _storage_root: <H as Hasher>::Out) -> Self::Cache<'_> {
		unimplemented!()
	}

	fn as_trie_db_mut_cache(&self) -> Self::Cache<'_> {
		unimplemented!()
	}

	fn merge<'a>(&'a self, _other: Self::Cache<'a>, _new_root: <H as Hasher>::Out) {
		unimplemented!()
	}
}

#[cfg(feature = "std")]
type DefaultCache<H> = LocalTrieCache<H>;

#[cfg(not(feature = "std"))]
type DefaultCache<H> = UnimplementedCacheProvider<H>;

/// Builder for creating a [`TrieBackend`].
pub struct TrieBackendBuilder<S: TrieBackendStorage<H>, H: Hasher, C = DefaultCache<H>> {
	storage: S,
	root: H::Out,
	#[cfg(feature = "std")]
	recorder: Option<Recorder<H>>,
	cache: Option<C>,
}

impl<S, H> TrieBackendBuilder<S, H, DefaultCache<H>>
where
	S: TrieBackendStorage<H>,
	H: Hasher,
{
	/// Create a new builder instance.
	pub fn new(storage: S, root: H::Out) -> Self {
		Self {
			storage,
			root,
			#[cfg(feature = "std")]
			recorder: None,
			cache: None,
		}
	}
}

impl<S, H, C> TrieBackendBuilder<S, H, C>
where
	S: TrieBackendStorage<H>,
	H: Hasher,
{
	/// Create a new builder instance.
	pub fn new_with_cache(storage: S, root: H::Out, cache: C) -> Self {
		Self {
			storage,
			root,
			#[cfg(feature = "std")]
			recorder: None,
			cache: Some(cache),
		}
	}
	/// Wrap the given [`TrieBackend`].
	///
	/// This can be used for example if all accesses to the trie should
	/// be recorded while some other functionality still uses the non-recording
	/// backend.
	///
	/// The backend storage and the cache will be taken from `other`.
	pub fn wrap(other: &TrieBackend<S, H, C>) -> TrieBackendBuilder<&S, H, &C> {
		TrieBackendBuilder {
			storage: other.essence.backend_storage(),
			root: *other.essence.root(),
			#[cfg(feature = "std")]
			recorder: None,
			cache: other.essence.trie_node_cache.as_ref(),
		}
	}

	/// Use the given optional `recorder` for the to be configured [`TrieBackend`].
	#[cfg(feature = "std")]
	pub fn with_optional_recorder(self, recorder: Option<Recorder<H>>) -> Self {
		Self { recorder, ..self }
	}

	/// Use the given `recorder` for the to be configured [`TrieBackend`].
	#[cfg(feature = "std")]
	pub fn with_recorder(self, recorder: Recorder<H>) -> Self {
		Self {
			recorder: Some(recorder),
			..self
		}
	}

	/// Use the given optional `cache` for the to be configured [`TrieBackend`].
	pub fn with_optional_cache<LC>(self, cache: Option<LC>) -> TrieBackendBuilder<S, H, LC> {
		TrieBackendBuilder {
			cache,
			root: self.root,
			storage: self.storage,
			#[cfg(feature = "std")]
			recorder: self.recorder,
		}
	}

	/// Use the given `cache` for the to be configured [`TrieBackend`].
	pub fn with_cache<LC>(self, cache: LC) -> TrieBackendBuilder<S, H, LC> {
		TrieBackendBuilder {
			cache: Some(cache),
			root: self.root,
			storage: self.storage,
			#[cfg(feature = "std")]
			recorder: self.recorder,
		}
	}

	/// Build the configured [`TrieBackend`].
	#[cfg(feature = "std")]
	pub fn build(self) -> TrieBackend<S, H, C> {
		TrieBackend {
			essence: TrieBackendEssence::new_with_cache_and_recorder(
				self.storage,
				self.root,
				self.cache,
				self.recorder,
			),
			next_storage_key_cache: Default::default(),
		}
	}

	/// Build the configured [`TrieBackend`].
	#[cfg(not(feature = "std"))]
	pub fn build(self) -> TrieBackend<S, H, C> {
		TrieBackend {
			essence: TrieBackendEssence::new_with_cache(self.storage, self.root, self.cache),
			next_storage_key_cache: Default::default(),
		}
	}
}

/// A cached iterator.
struct CachedIter<S, H, C>
where
	H: Hasher,
{
	last_key: sp_std::vec::Vec<u8>,
	iter: RawIter<S, H, C>,
}

impl<S, H, C> Default for CachedIter<S, H, C>
where
	H: Hasher,
{
	fn default() -> Self {
		Self {
			last_key: Default::default(),
			iter: Default::default(),
		}
	}
}

#[cfg(feature = "std")]
type CacheCell<T> = parking_lot::Mutex<T>;

#[cfg(not(feature = "std"))]
type CacheCell<T> = core::cell::RefCell<T>;

#[cfg(feature = "std")]
fn access_cache<T, R>(cell: &CacheCell<T>, callback: impl FnOnce(&mut T) -> R) -> R {
	callback(&mut *cell.lock())
}

#[cfg(not(feature = "std"))]
fn access_cache<T, R>(cell: &CacheCell<T>, callback: impl FnOnce(&mut T) -> R) -> R {
	callback(&mut *cell.borrow_mut())
}

/// Patricia trie-based backend. Transaction type is an overlay of changes to commit.
pub struct TrieBackend<S: TrieBackendStorage<H>, H: Hasher, C = DefaultCache<H>> {
	pub(crate) essence: TrieBackendEssence<S, H, C>,
	next_storage_key_cache: CacheCell<Option<CachedIter<S, H, C>>>,
}

impl<S: TrieBackendStorage<H>, H: Hasher, C: TrieCacheProvider<H> + Send + Sync>
	TrieBackend<S, H, C>
where
	H::Out: Codec,
{
	#[cfg(test)]
	#[allow(dead_code)]
	pub(crate) fn from_essence(essence: TrieBackendEssence<S, H, C>) -> Self {
		Self {
			essence,
			next_storage_key_cache: Default::default(),
		}
	}

	/// Get backend essence reference.
	pub fn essence(&self) -> &TrieBackendEssence<S, H, C> {
		&self.essence
	}

	/// Get backend storage reference.
	pub fn backend_storage_mut(&mut self) -> &mut S {
		self.essence.backend_storage_mut()
	}

	/// Get backend storage reference.
	pub fn backend_storage(&self) -> &S {
		self.essence.backend_storage()
	}

	/// Set trie root.
	pub fn set_root(&mut self, root: H::Out) {
		self.essence.set_root(root)
	}

	/// Get trie root.
	pub fn root(&self) -> &H::Out {
		self.essence.root()
	}

	/// Consumes self and returns underlying storage.
	pub fn into_storage(self) -> S {
		self.essence.into_storage()
	}

	/// Extract the [`StorageProof`].
	///
	/// This only returns `Some` when there was a recorder set.
	#[cfg(feature = "std")]
	pub fn extract_proof(mut self) -> Option<StorageProof> {
		self.essence
			.recorder
			.take()
			.map(|r| r.drain_storage_proof())
	}
}

impl<S: TrieBackendStorage<H>, H: Hasher, C: TrieCacheProvider<H>> sp_std::fmt::Debug
	for TrieBackend<S, H, C>
{
	fn fmt(&self, f: &mut sp_std::fmt::Formatter<'_>) -> sp_std::fmt::Result {
		write!(f, "TrieBackend")
	}
}

impl<S: TrieBackendStorage<H>, H: Hasher, C: TrieCacheProvider<H> + Send + Sync> Backend<H>
	for TrieBackend<S, H, C>
where
	H::Out: Ord + Codec,
{
	type Error = crate::DefaultError;
	type TrieBackendStorage = S;
	type RawIter = crate::trie_backend_essence::RawIter<S, H, C>;

	fn storage_hash(&self, key: &[u8]) -> Result<Option<H::Out>, Self::Error> {
		self.essence.storage_hash(key)
	}

	fn storage(&self, key: &[u8]) -> Result<Option<StorageValue>, Self::Error> {
		self.essence.storage(key)
	}

	fn child_storage_hash(
		&self,
		child_info: &ChildInfo,
		key: &[u8],
	) -> Result<Option<H::Out>, Self::Error> {
		self.essence.child_storage_hash(child_info, key)
	}

	fn child_storage(
		&self,
		child_info: &ChildInfo,
		key: &[u8],
	) -> Result<Option<StorageValue>, Self::Error> {
		self.essence.child_storage(child_info, key)
	}

	fn closest_merkle_value(&self, key: &[u8]) -> Result<Option<MerkleValue<H::Out>>, Self::Error> {
		self.essence.closest_merkle_value(key)
	}

	fn child_closest_merkle_value(
		&self,
		child_info: &ChildInfo,
		key: &[u8],
	) -> Result<Option<MerkleValue<H::Out>>, Self::Error> {
		self.essence.child_closest_merkle_value(child_info, key)
	}

	fn next_storage_key(&self, key: &[u8]) -> Result<Option<StorageKey>, Self::Error> {
		let (is_cached, mut cache) = access_cache(&self.next_storage_key_cache, Option::take)
			.map(|cache| (cache.last_key == key, cache))
			.unwrap_or_default();

		if !is_cached {
			cache.iter = self.raw_iter(IterArgs {
				start_at: Some(key),
				start_at_exclusive: true,
				..IterArgs::default()
			})?
		};

		let next_key = match cache.iter.next_key(self) {
			None => return Ok(None),
			Some(Err(error)) => return Err(error),
			Some(Ok(next_key)) => next_key,
		};

		cache.last_key.clear();
		cache.last_key.extend_from_slice(&next_key);
		access_cache(&self.next_storage_key_cache, |cache_cell| {
			cache_cell.replace(cache)
		});

		#[cfg(debug_assertions)]
		debug_assert_eq!(
			self.essence
				.next_storage_key_slow(key)
				.expect(
					"fetching the next key through iterator didn't fail so this shouldn't either"
				)
				.as_ref(),
			Some(&next_key)
		);

		Ok(Some(next_key))
	}

	fn next_child_storage_key(
		&self,
		child_info: &ChildInfo,
		key: &[u8],
	) -> Result<Option<StorageKey>, Self::Error> {
		self.essence.next_child_storage_key(child_info, key)
	}

	fn raw_iter(&self, args: IterArgs) -> Result<Self::RawIter, Self::Error> {
		self.essence.raw_iter(args)
	}

	fn storage_root<'a>(
		&self,
		delta: impl Iterator<Item = (&'a [u8], Option<&'a [u8]>)>,
		state_version: StateVersion,
	) -> (H::Out, PrefixedMemoryDB<H>)
	where
		H::Out: Ord,
	{
		self.essence.storage_root(delta, state_version)
	}

	fn child_storage_root<'a>(
		&self,
		child_info: &ChildInfo,
		delta: impl Iterator<Item = (&'a [u8], Option<&'a [u8]>)>,
		state_version: StateVersion,
	) -> (H::Out, bool, PrefixedMemoryDB<H>)
	where
		H::Out: Ord,
	{
		self.essence
			.child_storage_root(child_info, delta, state_version)
	}

	fn register_overlay_stats(&self, _stats: &crate::stats::StateMachineStats) {}

	fn usage_info(&self) -> crate::UsageInfo {
		crate::UsageInfo::empty()
	}

	fn wipe(&self) -> Result<(), Self::Error> {
		Ok(())
	}
}

#[cfg(feature = "std")]
impl<S: TrieBackendStorage<H>, H: Hasher, C> AsTrieBackend<H, C> for TrieBackend<S, H, C> {
	type TrieBackendStorage = S;

	fn as_trie_backend(&self) -> &TrieBackend<S, H, C> {
		self
	}
}

/// Create a backend used for checking the proof, using `H` as hasher.
///
/// `proof` and `root` must match, i.e. `root` must be the correct root of `proof` nodes.
#[cfg(feature = "std")]
pub fn create_proof_check_backend<H>(
	root: H::Out,
	proof: StorageProof,
) -> Result<TrieBackend<MemoryDB<H>, H>, Box<dyn crate::Error>>
where
	H: Hasher,
	H::Out: Codec,
{
	let db = proof.into_memory_db();

	if db.contains(&root, hash_db::EMPTY_PREFIX) {
		Ok(TrieBackendBuilder::new(db, root).build())
	} else {
		Err(Box::new(crate::ExecutionError::InvalidProof))
	}
}
