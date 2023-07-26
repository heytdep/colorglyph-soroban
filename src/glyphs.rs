// use std::println;
// extern crate std;

use crate::{
    contract::MAX_ENTRY_LIFETIME,
    types::{Error, Glyph, GlyphType, HashId, StorageKey},
};
use soroban_sdk::{
    contracttype, panic_with_error, xdr::ToXdr, Address, Bytes, BytesN, Env, Map, Vec,
};

// TODO
// Limit number of unique miner addresses in a mint `colors` Vec

pub const MAX_PAYMENT_COUNT: u8 = 15;

// TODO use PRNG to generate random ids
// Then use that id to hold the colors and add an additional store to hold the GlyphBox owner
// Note this really only helps GlyphBox transfers which is likely a rare case so probably not worth it
// Not fully verifying ownership atm

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
enum GlyphCraftType {
    Id(u64),
    Colors(Map<Address, Map<u32, Vec<u32>>>),
}

pub fn glyph_mint(
    env: &Env,
    minter: Address,
    to: Option<Address>,
    colors: Option<Map<Address, Map<u32, Vec<u32>>>>,
    width: Option<u32>,
    id: Option<u64>,
) -> HashId {
    minter.require_auth();

    // TODO allow for craft and build in a single call

    /*
    if `colors` we have some crafting to do
        if `width` we're also storing
            if `id` we're continuing a craft
    else we're just storing
        requires width and id
    */

    match colors {
        Some(colors) => {
            match width {
                // Craft and Store (quick mint)
                Some(width) => match glyph_craft(env, minter.clone(), colors, id, false) {
                    GlyphCraftType::Colors(colors) => {
                        glyph_store(env, minter, to, Some(colors), width, None)
                    }
                    _ => panic_with_error!(&env, Error::GroguDown),
                },
                // Craft (start or continue mint of Colors)
                None => match glyph_craft(env, minter, colors, id, true) {
                    GlyphCraftType::Id(id) => HashId::Id(id),
                    _ => panic_with_error!(&env, Error::GroguDown),
                },
            }
        }
        // Store (mint of crafted colors)
        None => match id {
            Some(id) => match width {
                Some(width) => glyph_store(env, minter, to, None, width, Some(id)),
                None => panic_with_error!(&env, Error::MissingWidth),
            },
            None => panic_with_error!(&env, Error::MissingId),
        },
    }
}

fn glyph_craft(
    env: &Env,
    minter: Address,
    colors: Map<Address, Map<u32, Vec<u32>>>,
    mut id: Option<u64>,
    store: bool,
) -> GlyphCraftType {
    let mut glyph_colors: Map<Address, Map<u32, Vec<u32>>> = Map::new(&env);

    match id {
        None => {
            if store {
                let mut id_ = env.ledger().timestamp();

                for byte in minter.clone().to_xdr(&env).iter() {
                    id_ = id_.wrapping_add(byte as u64);
                }

                id = Some(id_);
            }
        }
        Some(id_) => {
            let colors_key = StorageKey::Colors(id_);

            glyph_colors = env
                .storage()
                .persistent()
                .get::<StorageKey, Map<Address, Map<u32, Vec<u32>>>>(&colors_key)
                .unwrap_or_else(|| panic_with_error!(env, Error::NotFound));

            env.storage()
                .persistent()
                .bump(&colors_key, MAX_ENTRY_LIFETIME);
        }
    }

    // spend colors
    for (miner, color_indexes) in colors.iter() {
        let mut skip = false;

        for (color, indexes) in color_indexes.iter() {
            let miner_minter_color = StorageKey::Color(miner.clone(), minter.clone(), color);
            let current_amount = env
                .storage()
                .persistent()
                .get::<StorageKey, u32>(&miner_minter_color)
                .unwrap_or(0);

            env.storage()
                .persistent()
                .set(&miner_minter_color, &(current_amount - indexes.len()));

            env.storage()
                .persistent()
                .bump(&miner_minter_color, MAX_ENTRY_LIFETIME);

            if !skip {
                match glyph_colors.get(miner.clone()) {
                    Some(result) => match result {
                        mut color_indexes_ => match color_indexes_.get(color) {
                            // Exising miner and color
                            Some(result) => match result {
                                mut indexes_ => {
                                    for index in indexes.iter() {
                                        indexes_.push_back(index);
                                    }
                                    color_indexes_.set(color, indexes_);
                                    glyph_colors.set(miner.clone(), color_indexes_);
                                }
                            },
                            // No color
                            None => {
                                color_indexes_.set(color, indexes);
                                glyph_colors.set(miner.clone(), color_indexes_);
                            }
                        },
                    },
                    // No miner (or no exisiting glyphbox)
                    None => {
                        glyph_colors.set(miner.clone(), color_indexes.clone());
                        // We set a skip vs using break to ensure we continue to bill for the spent colors
                        skip = true; // we need to break here otherwise we continue looping inside this nested color loop which we've already fully added
                    }
                }
            }
        }
    }

    // store glyph
    if store {
        let id_ = id.unwrap();
        let colors_key = StorageKey::Colors(id_);

        env.storage().persistent().set(&colors_key, &glyph_colors);

        env.storage()
            .persistent()
            .bump(&colors_key, MAX_ENTRY_LIFETIME);

        GlyphCraftType::Id(id_)
    } else {
        GlyphCraftType::Colors(glyph_colors)
    }
}

