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

use std::{ops::DerefMut, sync::Arc, time::Duration};

use futures::prelude::*;
// Substrate
use sc_client_api::backend::{Backend as BackendT, StateBackend, StorageProvider};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::{Backend, HeaderBackend};
use sp_consensus::SyncOracle;
use sp_core::H256;
use sp_runtime::traits::{BlakeTwo256, Block as BlockT, Header as HeaderT, UniqueSaturatedInto};
// Frontier
use tp_rpc::EthereumRuntimeRPCApi;

use crate::{EthereumBlockNotification, EthereumBlockNotificationSinks, SyncStrategy};

/// Defines the commands for the sync worker.
#[derive(Debug)]
pub enum WorkerCommand {
	/// Resume indexing from the last indexed canon block.
	ResumeSync,
	/// Index leaves.
	IndexLeaves(Vec<H256>),
	/// Index the best block known so far via import notifications.
	IndexBestBlock(H256),
	/// Canonicalize the enacted and retracted blocks reported via import notifications.
	Canonicalize {
		common: H256,
		enacted: Vec<H256>,
		retracted: Vec<H256>,
	},
	/// Verify indexed blocks' consistency.
	/// Check for any canon blocks that haven't had their logs indexed.
	/// Check for any missing parent blocks from the latest canon block.
	CheckIndexedBlocks,
}

/// Config parameters for the SyncWorker.
pub struct SyncWorkerConfig {
	pub check_indexed_blocks_interval: Duration,
	pub read_notification_timeout: Duration,
}

/// Implements an indexer that imports blocks and their transactions.
pub struct SyncWorker<Block, Backend, Client> {
	_phantom: std::marker::PhantomData<(Block, Backend, Client)>,
}

