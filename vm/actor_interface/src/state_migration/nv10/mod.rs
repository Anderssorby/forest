mod init_actor;
mod util;
use std::collections::HashMap;

use cid::Cid;
use clock::ChainEpoch;
use ipld_blockstore::BlockStore;

pub struct ActorMigrations {}

pub fn migrate_state_tree<BS: BlockStore>(
    store: &BS,
    actors_root_in: Cid,
    prior_epoch: ChainEpoch,
) {
    // let migrations: HashMap<Cid, ActorMigrations> = HashMap::new();
    // migrations.insert(actorv2::INIT_ACTOR_CODE_ID)
    todo!()
}