fn glyph_store(
    env: &Env,
    minter: Address,
    to: Option<Address>,
    mut colors: Option<Map<Address, Map<u32, Vec<u32>>>>,
    width: u32,
    id: Option<u64>,
) -> HashId {
    if colors.is_none() {
        match id {
            Some(id) => {
                let colors_key = StorageKey::Colors(id);
                colors = Some(
                    env.storage()
                        .persistent()
                        .get::<StorageKey, Map<Address, Map<u32, Vec<u32>>>>(&colors_key)
                        .unwrap_or_else(|| panic_with_error!(env, Error::NotFound)),
                );

                env.storage()
                    .persistent()
                    .bump(&colors_key, MAX_ENTRY_LIFETIME)
            }
            None => panic_with_error!(env, Error::MissingId),
        }
    }

    let mut hash_data = Bytes::new(&env);

    // TODO
    // better error for not enough colors
    // should we error if there's a dupe index? (will result in burned colors)
    // Need to ensure hash gen is consistent when duping indexes or mixing in white/missing pixels
    // Should we enable some concept of ranging between 2 indexs vs listing out all the indexes? 0..=5 vs 0,1,2,3,4,5

    for (_, color_indexes) in colors.clone().unwrap().iter() {
        for (color, indexes) in color_indexes.iter() {
            // TODO
            // This is expensive and it's only for getting the sha256 hash. We should find a cheaper way to derive a hash from the Glyph colors themselves.
            // RawVal maybe?
            // Ordering is important so you can't just hash the arg directly
            // May be able to improve perf by ordering indexes (and maybe reversing them so we extend and then insert vs lots of inserts?)

            for index in indexes.iter() {
                // We need to extend the length of the palette
                if (hash_data.len() / 3) <= index {
                    // Start wherever we have data .. wherever we need data
                    for i in (hash_data.len() / 3)..=index {
                        // If this is the section we're interested in filling, just fill
                        if i == index {
                            let slice: [u8; 3] = color.to_le_bytes()[..3].try_into().unwrap();
                            hash_data.insert_from_slice(index * 3, &slice);
                        }
                        // Push empty white pixels
                        // NOTE: this is a "free" way to use white pixels atm
                        else {
                            hash_data.extend_from_slice(&[255; 3]);
                        }
                    }
                }
                // If the bytes already exist just fill them in
                else {
                    let slice: [u8; 3] = color.to_le_bytes()[..3].try_into().unwrap();
                    hash_data.copy_from_slice(index * 3, &slice);
                }
            }
        }
    }

    // NOTE
    // the hash should include something with the width. Otherwise two identical palettes with different widths would clash

    hash_data.extend_from_slice(&width.to_le_bytes());
    let hash = env.crypto().sha256(&hash_data);
    let glpyh_owner_key = StorageKey::GlyphOwner(hash.clone());

    // Glyph has already been minted and is currently owned (not scraped)
    if env.storage().persistent().has(&glpyh_owner_key) {
        panic_with_error!(env, Error::NotEmpty);
    }

    // Save the glyph owner to storage
    env.storage().persistent().set(
        &glpyh_owner_key,
        &match to {
            None => minter.clone(),
            Some(address) => address,
        },
    );

    env.storage()
        .persistent()
        .bump(&glpyh_owner_key, MAX_ENTRY_LIFETIME);

    // Save the glyph minter to storage (if glyph hasn't already been minted)
    let glyph_minter_key = StorageKey::GlyphMinter(hash.clone());

    if !env.storage().persistent().has(&glyph_minter_key) {
        env.storage().persistent().set(&glyph_minter_key, &minter);

        env.storage()
            .persistent()
            .bump(&glyph_minter_key, MAX_ENTRY_LIFETIME);
    }

    // Save the glyph to storage
    let glyph_key = StorageKey::Glyph(hash.clone());

    env.storage().persistent().set(
        &glyph_key,
        &Glyph {
            width,
            length: (hash_data.len() - 4) / 3, // -4 because we're appending width, /3 because there are 3 u8 values per u32 color
            colors: colors.clone().unwrap(),
        },
    );

    env.storage()
        .persistent()
        .bump(&glyph_key, MAX_ENTRY_LIFETIME);

    // Remove the temp Colors
    if id.is_some() {
        env.storage()
            .persistent()
            .remove(&StorageKey::Colors(id.unwrap()));
    }

    HashId::Hash(hash)
}

