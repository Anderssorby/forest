// Copyright 2020 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use super::{ChannelInfo, Error, PaychStore, };
use crate::{ChannelAccessor, PaychFundsRes, VoucherInfo, DIR_INBOUND, PaychProvider};
use actor::paych::{Method, SignedVoucher};
use address::Address;
use async_std::sync::{Arc, RwLock};
use async_std::task;
use blockstore::BlockStore;
use cid::Cid;
use encoding::Cbor;
use fil_types::verifier::{FullVerifier, ProofVerifier};
use futures::{TryFutureExt, stream::{FuturesUnordered, StreamExt}};
use message::UnsignedMessage;
use message_pool::{MessagePool, MpoolRpcProvider};
use num_bigint::BigInt;
use std::collections::HashMap;
use wallet::{KeyStore, Wallet};

/// Thread safe payment channel management
pub struct Manager<P, BS>
where
    BS: BlockStore + Send + Sync + 'static,
    P: PaychProvider<BS> + Send + Sync + 'static,
{
    pub store: Arc<RwLock<PaychStore>>,
    #[allow(clippy::type_complexity)]
    pub channels: Arc<RwLock<HashMap<String, Arc<ChannelAccessor<P, BS>>>>>,
    // pub state: Arc<ResourceAccessor<DB, KS>>,
    pub api: Arc<P>,
}
// /// Thread safe access to message pool, state manager and keystore resource for paychannel usage
// pub struct ResourceAccessor<DB, KS, P>
// where
//     DB: BlockStore + Send + Sync + 'static,
//     KS: KeyStore + Send + Sync + 'static,
//     P: PaychProvider + Send + Sync + 'static,
// {
//     pub keystore: Arc<RwLock<KS>>,
//     pub mpool: Arc<MessagePool<MpoolRpcProvider<DB>>>,
//     pub sa: StateAccessor<DB, P>,
//     pub wallet: Arc<RwLock<Wallet<KS>>>,
// }
/// Funds made available in channel
pub struct ChannelAvailableFunds {
    // The address of the channel
    pub channel: Option<Address>,
    // Address of the channel (channel creator)
    pub from: Address,
    // To is the to address of the channel
    pub to: Address,
    // ConfirmedAmt is the amount of funds that have been confirmed on-chain
    // for the channel
    pub confirmed_amt: BigInt,
    // PendingAmt is the amount of funds that are pending confirmation on-chain
    pub pending_amt: BigInt,
    // PendingWaitSentinel can be used with PaychGetWaitReady to wait for
    // confirmation of pending funds
    pub pending_wait_sentinel: Option<Cid>,
    // QueuedAmt is the amount that is queued up behind a pending request
    pub queued_amt: BigInt,
    // VoucherRedeemedAmt is the amount that is redeemed by vouchers on-chain
    // and in the local datastore
    pub voucher_redeemed_amt: BigInt,
}

