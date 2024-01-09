use borsh::BorshDeserialize;
use near_chain::types::RuntimeAdapter;
use near_chain::{ChainStore, ChainStoreAccess};
use near_epoch_manager::{EpochManager, EpochManagerAdapter};
use near_primitives::hash::CryptoHash;
use near_primitives::trie_key::TrieKey;
use near_primitives::types::{
    EpochId, ShardId, StateChangesForBlock, StateChangesForBlockRange, StateChangesForShard,
    StateRoot,
};
use near_primitives_core::types::BlockHeight;
use near_store::{KeyForStateChanges, Store, WrappedTrieChanges};
use nearcore::{NearConfig, NightshadeRuntime};
use std::path::{Path, PathBuf};

#[derive(clap::Subcommand, Debug, Clone)]
pub(crate) enum StateChangesSubCommand {
    /// Applies StateChanges from a file.
    /// Needs state roots because chunks state roots may not be known.
    /// Needs BlockHeaders available for the corresponding blocks.
    Apply {
        /// Location of the input file.
        /// The file must be generated by the Dump subcommand.
        #[clap(value_parser)]
        file: PathBuf,
        /// Id of the shard to apply state changes to.
        shard_id: ShardId,
        /// State root of the shard at the height of the first block with state changes.
        state_root: StateRoot,
    },
    /// Dumps state changes for a range of blocks from --height-from to --height-to inclusive.
    /// The dump will include all available state changes, i.e. of shards tracked at the time.
    Dump {
        /// --height-from defines the lower (inclusive) bound of a range of block heights to dump.
        height_from: BlockHeight,
        /// --height-to defines the upper (inclusive) bound of a range of block heights to dump.
        height_to: BlockHeight,
        /// Location of the output file.
        #[clap(value_parser)]
        file: PathBuf,
    },
}

impl StateChangesSubCommand {
    pub(crate) fn run(self, home_dir: &Path, near_config: NearConfig, store: Store) {
        match self {
            StateChangesSubCommand::Apply { file, shard_id, state_root } => {
                apply_state_changes(file, shard_id, state_root, home_dir, near_config, store)
            }
            StateChangesSubCommand::Dump { height_from, height_to, file } => {
                dump_state_changes(height_from, height_to, file, near_config, store)
            }
        }
    }
}

/// Reads StateChanges from the DB for the specified range of blocks.
/// Writes these state changes to the specified file.
/// The data written is borsh-serialized StateChangesForBlockRange.
/// State changes in that file are expected to be ordered in the increasing order of height.
/// Changes are grouped by (block_hash, shard_id).
/// If a block or a shard has no changes, they can be omitted from the serialization.
/// Row key of the StateChanges column are really important because the logic relies that state
/// changes of kinds DelayedReceipt and DelayedReceiptIndices have shard id encoded in the row key.
fn dump_state_changes(
    height_from: BlockHeight,
    height_to: BlockHeight,
    file: PathBuf,
    near_config: NearConfig,
    store: Store,
) {
    assert!(height_from <= height_to, "--height-from must be less than or equal to --height-to");

    let epoch_manager = EpochManager::new_arc_handle(store.clone(), &near_config.genesis.config);
    let chain_store = ChainStore::new(
        store.clone(),
        near_config.genesis.config.genesis_height,
        near_config.client_config.save_trie_changes,
    );

    let blocks = (height_from..=height_to).filter_map(|block_height| {
        let block_header = chain_store.get_block_header_by_height(block_height).unwrap();
        let block_hash = block_header.hash();
        let epoch_id = block_header.epoch_id();
        let key = KeyForStateChanges::for_block(block_header.hash());
        let mut state_changes_per_shard: Vec<_> =
            epoch_manager.shard_ids(epoch_id).unwrap().into_iter().map(|_| vec![]).collect();

        for row in key.find_rows_iter(&store) {
            let (key, value) = row.unwrap();
            let shard_id = get_state_change_shard_id(key.as_ref(), &value.trie_key, block_hash, epoch_id, epoch_manager.as_ref()).unwrap();
            state_changes_per_shard[shard_id as usize].push(value);
        }

        tracing::info!(target: "state-changes", block_height = block_header.height(), num_state_changes_per_shard = ?state_changes_per_shard.iter().map(|v|v.len()).collect::<Vec<usize>>());
        let state_changes : Vec<StateChangesForShard> = state_changes_per_shard.into_iter().enumerate().filter_map(|(shard_id,state_changes)|{
            if state_changes.is_empty() {
                // Skip serializing state changes for a shard if no state changes were found for this shard in this block.
                None
            } else {
                Some(StateChangesForShard{shard_id:shard_id as ShardId, state_changes})
            }
        }).collect();

        if state_changes.is_empty() {
            // Skip serializing state changes for a block if no state changes were found for this block.
            None
        } else {
            Some(StateChangesForBlock { block_hash: *block_hash, state_changes })
        }
    }).collect();

    let state_changes_for_block_range = StateChangesForBlockRange { blocks };

    tracing::info!(target: "state-changes", ?file, "Writing state changes to a file");
    let data: Vec<u8> = borsh::to_vec(&state_changes_for_block_range).unwrap();
    std::fs::write(&file, data).unwrap();
}