pub fn glyph_transfer(env: &Env, from: Address, to: Address, hash_id: HashId) -> Option<u64> {
    from.require_auth();

    match hash_id {
        HashId::Hash(hash) => {
            glyph_verify_ownership(env, from.clone(), hash.clone());

            let glyph_owner_key = StorageKey::GlyphOwner(hash);

            env.storage().persistent().set(&glyph_owner_key, &to);

            env.storage()
                .persistent()
                .bump(&glyph_owner_key, MAX_ENTRY_LIFETIME);

            None
        }
        HashId::Id(id) => {
            // TODO We need to ensure ownership of the Colors which I don't think we can atm

            let from_colors_key = StorageKey::Colors(id);
            let colors = env
                .storage()
                .persistent()
                .get::<StorageKey, Map<Address, Map<u32, Vec<u32>>>>(&from_colors_key)
                .unwrap_or_else(|| panic_with_error!(env, Error::NotFound));

            // TODO This is a pretty expensive transfer. Separating StorageKey::Colors from maybe a StorageKey::ColorsOwner might be the better way to go

            env.storage().persistent().remove(&from_colors_key);

            let mut id_ = env.ledger().timestamp();

            for byte in to.clone().to_xdr(&env).iter() {
                id_ = id_.wrapping_add(byte as u64);
            }

            let to_colors_key = StorageKey::Colors(id);

            env.storage().persistent().set(&to_colors_key, &colors);

            env.storage()
                .persistent()
                .bump(&to_colors_key, MAX_ENTRY_LIFETIME);

            Some(id_)
        }
    }
}

