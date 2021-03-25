// Copyright 2020 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use super::errors::Error;
use actor::paych::SignedVoucher;
use address::Address;
use cid::Cid;
use derive_builder::Builder;
use encoding::Cbor;
use log::warn;
use num_bigint::{
    bigint_ser::{BigIntDe, BigIntSer},
    BigInt,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::str::FromStr;
use uuid::Uuid;

pub const DIR_INBOUND: u8 = 1;
pub const DIR_OUTBOUND: u8 = 2;
const DS_KEY_CHANNEL_INFO: &str = "ChannelInfo";
const DS_KEY_MSG_CID: &str = "MsgCid";

/// VoucherInfo contains information about Voucher and its submission
#[derive(Serialize, Deserialize, Clone)]
pub struct VoucherInfo {
    pub voucher: SignedVoucher,
    pub proof: Vec<u8>,
    pub submitted: bool,
}

/// ChannelInfo keeps track of information about a channel
#[derive(Clone, Builder)]
#[builder(name = "ChannelInfoBuilder")]
pub struct ChannelInfo {
    /// id is a uuid that is created upon adding to the paychstore
    #[builder(default)]
    pub id: String,
    /// Channel address can only be None if the channel hasn't been created yet
    #[builder(default)]
    pub channel: Option<Address>,
    /// Address of the account that created the channel
    pub control: Address,
    /// Address of the account on the other side of the channel
    pub target: Address,
    /// Direction indicates if the channel is inbound (this node is the target)
    /// or outbound (this node is the control)
    pub direction: u8,
    /// The list of all vouchers sent on the channel
    #[builder(default)]
    pub vouchers: Vec<VoucherInfo>,
    /// Number of the next lane that should be used when the client requests a new lane
    /// (ie makes a new voucher for a new deal)
    pub next_lane: u64,
    /// Amount to be added to the channel
    /// This amount is only used by get_paych to keep track of how much
    /// has locally been added to the channel. It should reflect the channel's
    /// Balance on chain as long as all operations occur in the same datastore
    #[builder(default)]
    pub amount: BigInt,
    /// The amount that's awaiting confirmation
    #[builder(default)]
    pub pending_amount: BigInt,
    /// The CID of a pending create message while waiting for confirmation
    #[builder(default)]
    pub create_msg: Option<Cid>,
    /// The CID of a pending add funds message while waiting for confirmation
    #[builder(default)]
    pub add_funds_msg: Option<Cid>,
    /// indicates whether or not the channel has entered into the settling state
    #[builder(default)]
    pub settling: bool,
}

impl Serialize for ChannelInfo {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        (
            &self.id,
            &self.channel,
            &self.control,
            &self.target,
            &self.direction,
            &self.vouchers,
            &self.next_lane,
            BigIntSer(&self.amount),
            BigIntSer(&self.pending_amount),
            &self.create_msg,
            &self.add_funds_msg,
            &self.settling,
        )
            .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ChannelInfo {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as Deserializer<'de>>::Error>
    where
        D: Deserializer<'de>,
    {
        let (
            id,
            channel,
            control,
            target,
            direction,
            vouchers,
            next_lane,
            BigIntDe(amount),
            BigIntDe(pending_amount),
            create_msg,
            add_funds_msg,
            settling,
        ) = Deserialize::deserialize(deserializer)?;

        let ci = ChannelInfo {
            id,
            channel,
            control,
            target,
            direction,
            vouchers,
            next_lane,
            amount,
            pending_amount,
            create_msg,
            add_funds_msg,
            settling,
        };

        Ok(ci)
    }
}

impl ChannelInfo {
    pub fn builder() -> ChannelInfoBuilder {
        ChannelInfoBuilder::default()
    }

    pub fn from(&self) -> Address {
        if self.direction == DIR_OUTBOUND {
            return self.control;
        }
        self.target
    }

    pub fn to(&self) -> Address {
        if self.direction == DIR_OUTBOUND {
            return self.target;
        }
        self.control
    }

    /// Retrieves the VoucherInfo for the given voucher.
    /// returns nil if the channel doesn't have the voucher.
    pub fn info_for_voucher(&self, sv: &SignedVoucher) -> Result<Option<VoucherInfo>, Error> {
        // return voucher info
        for v in &self.vouchers {
            let cbor_v = v
                .voucher
                .signing_bytes()
                .map_err(|e| Error::Encoding(e.to_string()))?;
            let cbor_sv = sv
                .signing_bytes()
                .map_err(|e| Error::Encoding(e.to_string()))?;
            if cbor_v == cbor_sv {
                return Ok(Some(v.clone()));
            }
        }

        Ok(None)
    }
    /// Returns true if voucher is present
    pub fn has_voucher(&self, sv: &SignedVoucher) -> Result<bool, Error> {
        Ok(self.info_for_voucher(sv)?.is_some())
    }
    /// Marks the voucher, and any vouchers of lower nonce
    /// in the same lane, as being submitted.
    /// Note: This method doesn't write anything to the store.
    pub fn mark_voucher_submitted(&mut self, sv: &SignedVoucher) -> Result<(), Error> {
        if let Some(mut vi) = self.info_for_voucher(&sv)? {
            // mark the voucher as submitted
            vi.submitted = true;

            // Mark lower-nonce vouchers in the same lane as submitted (lower-nonce
            // vouchers are superseded by the submitted voucher)
            for mut v in self.vouchers.iter_mut() {
                if v.voucher.lane() == sv.lane() && v.voucher.nonce() < sv.nonce() {
                    v.submitted = true;
                }
            }
        } else {
            return Err(Error::Other(
                "cannot submit voucher that has not been added to channel".to_string(),
            ));
        }

        Ok(())
    }
    /// Returns true if the voucher has been submitted
    pub fn was_voucher_submitted(&self, sv: &SignedVoucher) -> Result<bool, Error> {
        if let Some(vi) = self.info_for_voucher(sv)? {
            Ok(vi.submitted)
        } else {
            Ok(false)
        }
    }
}
#[derive(Clone)]
pub struct PaychStore {
    pub ds: HashMap<String, Vec<u8>>,
}

impl Default for PaychStore {
    fn default() -> Self {
        Self::new()
    }
}

impl Cbor for ChannelInfo {}

impl PaychStore {
    /// Create new Pay Channel Store
    pub fn new() -> Self {
        let ds: HashMap<String, Vec<u8>> = HashMap::new();
        PaychStore { ds }
    }
    /// Stores a channel, returning an error if the channel was already
    /// being tracked
    pub async fn track_channel(&mut self, ch: ChannelInfo) -> Result<ChannelInfo, Error> {
        let addr = ch.channel.ok_or_else(|| Error::NoAddress)?;
        match self.by_address(addr).await {
            Err(Error::ChannelNotTracked) => {
                self.put_channel_info(&mut ch.clone()).await?;
                self.by_address(ch.channel.unwrap()).await
            }
            Ok(_) => Err(Error::DupChannelTracking),
            Err(err) => Err(err),
        }
    }
    /// Return a Vec of all ChannelInfo Addresses in paych_store
    pub async fn list_channels(&self) -> Result<Vec<Address>, Error> {
        let res = self.ds.keys();
        let mut out = Vec::new();
        for addr_str in res {
            if addr_str.starts_with("ChannelInfo/") {
                out.push(
                    Address::from_str(addr_str.trim_start_matches("ChannelInfo/"))
                        .map_err(|err| Error::Other(err.to_string()))?,
                )
            } else {
                warn!("invalid ChannelInfo Channel Address: {}", addr_str);
                continue;
            }
        }
        Ok(out)
    }
    /// Find a single channel using the given filter, if no channel matches, return ChannelNotTrackedError
    pub async fn find_chan(
        &self,
        filter: Box<dyn Fn(&ChannelInfo) -> bool + Send>,
    ) -> Result<ChannelInfo, Error> {
        let one: usize = 1;
        let mut ci = self.find_chans(filter, one).await?;

        if ci.is_empty() {
            return Err(Error::ChannelNotTracked);
        }

        Ok(ci.pop().unwrap())
    }
    /// Loop over all channels, return Vec of all channels that fit given filter, specify max to be the max length
    /// of returned Vec, set max to 0 for Vec of all channels that fit the given filter
    pub async fn find_chans(
        &self,
        filter: Box<dyn Fn(&ChannelInfo) -> bool + Send>,
        max: usize,
    ) -> Result<Vec<ChannelInfo>, Error> {
        let mut matches = Vec::new();

        for val in self.ds.values() {
            let ci = ChannelInfo::unmarshal_cbor(val)?;
            if filter(&ci) {
                matches.push(ci);
                if matches.len() == max {
                    return Ok(matches);
                }
            }
        }
        Ok(matches)
    }
    /// Allocate a lane for a given ChannelInfo
    pub async fn allocate_lane(&mut self, ch: Address) -> Result<u64, Error> {
        let mut ci = self.get_channel_info(&ch).await?;
        let out = ci.next_lane;
        ci.next_lane += 1;
        self.put_channel_info(&mut ci).await?;
        Ok(out)
    }
    /// Return Vec of all voucher infos for given ChannelInfo Address
    pub async fn vouchers_for_paych(&self, ch: &Address) -> Result<Vec<VoucherInfo>, Error> {
        let ci = self.get_channel_info(ch).await?;
        Ok(ci.vouchers)
    }
    /// Marks voucher as submitted and put it in storage
    pub async fn mark_voucher_submitted(
        &mut self,
        ci: &mut ChannelInfo,
        sv: &SignedVoucher,
    ) -> Result<(), Error> {
        ci.mark_voucher_submitted(sv)?;

        Ok(self.put_channel_info(ci).await?)
    }
    /// Retrieves the ChannelInfo that matches given Address
    pub async fn by_address(&self, addr: Address) -> Result<ChannelInfo, Error> {
        for val in self.ds.values() {
            let ci = ChannelInfo::unmarshal_cbor(val)?;
            if ci.channel.ok_or_else(|| Error::NoAddress)? == addr {
                return Ok(ci);
            }
        }
        Err(Error::ChannelNotTracked)
    }
    /// Stores message when a new message is sent
    pub async fn save_new_message(&mut self, channel_id: String, mcid: Cid) -> Result<(), Error> {
        let k = key_for_msg(&mcid);
        let mi: MsgInfo = MsgInfo {
            channel_id,
            msg_cid: mcid,
            received: false,
            err: "".to_string(),
        };
        let bytes = mi
            .marshal_cbor()
            .map_err(|err| Error::Other(err.to_string()))?;
        self.ds.insert(k, bytes);
        Ok(())
    }
    /// Stores the result of a message when the result is received
    pub async fn save_msg_result(
        &mut self,
        mcid: Cid,
        msg_err: Option<Error>,
    ) -> Result<(), Error> {
        let k = key_for_msg(&mcid);
        let mut minfo = self.get_message(&mcid).await?;
        if msg_err.is_some() {
            minfo.err = msg_err.unwrap().to_string();
        }
        let b = minfo
            .marshal_cbor()
            .map_err(|err| Error::Other(err.to_string()))?;
        self.ds.insert(k, b);
        Ok(())
    }
    /// Retrieves the channel info associated with a message
    pub async fn by_message_cid(&self, mcid: &Cid) -> Result<ChannelInfo, Error> {
        let minfo = self.get_message(mcid).await?;
        for val in self.ds.values() {
            let ci = ChannelInfo::unmarshal_cbor(val)?;
            if ci.id == minfo.channel_id {
                return Ok(ci);
            }
        }
        Err(Error::ChannelNotTracked)
    }
    /// Get the message info for a given message CID
    pub async fn get_message(&self, mcid: &Cid) -> Result<MsgInfo, Error> {
        let k = key_for_msg(mcid);
        let val = self.ds.get(&k).ok_or_else(|| Error::NoVal)?;
        let minfo = MsgInfo::unmarshal_cbor(val.as_slice())?;
        Ok(minfo)
    }
    /// Return first outbound channel that has not been settles with given to and from address
    pub async fn outbound_active_by_from_to(
        &self,
        from: Address,
        to: Address,
    ) -> Result<ChannelInfo, Error> {
        for val in self.ds.values() {
            let ci = ChannelInfo::unmarshal_cbor(val)?;
            if ci.direction == DIR_OUTBOUND {
                continue;
            }
            if ci.settling {
                continue;
            }
            if (ci.control == from) & (ci.target == to) {
                return Ok(ci);
            }
        }
        Err(Error::ChannelNotTracked)
    }
    /// This function is used on start up to find channels where a create channel or add funds message
    /// has been sent, but node was shut down before response was received
    pub async fn with_pending_add_funds(&mut self) -> Result<Vec<ChannelInfo>, Error> {
        self.find_chans(
            Box::new(|ci| {
                if ci.direction != DIR_OUTBOUND {
                    return false;
                }
                if ci.add_funds_msg.is_none() {
                    return false;
                }
                (ci.create_msg.as_ref().unwrap().clone() != Cid::default())
                    | (ci.add_funds_msg.as_ref().unwrap().clone() != Cid::default())
            }),
            0,
        )
        .await
    }
    /// Get channel info given channel ID
    pub async fn by_channel_id(&self, channel_id: &str) -> Result<ChannelInfo, Error> {
        let res = self
            .ds
            .get(channel_id)
            .ok_or_else(|| Error::ChannelNotTracked)?;
        let ci = ChannelInfo::unmarshal_cbor(res)?;
        Ok(ci)
    }
    /// Create a new new outbound channel for given parameters
    pub async fn create_channel(
        &mut self,
        from: Address,
        to: Address,
        create_msg_cid: Cid,
        amt: BigInt,
    ) -> Result<ChannelInfo, Error> {
        let ci = ChannelInfo {
            id: "".to_string(),
            channel: None,
            vouchers: Vec::new(),
            direction: DIR_OUTBOUND,
            next_lane: 0,
            control: from,
            target: to,
            create_msg: Some(create_msg_cid.clone()),
            pending_amount: amt,
            amount: BigInt::default(),
            add_funds_msg: None,
            settling: false,
        };
        self.put_channel_info(&mut ci.clone()).await?;
        self.save_new_message(ci.id.clone(), create_msg_cid).await?;
        Ok(ci)
    }
    /// Retrieves ChannelInfo for a given Channel Address
    pub async fn get_channel_info(&self, addr: &Address) -> Result<ChannelInfo, Error> {
        if let Some(k) = self.ds.get(&format!("ChannelInfo/{}", addr.to_string())) {
            let ci = ChannelInfo::unmarshal_cbor(&k)?;
            Ok(ci)
        } else {
            Err(Error::ChannelNotTracked)
        }
    }
    /// Remove a channel with given channel ID
    pub async fn remove_channel(&mut self, channel_id: String) -> Result<(), Error> {
        self.ds
            .remove(&format!("{}/{}", DS_KEY_CHANNEL_INFO, channel_id))
            .ok_or_else(|| Error::ChannelNotTracked)?;
        Ok(())
    }
    /// Add ChannelInfo to PaychStore
    pub async fn put_channel_info(&mut self, ci: &mut ChannelInfo) -> Result<(), Error> {
        if ci.id.is_empty() {
            ci.id = Uuid::new_v4().to_string();
        }
        let key = key_for_channel(ci.channel.ok_or_else(|| Error::NoAddress)?.to_string());
        let value = ci
            .marshal_cbor()
            .map_err(|err| Error::Other(err.to_string()))?;

        self.ds.insert(key, value);
        Ok(())
    }
}

fn key_for_channel(channel_id: String) -> String {
    return format!("{}/{}", DS_KEY_CHANNEL_INFO, channel_id);
}

fn key_for_msg(mcid: &Cid) -> String {
    return format!("{}/{}", DS_KEY_MSG_CID, mcid.to_string());
}

/// MsgInfo stores information about a created channel / add funds message that has been sent
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct MsgInfo {
    channel_id: String,
    msg_cid: Cid,
    pub received: bool,
    pub err: String,
}

impl Cbor for MsgInfo {}
