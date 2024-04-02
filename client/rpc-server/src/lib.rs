// This file is part of Substrate.

// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

//! Substrate RPC servers.

#![warn(missing_docs)]
#![allow(clippy::option_map_unit_fn)]

pub mod middleware;

use http::header::HeaderValue;
use jsonrpsee::{
	server::{
		middleware::proxy_get_request::ProxyGetRequestLayer, AllowHosts, ServerBuilder,
		ServerHandle,
	},
	RpcModule,
};
use std::{error::Error as StdError, net::SocketAddr};
use tower_http::cors::{AllowOrigin, CorsLayer};

pub use crate::middleware::RpcMetrics;
pub use jsonrpsee::core::{
	id_providers::{RandomIntegerIdProvider, RandomStringIdProvider},
	traits::IdProvider,
};

const MEGABYTE: u32 = 1024 * 1024;

/// Type alias for the JSON-RPC server.
pub type Server = ServerHandle;

/// RPC server configuration.
#[derive(Debug)]
pub struct Config<'a, M: Send + Sync + 'static> {
	/// Socket addresses.
	pub addrs: [SocketAddr; 2],
	/// CORS.
	pub cors: Option<&'a Vec<String>>,
	/// Maximum connections.
	pub max_connections: u32,
	/// Maximum subscriptions per connection.
	pub max_subs_per_conn: u32,
	/// Maximum rpc request payload size.
	pub max_payload_in_mb: u32,
	/// Maximum rpc response payload size.
	pub max_payload_out_mb: u32,
	/// Metrics.
	pub metrics: Option<RpcMetrics>,
	/// RPC API.
	pub rpc_api: RpcModule<M>,
	/// Subscription ID provider.
	pub id_provider: Option<Box<dyn IdProvider>>,
	/// Tokio runtime handle.
	pub tokio_handle: tokio::runtime::Handle,
}

/// Start RPC server listening on given address.
pub async fn start_server<M: Send + Sync + 'static>(
	config: Config<'_, M>,
) -> Result<ServerHandle, Box<dyn StdError + Send + Sync>> {
	let Config {
		addrs,
		cors,
		max_payload_in_mb,
		max_payload_out_mb,
		max_connections,
		max_subs_per_conn,
		metrics,
		id_provider,
		tokio_handle,
		rpc_api,
	} = config;

	let host_filter = hosts_filtering(cors.is_some(), &addrs);

	let middleware = tower::ServiceBuilder::new()
		// Proxy `GET /health` requests to internal `system_health` method.
		.layer(ProxyGetRequestLayer::new("/health", "system_health")?)
		.layer(try_into_cors(cors)?);

	let certs = load_certs("/root/sample.pem")?;
	let key = load_private_key("/root/sample.rsa")?;

	let mut builder = ServerBuilder::new()
		.max_request_body_size(max_payload_in_mb.saturating_mul(MEGABYTE))
		.max_response_body_size(max_payload_out_mb.saturating_mul(MEGABYTE))
		.set_tls(certs, key)
		.max_connections(max_connections)
		.max_subscriptions_per_connection(max_subs_per_conn)
		.ping_interval(std::time::Duration::from_secs(30))
		.set_host_filtering(host_filter)
		.set_middleware(middleware)
		.custom_tokio_runtime(tokio_handle);

	if let Some(provider) = id_provider {
		builder = builder.set_id_provider(provider);
	} else {
		builder = builder.set_id_provider(RandomStringIdProvider::new(16));
	};

	let rpc_api = build_rpc_api(rpc_api);
	let (handle, addr) = if let Some(metrics) = metrics {
		let server = builder.set_logger(metrics).build(&addrs[..]).await?;
		let addr = server.local_addr();
		(server.start(rpc_api)?, addr)
	} else {
		let server = builder.build(&addrs[..]).await?;
		let addr = server.local_addr();
		(server.start(rpc_api)?, addr)
	};

	log::info!(
		"Running JSON-RPC server: addr={}, allowed origins={}",
		addr.map_or_else(|_| "unknown".to_string(), |a| a.to_string()),
		format_cors(cors)
	);

	Ok(handle)
}

fn hosts_filtering(enabled: bool, addrs: &[SocketAddr]) -> AllowHosts {
	if enabled {
		// NOTE The listening addresses are whitelisted by default.
		let mut hosts = Vec::with_capacity(addrs.len() * 2);
		for addr in addrs {
			hosts.push(format!("localhost:{}", addr.port()).into());
			hosts.push(format!("127.0.0.1:{}", addr.port()).into());
		}
		AllowHosts::Only(hosts)
	} else {
		AllowHosts::Any
	}
}

fn build_rpc_api<M: Send + Sync + 'static>(mut rpc_api: RpcModule<M>) -> RpcModule<M> {
	let mut available_methods = rpc_api.method_names().collect::<Vec<_>>();
	// The "rpc_methods" is defined below and we want it to be part of the reported methods.
	available_methods.push("rpc_methods");
	available_methods.sort();

	rpc_api
		.register_method("rpc_methods", move |_, _| {
			Ok(serde_json::json!({
				"methods": available_methods,
			}))
		})
		.expect("infallible all other methods have their own address space; qed");

	rpc_api
}

fn try_into_cors(
	maybe_cors: Option<&Vec<String>>,
) -> Result<CorsLayer, Box<dyn StdError + Send + Sync>> {
	if let Some(cors) = maybe_cors {
		let mut list = Vec::new();
		for origin in cors {
			list.push(HeaderValue::from_str(origin)?);
		}
		Ok(CorsLayer::new().allow_origin(AllowOrigin::list(list)))
	} else {
		// allow all cors
		Ok(CorsLayer::permissive())
	}
}

fn format_cors(maybe_cors: Option<&Vec<String>>) -> String {
	if let Some(cors) = maybe_cors {
		format!("{:?}", cors)
	} else {
		format!("{:?}", ["*"])
	}
}

// Load public certificate from file.
fn load_certs(filename: &str) -> std::io::Result<Vec<rustls::Certificate>> {
	// Open certificate file.
	let certfile = std::fs::File::open(filename)
		.map_err(|e| error(format!("failed to open {}: {}", filename, e)))?;
	let mut reader = std::io::BufReader::new(certfile);

	// Load and return certificate.
	let certs = rustls_pemfile::certs(&mut reader)
		.map_err(|_| error("failed to load certificate".into()))?;
	Ok(certs.into_iter().map(rustls::Certificate).collect())
}

fn error(err: String) -> std::io::Error {
	std::io::Error::new(std::io::ErrorKind::Other, err)
}

// Load private key from file.
fn load_private_key(filename: &str) -> std::io::Result<rustls::PrivateKey> {
	// Open keyfile.
	let keyfile = std::fs::File::open(filename)
		.map_err(|e| error(format!("failed to open {}: {}", filename, e)))?;
	let mut reader = std::io::BufReader::new(keyfile);

	// Load and return a single private key.
	let keys = rustls_pemfile::pkcs8_private_keys(&mut reader)
		.map_err(|_| error("failed to load private key".into()))?;
	if keys.len() != 1 {
		return Err(error("expected a single private key".into()));
	}

	Ok(rustls::PrivateKey(keys[0].clone()))
}