pub fn glyph_scrape(
    env: &Env,
    owner: Address,
    to: Option<Address>,
    hash_id: HashId,
) -> Option<u64> {
    owner.require_auth();

    let mut miners_colors_indexes: Map<Address, Map<u32, Vec<u32>>>;
    let mut id_u64: u64;
    let id: Option<u64>;

    match &hash_id {
        HashId::Hash(hash) => {
            glyph_verify_ownership(env, owner.clone(), hash.clone());

            let glyph_key = StorageKey::Glyph(hash.clone());
            let glyph = env
                .storage()
                .persistent()
                .get::<StorageKey, Glyph>(&glyph_key)
                .unwrap_or_else(|| panic_with_error!(env, Error::NotFound));

            // TODO remove these `has` checks in the next release. `remove` should not break
            // https://discord.com/channels/897514728459468821/1129494558829465671

            // Remove glyph
            env.storage().persistent().remove(&glyph_key);

            // Remove glyph owner
            env.storage()
                .persistent()
                .remove(&StorageKey::GlyphOwner(hash.clone()));

            // Remove all glyph sell offers
            env.storage()
                .persistent()
                .remove(&StorageKey::GlyphOffer(hash.clone()));

            miners_colors_indexes = glyph.colors;

            id_u64 = env.ledger().timestamp();

            for byte in owner.clone().to_xdr(&env).iter() {
                id_u64 = id_u64.wrapping_add(byte as u64);
            }
        }
        HashId::Id(id_) => {
            let colors_key = StorageKey::Colors(id_.clone());

            miners_colors_indexes = env
                .storage()
                .persistent()
                .get::<StorageKey, Map<Address, Map<u32, Vec<u32>>>>(&colors_key)
                .unwrap_or_else(|| panic_with_error!(env, Error::NotFound));

            env.storage()
                .persistent()
                .bump(&colors_key, MAX_ENTRY_LIFETIME);

            id_u64 = id_.clone();
        }
    }

    // loop through the glyph colors and send them to `to`
    let mut payment_count: u8 = 0;

    for (miner, mut colors_indexes) in miners_colors_indexes.iter() {
        if payment_count >= MAX_PAYMENT_COUNT {
            break;
        }

        for (color, indexes) in colors_indexes.iter() {
            if payment_count >= MAX_PAYMENT_COUNT {
                break;
            }

            let miner_owner_color = StorageKey::Color(
                miner.clone(),
                match to.clone() {
                    None => owner.clone(),
                    Some(address) => address,
                },
                color,
            );
            let current_amount = env
                .storage()
                .persistent()
                .get::<StorageKey, u32>(&miner_owner_color)
                .unwrap_or(0);

            env.storage()
                .persistent()
                .set(&miner_owner_color, &(current_amount + indexes.len()));

            env.storage()
                .persistent()
                .bump(&miner_owner_color, MAX_ENTRY_LIFETIME);

            colors_indexes.remove(color);

            payment_count += 1;
        }

        if colors_indexes.len() == 0 {
            miners_colors_indexes.remove(miner);
        } else {
            miners_colors_indexes.set(miner, colors_indexes);
        }
    }

    if miners_colors_indexes.len() == 0 {
        match &hash_id {
            HashId::Id(id) => {
                env.storage()
                    .persistent()
                    .remove(&StorageKey::Colors(id.clone()));
            }
            _ => {}
        }

        id = None;
    } else {
        // save glyph
        let colors_key = StorageKey::Colors(id_u64);

        env.storage()
            .persistent()
            .set(&colors_key, &miners_colors_indexes);

        env.storage()
            .persistent()
            .bump(&colors_key, MAX_ENTRY_LIFETIME);

        id = Some(id_u64);
    }

    id
}

pub fn glyph_get(env: &Env, hash_id: HashId) -> Result<GlyphType, Error> {
    match hash_id {
        HashId::Hash(hash) => {
            let glyph_key = StorageKey::Glyph(hash);
            let glyph = env
                .storage()
                .persistent()
                .get::<StorageKey, Glyph>(&glyph_key)
                .unwrap_or_else(|| panic_with_error!(env, Error::NotFound));

            env.storage()
                .persistent()
                .bump(&glyph_key, MAX_ENTRY_LIFETIME);

            Ok(GlyphType::Glyph(glyph))
        }
        HashId::Id(id) => {
            let colors_key = StorageKey::Colors(id);
            let colors = env
                .storage()
                .persistent()
                .get::<StorageKey, Map<Address, Map<u32, Vec<u32>>>>(&colors_key)
                .unwrap_or_else(|| panic_with_error!(env, Error::NotFound));

            env.storage()
                .persistent()
                .bump(&colors_key, MAX_ENTRY_LIFETIME);

            Ok(GlyphType::Colors(colors))
        }
    }
}

pub fn glyph_verify_ownership(env: &Env, from: Address, glyph_hash: BytesN<32>) {
    let glyph_owner_key = StorageKey::GlyphOwner(glyph_hash);
    let glyph_owner = env
        .storage()
        .persistent()
        .get::<StorageKey, Address>(&glyph_owner_key)
        .unwrap_or_else(|| panic_with_error!(env, Error::NotFound));

    env.storage()
        .persistent()
        .bump(&glyph_owner_key, MAX_ENTRY_LIFETIME);

    if glyph_owner != from {
        panic_with_error!(env, Error::NotAuthorized);
    }
}
