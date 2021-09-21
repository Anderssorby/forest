use std::sync::Arc;

use cid::{Cid, Code::Blake2b256};
use ipld_blockstore::BlockStore;

use actor_interface::actorv2::market::State as MarketV2State;
use actor_interface::actorv3::market::State as MarketV3State;
use actor_interface::actorv3::MARKET_ACTOR_CODE_ID;
use actorv3::market::{PROPOSALS_AMT_BITWIDTH, STATES_AMT_BITWIDTH};
use actorv3::BALANCE_TABLE_BITWIDTH;

use crate::{
    ActorMigration, ActorMigrationInput, MigrationError, MigrationOutput, MigrationResult,
};

use super::{migrate_amt_raw, migrate_hamt_hamt_raw, migrate_hamt_raw};

pub struct MarketMigrator(Cid);

impl<BS: BlockStore + Send + Sync> ActorMigration<BS> for MarketMigrator {
    fn migrate_state(
        &self,
        store: Arc<BS>,
        input: ActorMigrationInput,
    ) -> MigrationResult<MigrationOutput> {
        let in_state: MarketV2State = store
            .get(&input.head)
            .map_err(|e| MigrationError::BlockStoreRead(e.to_string()))?
            .ok_or_else(|| {
                MigrationError::BlockStoreRead("Init actor: could not read v2 state".to_string())
            })?;

        let pending_proposals = map_pending_proposals(store.as_ref(), &in_state.pending_proposals)?;

        let proposals =
            migrate_amt_raw(store.as_ref(), &in_state.proposals, PROPOSALS_AMT_BITWIDTH)?;

        let states = migrate_amt_raw(store.as_ref(), &in_state.states, STATES_AMT_BITWIDTH)?;

        let escrow_table = migrate_hamt_raw(
            store.as_ref(),
            &in_state.escrow_table,
            BALANCE_TABLE_BITWIDTH,
        )?;

        let locked_table = migrate_hamt_raw(
            store.as_ref(),
            &in_state.locked_table,
            BALANCE_TABLE_BITWIDTH,
        )?;

        let deal_ops_by_epoch =
            migrate_hamt_hamt_raw(store.as_ref(), &in_state.deal_ops_by_epoch, 5)?;

        let out_state = MarketV3State {
            proposals,
            pending_proposals,
            states,
            escrow_table,
            locked_table,
            deal_ops_by_epoch,
            next_id: in_state.next_id,
            last_cron: in_state.last_cron,
            total_client_locked_colateral: in_state.total_client_locked_colateral,
            total_provider_locked_colateral: in_state.total_provider_locked_colateral,
            total_client_storage_fee: in_state.total_client_storage_fee,
        };

        let new_head = store
            .put(&out_state, Blake2b256)
            .map_err(|e| MigrationError::BlockStoreWrite(e.to_string()))?;

        Ok(MigrationOutput {
            new_code_cid: *MARKET_ACTOR_CODE_ID,
            new_head,
        })
    }
}

fn map_pending_proposals<BS: BlockStore + Send + Sync>(
    store: &BS,
    pending_proposals: &Cid,
) -> Result<Cid, MigrationError> {
    todo!()
}
