use std::collections::HashMap;

use crate::{
    types::{settings::get_settings, signature_map::SignatureMap, state::AssetHashes},
    utils::time::get_current_time,
};

use super::hash;
use crate::types::delegation::Delegation;
use ic_cdk::api::set_certified_data;
use ic_certified_map::{AsHashTree, Hash};

pub const LABEL_ASSETS: &[u8] = b"http_assets";
pub const LABEL_SIG: &[u8] = b"sig";

pub(crate) fn calculate_seed(address: &str) -> Hash {
    let settings = get_settings().unwrap();

    let mut blob: Vec<u8> = vec![];

    let salt = settings.salt.as_bytes();
    blob.push(salt.len() as u8);
    blob.extend_from_slice(&salt);

    let address = address.as_bytes();
    blob.push(address.len() as u8);
    blob.extend(address);

    let uri = settings.uri.as_bytes();
    blob.push(uri.len() as u8);
    blob.extend(uri);

    hash::hash_bytes(blob)
}

pub(crate) fn delegation_signature_msg_hash(delegation: &Delegation) -> Hash {
    use hash::Value;

    let mut hash_map = HashMap::new();
    hash_map.insert("pubkey", Value::Bytes(delegation.pubkey.as_slice()));
    hash_map.insert("expiration", Value::U64(delegation.expiration));
    if let Some(targets) = delegation.targets.as_ref() {
        let mut arr = Vec::with_capacity(targets.len());
        for t in targets.iter() {
            arr.push(Value::Bytes(t.as_ref()));
        }
        hash_map.insert("targets", Value::Array(arr));
    }
    let map_hash = hash::hash_of_map(hash_map);
    hash::hash_with_domain(b"ic-request-auth-delegation", &map_hash)
}

pub(crate) fn prune_expired_signatures(asset_hashes: &AssetHashes, sigs: &mut SignatureMap) {
    const MAX_SIGS_TO_PRUNE: usize = 10;
    let num_pruned = sigs.prune_expired(get_current_time() as u64, MAX_SIGS_TO_PRUNE);

    if num_pruned > 0 {
        update_root_hash(asset_hashes, sigs);
    }
}

pub(crate) fn update_root_hash(a: &AssetHashes, m: &SignatureMap) {
    use ic_certified_map::{fork_hash, labeled_hash};

    let prefixed_root_hash = fork_hash(
        // NB: Labels added in lexicographic order
        &labeled_hash(LABEL_ASSETS, &a.root_hash()),
        &labeled_hash(LABEL_SIG, &m.root_hash()),
    );
    set_certified_data(&prefixed_root_hash[..]);
}

pub(crate) fn der_encode_canister_sig_key(seed: Vec<u8>) -> Vec<u8> {
    let my_canister_id: Vec<u8> = ic_cdk::api::id().as_ref().to_vec();

    let mut bitstring: Vec<u8> = vec![];
    bitstring.push(my_canister_id.len() as u8);
    bitstring.extend(my_canister_id);
    bitstring.extend(seed);

    let mut der: Vec<u8> = vec![];
    // sequence of length 17 + the bit string length
    der.push(0x30);
    der.push(17 + bitstring.len() as u8);
    der.extend(vec![
        // sequence of length 12 for the OID
        0x30, 0x0C, // OID 1.3.6.1.4.1.56387.1.2
        0x06, 0x0A, 0x2B, 0x06, 0x01, 0x04, 0x01, 0x83, 0xB8, 0x43, 0x01, 0x02,
    ]);
    // BIT string of given length
    der.push(0x03);
    der.push(1 + bitstring.len() as u8);
    der.push(0x00);
    der.extend(bitstring);
    der
}