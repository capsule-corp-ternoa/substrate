// This file is part of Substrate.

// Copyright (C) 2017-2021 Parity Technologies (UK) Ltd.
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

//! Substrate block-author/full-node API.

#[cfg(test)]
mod tests;

use std::{convert::TryInto, sync::Arc};

use crate::SubscriptionTaskExecutor;

use codec::{Decode, Encode};
use futures::StreamExt;
use jsonrpsee::{
	types::{async_trait, error::Error as JsonRpseeError, RpcResult},
	SubscriptionSink,
};
use sc_rpc_api::DenyUnsafe;
use sc_transaction_pool_api::{
	error::IntoPoolError, BlockHash, InPoolTransaction, TransactionFor, TransactionPool,
	TransactionSource, TxHash,
};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_core::Bytes;
use sp_keystore::{SyncCryptoStore, SyncCryptoStorePtr};
use sp_runtime::{generic, traits::Block as BlockT};
use sp_session::SessionKeys;

use self::error::{Error, Result};
/// Re-export the API for backward compatibility.
pub use sc_rpc_api::author::*;

/// Authoring API
pub struct Author<P, Client> {
	/// Substrate client
	client: Arc<Client>,
	/// Transactions pool
	pool: Arc<P>,
	/// The key store.
	keystore: SyncCryptoStorePtr,
	/// Whether to deny unsafe calls
	deny_unsafe: DenyUnsafe,
	/// Executor to spawn subscriptions.
	executor: SubscriptionTaskExecutor,
}

impl<P, Client> Author<P, Client> {
	/// Create new instance of Authoring API.
	pub fn new(
		client: Arc<Client>,
		pool: Arc<P>,
		keystore: SyncCryptoStorePtr,
		deny_unsafe: DenyUnsafe,
		executor: SubscriptionTaskExecutor,
	) -> Self {
		Author { client, pool, keystore, deny_unsafe, executor }
	}
}

/// Currently we treat all RPC transactions as externals.
///
/// Possibly in the future we could allow opt-in for special treatment
/// of such transactions, so that the block authors can inject
/// some unique transactions via RPC and have them included in the pool.
const TX_SOURCE: TransactionSource = TransactionSource::External;

#[async_trait]
impl<P, Client> AuthorApiServer<TxHash<P>, BlockHash<P>> for Author<P, Client>
where
	P: TransactionPool + Sync + Send + 'static,
	Client: HeaderBackend<P::Block> + ProvideRuntimeApi<P::Block> + Send + Sync + 'static,
	Client::Api: SessionKeys<P::Block>,
	P::Hash: Unpin,
	<P::Block as BlockT>::Hash: Unpin,
{
	async fn submit_extrinsic(&self, ext: Bytes) -> RpcResult<TxHash<P>> {
		let xt = match Decode::decode(&mut &ext[..]) {
			Ok(xt) => xt,
			Err(err) => return Err(JsonRpseeError::to_call_error(err)),
		};
		let best_block_hash = self.client.info().best_hash;
		self.pool
			.submit_one(&generic::BlockId::hash(best_block_hash), TX_SOURCE, xt)
			.await
			.map_err(|e| {
				e.into_pool_error()
					.map(|e| JsonRpseeError::to_call_error(e))
					.unwrap_or_else(|e| JsonRpseeError::to_call_error(e))
			})
	}

	fn insert_key(&self, key_type: String, suri: String, public: Bytes) -> RpcResult<()> {
		self.deny_unsafe.check_if_safe()?;

		let key_type = key_type.as_str().try_into().map_err(|_| Error::BadKeyType)?;
		SyncCryptoStore::insert_unknown(&*self.keystore, key_type, &suri, &public[..])
			.map_err(|_| Error::KeyStoreUnavailable)?;
		Ok(())
	}

	fn rotate_keys(&self) -> RpcResult<Bytes> {
		self.deny_unsafe.check_if_safe()?;

		let best_block_hash = self.client.info().best_hash;
		self.client
			.runtime_api()
			.generate_session_keys(&generic::BlockId::Hash(best_block_hash), None)
			.map(Into::into)
			.map_err(|api_err| Error::Client(Box::new(api_err)).into())
	}

	fn has_session_keys(&self, session_keys: Bytes) -> RpcResult<bool> {
		self.deny_unsafe.check_if_safe()?;

		let best_block_hash = self.client.info().best_hash;
		let keys = self
			.client
			.runtime_api()
			.decode_session_keys(&generic::BlockId::Hash(best_block_hash), session_keys.to_vec())
			.map_err(|e| JsonRpseeError::to_call_error(e))?
			.ok_or_else(|| Error::InvalidSessionKeys)?;

		Ok(SyncCryptoStore::has_keys(&*self.keystore, &keys))
	}

	fn has_key(&self, public_key: Bytes, key_type: String) -> RpcResult<bool> {
		self.deny_unsafe.check_if_safe()?;

		let key_type = key_type.as_str().try_into().map_err(|_| Error::BadKeyType)?;
		Ok(SyncCryptoStore::has_keys(&*self.keystore, &[(public_key.to_vec(), key_type)]))
	}

	fn pending_extrinsics(&self) -> RpcResult<Vec<Bytes>> {
		Ok(self.pool.ready().map(|tx| tx.data().encode().into()).collect())
	}

	fn remove_extrinsic(
		&self,
		bytes_or_hash: Vec<hash::ExtrinsicOrHash<TxHash<P>>>,
	) -> RpcResult<Vec<TxHash<P>>> {
		self.deny_unsafe.check_if_safe()?;
		let hashes = bytes_or_hash
			.into_iter()
			.map(|x| match x {
				hash::ExtrinsicOrHash::Hash(h) => Ok(h),
				hash::ExtrinsicOrHash::Extrinsic(bytes) => {
					let xt = Decode::decode(&mut &bytes[..])?;
					Ok(self.pool.hash_of(&xt))
				},
			})
			.collect::<Result<Vec<_>>>()?;

		Ok(self
			.pool
			.remove_invalid(&hashes)
			.into_iter()
			.map(|tx| tx.hash().clone())
			.collect())
	}

	fn watch_extrinsic(&self, mut sink: SubscriptionSink, xt: Bytes) -> RpcResult<()> {
		let best_block_hash = self.client.info().best_hash;
		let dxt = match TransactionFor::<P>::decode(&mut &xt[..]) {
			Ok(dxt) => dxt,
			Err(e) => {
				log::error!("[watch_extrinsic sub] failed to decode extrinsic: {:?}", e);
				return Err(JsonRpseeError::to_call_error(e))
			},
		};

		let executor = self.executor.clone();
		let pool = self.pool.clone();
		let fut = async move {
			let stream = match pool
				.submit_and_watch(&generic::BlockId::hash(best_block_hash), TX_SOURCE, dxt)
				.await
			{
				Ok(stream) => stream,
				Err(e) => {
					let _ = sink.send(&format!(
						"txpool subscription failed: {:?}; subscription useless",
						e
					));
					return
				},
			};

			stream
				.for_each(|item| {
					let _ = sink.send(&item);
					futures::future::ready(())
				})
				.await;
		};

		executor.execute(Box::pin(fut));
		Ok(())
	}
}