impl<Block: BlockT, Backend, Client> SyncWorker<Block, Backend, Client>
where
	Block: BlockT<Hash = H256>,
	Client: ProvideRuntimeApi<Block>,
	Client::Api: EthereumRuntimeRPCApi<Block>,
	Client: HeaderBackend<Block> + StorageProvider<Block, Backend> + 'static,
	Backend: BackendT<Block> + 'static,
	Backend::State: StateBackend<BlakeTwo256>,
{
	/// Spawn the indexing worker. The worker can be given commands via the sender channel.
	/// Once the buffer is full, attempts to send new messages will wait until a message is read from the channel.
	pub async fn spawn_worker(
		client: Arc<Client>,
		substrate_backend: Arc<Backend>,
		indexer_backend: Arc<fc_db::sql::Backend<Block>>,
		pubsub_notification_sinks: Arc<
			EthereumBlockNotificationSinks<EthereumBlockNotification<Block>>,
		>,
	) -> tokio::sync::mpsc::Sender<WorkerCommand> {
		let (tx, mut rx) = tokio::sync::mpsc::channel(100);
		tokio::task::spawn(async move {
			while let Some(cmd) = rx.recv().await {
				log::debug!(target: "frontier-sql", "💬 Recv Worker Command {cmd:?}");
				match cmd {
					WorkerCommand::ResumeSync => {
						// Attempt to resume from last indexed block. If there is no data in the db, sync genesis.
						match indexer_backend.get_last_indexed_canon_block().await.ok() {
							Some(last_block_hash) => {
								log::debug!(target: "frontier-sql", "Resume from last block {last_block_hash:?}");
								if let Some(parent_hash) = client
									.header(last_block_hash)
									.ok()
									.flatten()
									.map(|header| *header.parent_hash())
								{
									index_canonical_block_and_ancestors(
										client.clone(),
										substrate_backend.clone(),
										indexer_backend.clone(),
										parent_hash,
									)
									.await;
								}
							}
							None => {
								index_genesis_block(client.clone(), indexer_backend.clone()).await;
							}
						};
					}
					WorkerCommand::IndexLeaves(leaves) => {
						for leaf in leaves {
							index_block_and_ancestors(
								client.clone(),
								substrate_backend.clone(),
								indexer_backend.clone(),
								leaf,
							)
							.await;
						}
					}
					WorkerCommand::IndexBestBlock(block_hash) => {
						index_canonical_block_and_ancestors(
							client.clone(),
							substrate_backend.clone(),
							indexer_backend.clone(),
							block_hash,
						)
						.await;
						let sinks = &mut pubsub_notification_sinks.lock();
						for sink in sinks.iter() {
							let _ = sink.unbounded_send(EthereumBlockNotification {
								is_new_best: true,
								hash: block_hash,
							});
						}
					}
					WorkerCommand::Canonicalize {
						common,
						enacted,
						retracted,
					} => {
						canonicalize_blocks(indexer_backend.clone(), common, enacted, retracted)
							.await;
					}
					WorkerCommand::CheckIndexedBlocks => {
						// Fix any indexed blocks that did not have their logs indexed
						if let Some(block_hash) =
							indexer_backend.get_first_pending_canon_block().await
						{
							log::debug!(target: "frontier-sql", "Indexing pending canonical block {block_hash:?}");
							indexer_backend
								.index_block_logs(client.clone(), block_hash)
								.await;
						}

						// Fix any missing blocks
						index_missing_blocks(
							client.clone(),
							substrate_backend.clone(),
							indexer_backend.clone(),
						)
						.await;
					}
				}
			}
		});

		tx
	}

	/// Start the worker.
	pub async fn run(
		client: Arc<Client>,
		substrate_backend: Arc<Backend>,
		indexer_backend: Arc<fc_db::sql::Backend<Block>>,
		import_notifications: sc_client_api::ImportNotifications<Block>,
		worker_config: SyncWorkerConfig,
		_sync_strategy: SyncStrategy,
		sync_oracle: Arc<dyn SyncOracle + Send + Sync + 'static>,
		pubsub_notification_sinks: Arc<
			EthereumBlockNotificationSinks<EthereumBlockNotification<Block>>,
		>,
	) {
		let tx = Self::spawn_worker(
			client.clone(),
			substrate_backend.clone(),
			indexer_backend.clone(),
			pubsub_notification_sinks.clone(),
		)
		.await;

		// Resume sync from the last indexed block until we reach an already indexed parent
		tx.send(WorkerCommand::ResumeSync).await.ok();
		// check missing blocks every interval
		let tx2 = tx.clone();
		tokio::task::spawn(async move {
			loop {
				futures_timer::Delay::new(worker_config.check_indexed_blocks_interval).await;
				tx2.send(WorkerCommand::CheckIndexedBlocks).await.ok();
			}
		});

		// check notifications
		let mut notifications = import_notifications.fuse();
		loop {
			let mut timeout =
				futures_timer::Delay::new(worker_config.read_notification_timeout).fuse();
			futures::select! {
				_ = timeout => {
					if let Ok(leaves) = substrate_backend.blockchain().leaves() {
						tx.send(WorkerCommand::IndexLeaves(leaves)).await.ok();
					}
					if sync_oracle.is_major_syncing() {
						let sinks = &mut pubsub_notification_sinks.lock();
						if !sinks.is_empty() {
							*sinks.deref_mut() = vec![];
						}
					}
				}
				notification = notifications.next() => if let Some(notification) = notification {
					log::debug!(
						target: "frontier-sql",
						"📣  New notification: #{} {:?} (parent {}), best = {}",
						notification.header.number(),
						notification.hash,
						notification.header.parent_hash(),
						notification.is_new_best,
					);
					if notification.is_new_best {
						if let Some(tree_route) = notification.tree_route {
							log::debug!(
								target: "frontier-sql",
								"🔀  Re-org happened at new best {}, proceeding to canonicalize db",
								notification.hash
							);
							let retracted = tree_route
								.retracted()
								.iter()
								.map(|hash_and_number| hash_and_number.hash)
								.collect::<Vec<_>>();
							let enacted = tree_route
								.enacted()
								.iter()
								.map(|hash_and_number| hash_and_number.hash)
								.collect::<Vec<_>>();

							let common = tree_route.common_block().hash;
							tx.send(WorkerCommand::Canonicalize {
								common,
								enacted,
								retracted,
							}).await.ok();
						}

						tx.send(WorkerCommand::IndexBestBlock(notification.hash)).await.ok();
					}
				}
			}
		}
	}
}

