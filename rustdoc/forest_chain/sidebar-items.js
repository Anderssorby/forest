window.SIDEBAR_ITEMS = {"constant":[["BASE_FEE_MAX_CHANGE_DENOM","Limits gas base fee change to 12.5% of the change."],["BLOCK_GAS_TARGET","Used in calculating the base fee change."],["INITIAL_BASE_FEE","Genesis base fee."],["MINIMUM_BASE_FEE",""],["PACKING_EFFICIENCY_DENOM",""],["PACKING_EFFICIENCY_NUM",""]],"enum":[["Error","Chain error"],["HeadChange","`Enum` for `pubsub` channel that defines message type variant and data contained in message type."]],"fn":[["block_messages","Returns a Tuple of BLS messages of type `UnsignedMessage` and SECP messages of type `SignedMessage`"],["block_messages_from_cids","Returns a tuple of `UnsignedMessage` and `SignedMessages` from their CID"],["compute_base_fee",""],["get_chain_message","Attempts to de-serialize to unsigned message or signed message and then returns it as a [`ChainMessage`]."],["get_parent_reciept","Returns parent message receipt given `block_header` and message index."],["messages_for_tipset","Given a tipset this function will return all unique messages in that tipset."],["messages_from_cids","Returns messages from key-value store based on a slice of [`Cid`]s."],["persist_block_messages","Partition the messages into SECP and BLS variants, store them individually in the IPLD store, and the corresponding `TxMeta` as well, returning its CID so that it can be put in a block header. Also return the aggregated BLS signature of all BLS messages."],["persist_objects","Persists slice of `serializable` objects to `blockstore`."],["read_msg_cids","Returns a tuple of CIDs for both unsigned and signed messages"]],"mod":[["base_fee",""],["headchange_json",""]],"struct":[["ChainStore","Stores chain data such as heaviest tipset and cached tipset info at each epoch. This structure is thread-safe, and all caches are wrapped in a mutex to allow a consistent `ChainStore` to be shared across tasks."],["PersistedBlockMessages","Result of persisting a vector of `SignedMessage`s that are to be included in a block."]],"trait":[["Scale","The `Scale` trait abstracts away the logic of assigning a weight to a chain, which can be consensus specific. For example it can depend on the stake and power of validators, or it can be as simple as the height of the blocks in a `Nakamoto` style consensus."]],"type":[["Weight",""]]};