/// Reads StateChanges from a file. Applies StateChanges in the order of increasing block height.
///
/// The file is assumed to be created by `dump_state_changes`. Same assumptions apply.
/// Row key of the StateChanges column are really important because the logic relies that state
/// changes of kinds DelayedReceipt and DelayedReceiptIndices have shard id encoded in the row key.
///
/// The operation needs state viewer to be run in read-write mode, use `--readwrite` flag.
///
/// In case the DB contains state roots of chunks, the process compares resulting StateRoots with those known StateRoots.
fn apply_state_changes(
    file: PathBuf,
    shard_id: ShardId,
    mut state_root: StateRoot,
    home_dir: &Path,
    near_config: NearConfig,
    store: Store,
) {
    let epoch_manager = EpochManager::new_arc_handle(store.clone(), &near_config.genesis.config);
    let runtime = NightshadeRuntime::from_config(
        home_dir,
        store.clone(),
        &near_config,
        epoch_manager.clone(),
    );
    let mut chain_store = ChainStore::new(
        store,
        near_config.genesis.config.genesis_height,
        near_config.client_config.save_trie_changes,
    );

    let data = std::fs::read(&file).unwrap();
    let state_changes_for_block_range = StateChangesForBlockRange::try_from_slice(&data).unwrap();

    for StateChangesForBlock { block_hash, state_changes } in state_changes_for_block_range.blocks {
        let block_header = chain_store.get_block_header(&block_hash).unwrap();
        let block_hash = block_header.hash();
        let block_height = block_header.height();
        let epoch_id = block_header.epoch_id();
        let shard_uid = epoch_manager.shard_id_to_uid(shard_id, epoch_id).unwrap();

        for StateChangesForShard { shard_id: state_change_shard_id, state_changes } in state_changes
        {
            if state_change_shard_id != shard_id {
                continue;
            }

            if let Ok(block) = chain_store.get_block(block_hash) {
                let known_state_root = block.chunks()[shard_id as usize].prev_state_root();
                assert_eq!(known_state_root, state_root);
                tracing::debug!(target: "state-changes", block_height, ?state_root, "Known StateRoot matches");
            }

            tracing::info!(target: "state-changes", block_height, ?block_hash, ?shard_uid, ?state_root, num_changes = state_changes.len(), "Applying state changes");
            let trie = runtime.get_trie_for_shard(shard_id, block_hash, state_root, false).unwrap();

            let trie_update = trie
                .update(state_changes.iter().map(|raw_state_changes_with_trie_key| {
                    tracing::debug!(target: "state-changes", ?raw_state_changes_with_trie_key);
                    let raw_key = raw_state_changes_with_trie_key.trie_key.to_vec();
                    let data = raw_state_changes_with_trie_key.changes.last().unwrap().data.clone();
                    (raw_key, data)
                }))
                .unwrap();

            tracing::info!(target: "state-change", block_height, ?block_hash, ?shard_uid, old_state_root = ?trie_update.old_root, new_state_root = ?trie_update.new_root, "Applied state changes");
            state_root = trie_update.new_root;

            let wrapped_trie_changes = WrappedTrieChanges::new(
                runtime.get_tries(),
                shard_uid,
                trie_update,
                state_changes,
                *block_hash,
                block_height,
            );
            let mut store_update = chain_store.store_update();
            store_update.save_trie_changes(wrapped_trie_changes);
            store_update.commit().unwrap();
        }
    }

    tracing::info!(target: "state-changes", ?file, ?shard_id, ?state_root, "Done applying changes");
}

/// Determines the shard id which produced the StateChange based the row key,
/// part of the value (TrieKey) and the block that resulted in this state change.
pub fn get_state_change_shard_id(
    row_key: &[u8],
    trie_key: &TrieKey,
    block_hash: &CryptoHash,
    epoch_id: &EpochId,
    epoch_manager: &dyn EpochManagerAdapter,
) -> Result<ShardId, near_chain::near_chain_primitives::error::Error> {
    if let Some(account_id) = trie_key.get_account_id() {
        let shard_id = epoch_manager.account_id_to_shard_id(&account_id, epoch_id)?;
        Ok(shard_id)
    } else {
        let shard_uid =
            KeyForStateChanges::delayed_receipt_key_decode_shard_uid(row_key, block_hash, trie_key)
                .map_err(|err| {
                    near_chain::near_chain_primitives::error::Error::Other(err.to_string())
                })?;
        Ok(shard_uid.shard_id as ShardId)
    }
}