/// Index the provided blocks. The function loops over the ancestors of the provided nodes
/// until it encounters the genesis block, or a block that has already been imported, or
/// is already in the active set. The `hashes` parameter is populated with any parent blocks
/// that is scheduled to be indexed.
async fn index_block_and_ancestors<Block, Backend, Client>(
	client: Arc<Client>,
	substrate_backend: Arc<Backend>,
	indexer_backend: Arc<fc_db::sql::Backend<Block>>,
	hash: H256,
) where
	Block: BlockT<Hash = H256>,
	Client: ProvideRuntimeApi<Block>,
	Client::Api: EthereumRuntimeRPCApi<Block>,
	Client: HeaderBackend<Block> + StorageProvider<Block, Backend> + 'static,
	Backend: BackendT<Block> + 'static,
	Backend::State: StateBackend<BlakeTwo256>,
{
	let blockchain_backend = substrate_backend.blockchain();
	let mut hashes = vec![hash];
	while let Some(hash) = hashes.pop() {
		// exit if genesis block is reached
		if hash == H256::default() {
			break;
		}

		// exit if block is already imported
		if indexer_backend.is_block_indexed(hash).await {
			log::debug!(target: "frontier-sql", "🔴 Block {hash:?} already imported");
			break;
		}

		log::debug!(target: "frontier-sql", "🛠️  Importing {hash:?}");
		let _ = indexer_backend
			.insert_block_metadata(client.clone(), hash)
			.await
			.map_err(|e| {
				log::error!(target: "frontier-sql", "{e}");
			});
		log::debug!(target: "frontier-sql", "Inserted block metadata");
		indexer_backend.index_block_logs(client.clone(), hash).await;

		if let Ok(Some(header)) = blockchain_backend.header(hash) {
			let parent_hash = header.parent_hash();
			hashes.push(*parent_hash);
		}
	}
}

/// Index the provided known canonical blocks. The function loops over the ancestors of the provided nodes
/// until it encounters the genesis block, or a block that has already been imported, or
/// is already in the active set. The `hashes` parameter is populated with any parent blocks
/// that is scheduled to be indexed.
async fn index_canonical_block_and_ancestors<Block, Backend, Client>(
	client: Arc<Client>,
	substrate_backend: Arc<Backend>,
	indexer_backend: Arc<fc_db::sql::Backend<Block>>,
	hash: H256,
) where
	Block: BlockT<Hash = H256>,
	Client: ProvideRuntimeApi<Block>,
	Client::Api: EthereumRuntimeRPCApi<Block>,
	Client: HeaderBackend<Block> + StorageProvider<Block, Backend> + 'static,
	Backend: BackendT<Block> + 'static,
	Backend::State: StateBackend<BlakeTwo256>,
{
	let blockchain_backend = substrate_backend.blockchain();
	let mut hashes = vec![hash];
	while let Some(hash) = hashes.pop() {
		// exit if genesis block is reached
		if hash == H256::default() {
			break;
		}

		let status = indexer_backend.block_indexed_and_canon_status(hash).await;

		// exit if canonical block is already imported
		if status.indexed && status.canon {
			log::debug!(target: "frontier-sql", "🔴 Block {hash:?} already imported");
			break;
		}

		// If block was previously indexed as non-canon then mark it as canon
		if status.indexed && !status.canon {
			if let Err(err) = indexer_backend.set_block_as_canon(hash).await {
				log::error!(target: "frontier-sql", "Failed setting block {hash:?} as canon: {err:?}");
				continue;
			}

			log::debug!(target: "frontier-sql", "🛠️  Marked block as canon {hash:?}");

			// Check parent block
			if let Ok(Some(header)) = blockchain_backend.header(hash) {
				let parent_hash = header.parent_hash();
				hashes.push(*parent_hash);
			}
			continue;
		}

		// Else, import the new block
		log::debug!(target: "frontier-sql", "🛠️  Importing {hash:?}");
		let _ = indexer_backend
			.insert_block_metadata(client.clone(), hash)
			.await
			.map_err(|e| {
				log::error!(target: "frontier-sql", "{e}");
			});
		log::debug!(target: "frontier-sql", "Inserted block metadata  {hash:?}");
		indexer_backend.index_block_logs(client.clone(), hash).await;

		if let Ok(Some(header)) = blockchain_backend.header(hash) {
			let parent_hash = header.parent_hash();
			hashes.push(*parent_hash);
		}
	}
}

