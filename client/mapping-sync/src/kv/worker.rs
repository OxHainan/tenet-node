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

use std::{pin::Pin, sync::Arc, time::Duration};

use futures::{
	prelude::*,
	task::{Context, Poll},
};
use futures_timer::Delay;
use log::debug;
// Substrate
use sc_client_api::{
	backend::{Backend, StorageProvider},
	client::ImportNotifications,
};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_consensus::SyncOracle;
use sp_runtime::traits::{Block as BlockT, Header as HeaderT};
// Frontier
use tc_storage::OverrideHandle;
use tp_rpc::EthereumRuntimeRPCApi;

use crate::SyncStrategy;

pub struct MappingSyncWorker<Block: BlockT, C, BE> {
	import_notifications: ImportNotifications<Block>,
	timeout: Duration,
	inner_delay: Option<Delay>,

	client: Arc<C>,
	substrate_backend: Arc<BE>,
	overrides: Arc<OverrideHandle<Block>>,
	frontier_backend: Arc<tc_db::kv::Backend<Block>>,

	have_next: bool,
	retry_times: usize,
	sync_from: <Block::Header as HeaderT>::Number,
	strategy: SyncStrategy,

	sync_oracle: Arc<dyn SyncOracle + Send + Sync + 'static>,
	pubsub_notification_sinks:
		Arc<crate::EthereumBlockNotificationSinks<crate::EthereumBlockNotification<Block>>>,
}

impl<Block: BlockT, C, BE> Unpin for MappingSyncWorker<Block, C, BE> {}

impl<Block: BlockT, C, BE> MappingSyncWorker<Block, C, BE> {
	pub fn new(
		import_notifications: ImportNotifications<Block>,
		timeout: Duration,
		client: Arc<C>,
		substrate_backend: Arc<BE>,
		overrides: Arc<OverrideHandle<Block>>,
		frontier_backend: Arc<tc_db::kv::Backend<Block>>,
		retry_times: usize,
		sync_from: <Block::Header as HeaderT>::Number,
		strategy: SyncStrategy,
		sync_oracle: Arc<dyn SyncOracle + Send + Sync + 'static>,
		pubsub_notification_sinks: Arc<
			crate::EthereumBlockNotificationSinks<crate::EthereumBlockNotification<Block>>,
		>,
	) -> Self {
		Self {
			import_notifications,
			timeout,
			inner_delay: None,

			client,
			substrate_backend,
			overrides,
			frontier_backend,

			have_next: true,
			retry_times,
			sync_from,
			strategy,

			sync_oracle,
			pubsub_notification_sinks,
		}
	}
}

impl<Block: BlockT, C, BE> Stream for MappingSyncWorker<Block, C, BE>
where
	C: ProvideRuntimeApi<Block>,
	C::Api: EthereumRuntimeRPCApi<Block>,
	C: HeaderBackend<Block> + StorageProvider<Block, BE>,
	BE: Backend<Block>,
{
	type Item = ();

	fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<()>> {
		let mut fire = false;

		loop {
			match Stream::poll_next(Pin::new(&mut self.import_notifications), cx) {
				Poll::Pending => break,
				Poll::Ready(Some(_)) => {
					fire = true;
				}
				Poll::Ready(None) => return Poll::Ready(None),
			}
		}

		let timeout = self.timeout;
		let inner_delay = self.inner_delay.get_or_insert_with(|| Delay::new(timeout));

		match Future::poll(Pin::new(inner_delay), cx) {
			Poll::Pending => (),
			Poll::Ready(()) => {
				fire = true;
			}
		}

		if self.have_next {
			fire = true;
		}

		if fire {
			self.inner_delay = None;

			match crate::kv::sync_blocks(
				self.client.as_ref(),
				self.substrate_backend.as_ref(),
				self.overrides.clone(),
				self.frontier_backend.as_ref(),
				self.retry_times,
				self.sync_from,
				self.strategy,
				self.sync_oracle.clone(),
				self.pubsub_notification_sinks.clone(),
			) {
				Ok(have_next) => {
					self.have_next = have_next;
					Poll::Ready(Some(()))
				}
				Err(e) => {
					self.have_next = false;
					debug!(target: "mapping-sync", "Syncing failed with error {:?}, retrying.", e);
					Poll::Ready(Some(()))
				}
			}
		} else {
			Poll::Pending
		}
	}
}