impl<P, BS> Manager<P, BS>
where
    BS: BlockStore + Send + Sync + 'static,
    // KS: KeyStore + Send + Sync + 'static,
    P: PaychProvider<BS> + Send + Sync + 'static,
{
    // pub fn new(store: PaychStore, state: ResourceAccessor<DB, KS, P>, provider: P) -> Self {
    pub fn new(store: PaychStore, provider: Arc<P>) -> Self
    where
        P: PaychProvider<BS>,
    {
        Manager {
            store: Arc::new(RwLock::new(store)),
            // state: Arc::new(state),
            channels: Arc::new(RwLock::new(HashMap::new())),
            api: provider,
        }
    }
    /// Start restarts tracking of any messages that were sent to chain.
    pub async fn start(&mut self) -> Result<(), Error> {
        self.restart_pending().await
    }
    /// Checks the datastore to see if there are any channels that
    /// have outstanding create / add funds messages, and if so, waits on the
    /// messages.
    /// Outstanding messages can occur if a create / add funds message was sent and
    /// then the system was shut down or crashed before the result was received.
    async fn restart_pending(&mut self) -> Result<(), Error> {
        let mut st = self.store.write().await;
        let cis = st.with_pending_add_funds().await?;

        drop(st);

        let mut err_wait_group = FuturesUnordered::new();

        for mut ci in cis {
            if let Some(msg) = ci.create_msg.clone() {
                let ca = self.accessor_by_from_to(ci.control, ci.target).await?;

                err_wait_group.push(task::spawn(async move {
                    ca.wait_paych_create_msg(ci.id, &msg).await
                }));
                return Ok(());
            } else if let Some(msg) = ci.add_funds_msg.clone() {
                let ch = ci
                    .channel
                    .ok_or_else(|| Error::Other("error retrieving channel".to_string()))?;
                let ca = self.accessor_by_address(ch).await?;

                err_wait_group.push(task::spawn(async move {
                    ca.wait_add_funds_msg(&mut ci, msg).await
                }));
                return Ok(());
            }
        }

        while let Some(result) = err_wait_group.next().await {
            result?;
        }
        Ok(())
    }
    /// Ensures that a channel exists between the from and to addresses,
    /// and adds the given amount of funds.
    pub async fn get_paych(
        &self,
        from: Address,
        to: Address,
        amt: BigInt,
    ) -> Result<PaychFundsRes, Error> {
        let chan_accesor = self.accessor_by_from_to(from, to).await?;
        Ok(chan_accesor.get_paych(from, to, amt).await?)
    }
    /// Returns available funds within provided addressed channel
    pub async fn available_funds(&self, ch: Address) -> Result<ChannelAvailableFunds, Error> {
        let ca = self.accessor_by_address(ch).await?;

        let ci = self.get_channel_info(&ch).await?;

        ca.process_queue(ci.id).await
    }
    // intentionally unused, to be used for paych RPC usage
    async fn _available_funds_by_from_to(
        &self,
        from: Address,
        to: Address,
    ) -> Result<ChannelAvailableFunds, Error> {
        let st = self.store.read().await;
        let ca = self.accessor_by_from_to(from, to).await?;

        let ci = match st.outbound_active_by_from_to(from, to).await {
            Ok(ci) => ci,
            Err(e) => {
                if e == Error::ChannelNotTracked {
                    // If there is no active channel between from / to we still want to
                    // return an empty ChannelAvailableFunds, so that clients can check
                    // for the existence of a channel between from / to without getting
                    // an error.
                    return Ok(ChannelAvailableFunds {
                        channel: None,
                        from,
                        to,
                        confirmed_amt: BigInt::default(),
                        pending_amt: BigInt::default(),
                        pending_wait_sentinel: None,
                        queued_amt: BigInt::default(),
                        voucher_redeemed_amt: BigInt::default(),
                    });
                } else {
                    return Err(Error::Other(e.to_string()));
                }
            }
        };
        ca.process_queue(ci.id).await
    }
    /// Lists channels that exist in the paych store
    pub async fn list_channels(&self) -> Result<Vec<Address>, Error> {
        let store = self.store.read().await;
        store.list_channels().await
    }
    /// Returns channel info by provided address
    pub async fn get_channel_info(&self, addr: &Address) -> Result<ChannelInfo, Error> {
        let store = self.store.read().await;
        store.get_channel_info(addr).await
    }
    /// Creates a voucher from the provided address and signed voucher  
    pub async fn create_voucher<V: ProofVerifier + Send + Sync + 'static>(
        &self,
        addr: Address,
        voucher: SignedVoucher,
    ) -> Result<SignedVoucher, Error> {
        let ca = self.accessor_by_address(addr).await?;
        ca.create_voucher::<V>(addr, voucher).await
    }
    /// Check if the given voucher is valid (is or could become spendable at some point).
    /// If the channel is not in the store, fetches the channel from state (and checks that
    /// the channel To address is owned by the wallet).
    pub async fn check_voucher_valid(
        &mut self,
        ch: Address,
        sv: SignedVoucher,
    ) -> Result<(), Error> {
        println!("heh1");
        let ca = self.inbound_channel_accessor(ch).await?;
        println!("heh2");

        let _ = ca.check_voucher_valid(ch, sv).await?;
        Ok(())
    }
    /// Returns true if voucher is deemed spendable
    pub async fn check_voucher_spendable(
        &self,
        addr: Address,
        sv: SignedVoucher,
        secret: Vec<u8>,
        proof: Vec<u8>,
    ) -> Result<bool, Error> {
        if !proof.is_empty() {
            return Err(Error::Other("payment channel proof parameter is not supported".to_string()));
        }
        let ca = self.accessor_by_address(addr).await?;
        ca.check_voucher_spendable(addr, sv, secret, proof).await
    }
    pub async fn submit_voucher (&self, ch: Address, sv: SignedVoucher, secret: &[u8], proof: &[u8]) -> Result<Cid, Error> {
        if !proof.is_empty() {
            return Err(Error::Other("payment channel proof parameter is not supported".to_string()));
        }
        let ca = self.accessor_by_address(ch).await?;
        ca.submit_voucher(ch, sv, secret).await
    }
    /// Adds a voucher for an outbound channel.
    /// Returns an error if the channel is not already in the store.
    pub async fn add_voucher_outbound(
        &self,
        ch: Address,
        sv: SignedVoucher,
        proof: Vec<u8>,
        min_delta: BigInt,
    ) -> Result<BigInt, Error> {
        let ca = &self.accessor_by_address(ch).await?;
        ca.add_voucher(ch, sv, proof, min_delta).await
    }
    /// Get an accessor for the given channel. The channel
    /// must either exist in the store, or be an inbound channel that can be created
    /// from state.
    pub async fn inbound_channel_accessor(
        &mut self,
        ch: Address,
    ) -> Result<Arc<ChannelAccessor<P, BS>>, Error> {
        // Make sure channel is in store, or can be fetched from state, and that
        // the channel To address is owned by the wallet
        let ci = self.track_inbound_channel(ch).await?;
        println!("heh3333333");

        let from = ci.target;
        let to = ci.control;

        self.accessor_by_from_to(from, to).await
    }
    async fn track_inbound_channel(&mut self, ch: Address) -> Result<ChannelInfo, Error> {
        let mut store = self.store.write().await;

        // Check if channel is in store
        let ci = store.by_address(ch).await;
        match ci {
            Ok(_) => return ci,
            Err(err) => {
                // If there's an error (besides channel not in store) return err
                if err != Error::ChannelNotTracked {
                    return Err(err);
                }
            }
        }
        let state_ci = self
            .api
            .load_state_channel_info(ch, DIR_INBOUND)
            .await
            .map_err(|e| Error::Other(e.to_string()))?;

        let to = state_ci.control;
        println!("tooo: {}", to);
        let to_key = self.api.state_account_key(to, None).map_err(|e| Error::Other(e.to_string()))?;
        println!("tooo key {}", to_key);
        // let has = self.api.wallet_has(to_key).await.map_err(|e| Error::Other(e.to_string()))?;


        if !self
            .api
            .wallet_has(to_key)
            .await
            .map_err(|e| Error::Other(e.to_string()))?
        {
            println!("heh444444444444");

            return Err(Error::NoAddress);
        }

        // save channel to store
        store.track_channel(state_ci).await
    }
    /// Allocates a lane for given address
    pub async fn allocate_lane(&self, ch: Address) -> Result<u64, Error> {
        let mut store = self.store.write().await;
        store.allocate_lane(ch).await
    }
    /// Lists vouchers for given address
    pub async fn list_vouchers(&self, ch: Address) -> Result<Vec<VoucherInfo>, Error> {
        let store = self.store.read().await;
        store.vouchers_for_paych(&ch).await
    }
    /// Returns the next available sequence for lane allocation
    pub async fn next_sequence_for_lane(&self, ch: Address, lane: u64) -> Result<u64, Error> {
        let ca = self.accessor_by_address(ch).await?;
        ca.next_sequence_for_lane(ch, lane).await
    }
    /// Returns CID of signed message thats prepared to be settled on-chain
    pub async fn settle(&self, ch: Address) -> Result<Cid, Error> {
        let mut store = self.store.write().await;
        let mut ci = store.by_address(ch).await?;

        let umsg: UnsignedMessage = UnsignedMessage::builder()
            .to(ch)
            .from(ci.control)
            .value(BigInt::default())
            .method_num(Method::Settle as u64)
            .build()
            .map_err(Error::Other)?;

        let smgs = self
        .api
        .mpool_push_message::<FullVerifier>(umsg)
            .await
            .map_err(|e| Error::Other(e.to_string()))?;

        ci.settling = true;
        store.put_channel_info(&mut ci).await?;

        Ok(smgs.cid()?)
    }
    /// Returns CID of signed message ready to be collected
    pub async fn collect(&self, ch: Address) -> Result<Cid, Error> {
        let store = self.store.read().await;
        let ci = store.by_address(ch).await?;

        let umsg: UnsignedMessage = UnsignedMessage::builder()
            .to(ch)
            .from(ci.control)
            .value(BigInt::default())
            .method_num(Method::Collect as u64)
            .build()
            .map_err(Error::Other)?;

        let smgs = self
            .api
            .mpool_push_message::<FullVerifier>(umsg)
            .await
            .map_err(|e| Error::Other(e.to_string()))?;

        Ok(smgs.cid()?)
    }
    async fn accessor_by_from_to(
        &self,
        from: Address,
        to: Address,
    ) -> Result<Arc<ChannelAccessor<P, BS>>, Error> {
        let channels = self.channels.read().await;
        let key = accessor_cache_key(&from, &to);

        // check if channel accessor is in cache without taking write lock
        let op = channels.get(&key);
        if let Some(channel) = op {
            return Ok(channel.clone());
        }
        drop(channels);
        println!("do we get here?");
        // channel accessor is not in cache so take a write lock
        let channel_write = self.channels.read().await;

        // Need to check cache again in case it was updated between releasing read
	    // lock and taking write lock
        println!("do we get here?1");

        let op_locked = channel_write.get(&key);
        if let Some(channel) = op_locked {
            println!("do we get here?2");

            Ok(channel.clone())
        } else {
            println!("do we get here?3");

            drop(channel_write);
            Ok(self.add_accessor_to_cache(from, to).await)
        }
    }
    /// Add a channel accessor to the cache. Note that the
    /// channel may not have been created yet, but we still want to reference
    /// the same channel accessor for a given from/to, so that all attempts to
    /// access a channel use the same lock (the lock on the accessor)
    async fn add_accessor_to_cache(
        &self,
        from: Address,
        to: Address,
    ) -> Arc<ChannelAccessor<P, BS>> {
        let key = accessor_cache_key(&from, &to);
        let ca = Arc::new(ChannelAccessor::new(&self));
        let mut channels = self.channels.write().await;
        channels
            .insert(key, ca.clone());
        ca
    }
    async fn accessor_by_address(
        &self,
        ch: Address,
    ) -> Result<Arc<ChannelAccessor<P, BS>>, Error> {
        let store = self.store.read().await;
        let ci = store.by_address(ch).await?;
        self.accessor_by_from_to(ci.control, ci.target).await
    }
    /// Adds a voucher for an inbound channel.
    /// If the channel is not in the store, fetches the channel from state (and checks that
    /// the channel To address is owned by the wallet).
    pub async fn add_voucher_inbound(
        &mut self,
        ch: Address,
        sv: SignedVoucher,
        proof: Vec<u8>,
        min_delta: BigInt,
    ) -> Result<BigInt, Error> {
        let ca = self.inbound_channel_accessor(ch).await?;
        ca.add_voucher(ch, sv, proof, min_delta).await
    }
}
fn accessor_cache_key(from: &Address, to: &Address) -> String {
    from.to_string() + "->" + &to.to_string()
}