/// Canonicalizes the database by setting the `is_canon` field for the retracted blocks to `0`,
/// and `1` if they are enacted.
async fn canonicalize_blocks<Block: BlockT<Hash = H256>>(
	indexer_backend: Arc<fc_db::sql::Backend<Block>>,
	common: H256,
	enacted: Vec<H256>,
	retracted: Vec<H256>,
) {
	if (indexer_backend.canonicalize(&retracted, &enacted).await).is_err() {
		log::error!(
			target: "frontier-sql",
			"❌  Canonicalization failed for common ancestor {}, potentially corrupted db. Retracted: {:?}, Enacted: {:?}",
			common,
			retracted,
			enacted,
		);
	}
}

/// Attempts to index any missing blocks that are in the past. This fixes any gaps that may
/// be present in the indexing strategy, since the indexer only walks the parent hashes until
/// it finds the first ancestor that has already been indexed.
async fn index_missing_blocks<Block, Client, Backend>(
	client: Arc<Client>,
	substrate_backend: Arc<Backend>,
	indexer_backend: Arc<fc_db::sql::Backend<Block>>,
) where
	Block: BlockT<Hash = H256>,
	Client: ProvideRuntimeApi<Block>,
	Client::Api: EthereumRuntimeRPCApi<Block>,
	Client: HeaderBackend<Block> + StorageProvider<Block, Backend> + 'static,
	Backend: BackendT<Block> + 'static,
	Backend::State: StateBackend<BlakeTwo256>,
{
	if let Some(block_number) = indexer_backend.get_first_missing_canon_block().await {
		log::debug!(target: "frontier-sql", "Missing {block_number:?}");
		if block_number == 0 {
			index_genesis_block(client.clone(), indexer_backend.clone()).await;
		} else if let Ok(Some(block_hash)) = client.hash(block_number.unique_saturated_into()) {
			log::debug!(
				target: "frontier-sql",
				"Indexing past canonical blocks from #{} {:?}",
				block_number,
				block_hash,
			);
			index_canonical_block_and_ancestors(
				client.clone(),
				substrate_backend.clone(),
				indexer_backend.clone(),
				block_hash,
			)
			.await;
		} else {
			log::debug!(target: "frontier-sql", "Failed retrieving hash for block #{block_number}");
		}
	}
}

/// Attempts to index any missing blocks that are in the past. This fixes any gaps that may
/// be present in the indexing strategy, since the indexer only walks the parent hashes until
/// it finds the first ancestor that has already been indexed.
async fn index_genesis_block<Block, Client, Backend>(
	client: Arc<Client>,
	indexer_backend: Arc<fc_db::sql::Backend<Block>>,
) where
	Block: BlockT<Hash = H256>,
	Client: ProvideRuntimeApi<Block>,
	Client::Api: EthereumRuntimeRPCApi<Block>,
	Client: HeaderBackend<Block> + StorageProvider<Block, Backend> + 'static,
	Backend: BackendT<Block> + 'static,
	Backend::State: StateBackend<BlakeTwo256>,
{
	log::info!(
		target: "frontier-sql",
		"Import genesis",
	);
	if let Ok(Some(substrate_genesis_hash)) = indexer_backend
		.insert_genesis_block_metadata(client.clone())
		.await
		.map_err(|e| {
			log::error!(target: "frontier-sql", "💔  Cannot sync genesis block: {e}");
		}) {
		log::debug!(target: "frontier-sql", "Imported genesis block {substrate_genesis_hash:?}");
	}
}
