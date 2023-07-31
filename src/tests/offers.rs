#![cfg(test)]

use std::println;
extern crate std;

use fixed_point_math::FixedPoint;
use soroban_sdk::{map, testutils::Address as _, token, vec, Address, Env, Map, Vec};

use crate::{
    contract::{ColorGlyph, ColorGlyphClient},
    types::{Error, HashType, Offer, StorageKey},
};

const ITERS: i128 = 10i128;

#[test]
fn test_self_purchase() {
    let env = Env::default();

    env.mock_all_auths();
    env.budget().reset_unlimited();

    // Contract
    let contract_address = env.register_contract(None, ColorGlyph);
    let client = ColorGlyphClient::new(&env, &contract_address);

    // Token
    let token_admin = Address::random(&env);
    let token_address = env.register_stellar_asset_contract(token_admin.clone());
    let token_admin_client = token::AdminClient::new(&env, &token_address);

    // Accounts
    let u1_address = Address::random(&env);
    let u2_address = Address::random(&env);
    let fee_address = Address::random(&env);

    token_admin_client.mint(&u1_address, &10_000);
    token_admin_client.mint(&u2_address, &10_000);

    client.initialize(&token_address, &fee_address);

    // Tests
    let mut colors_indexes: Map<u32, Vec<u32>> = Map::new(&env);
    let mut color_amount: Map<u32, u32> = Map::new(&env);

    for i in 0..ITERS {
        let hex = 16777215i128.fixed_div_floor(ITERS - 1, i).unwrap(); // 0 - 16777215 (black to white)

        colors_indexes.set(hex as u32, vec![&env, i as u32]);
        color_amount.set(hex as u32, 1);
    }

    client.colors_mine(&u1_address, &None, &color_amount);

    let hash = client
        .glyph_mint(
            &u1_address,
            &None,
            &map![&env, (u1_address.clone(), colors_indexes.clone())],
            &Some(16),
        )
        .unwrap();

    // Real Tests
    let amount: i128 = 100;
    let glyph = Offer::Glyph(hash.clone());
    let asset = Offer::Asset(token_address.clone(), amount);
    let asset_sell = Offer::AssetSell(u1_address.clone(), token_address.clone(), amount);

    client.offer_post(&glyph, &asset);
    client.offer_post(&asset_sell, &glyph);

    client.offer_post(&glyph, &glyph);

    client.offers_get(&glyph, &Some(glyph.clone()));

    client.offer_post(&glyph, &glyph.clone());

    assert_eq!(
        client.try_offers_get(&glyph, &Some(glyph.clone())),
        Err(Ok(Error::NotFound))
    );

    // NOTE self purchases are possible. Not sure how I feel about this. Probably fine?
}

#[test]
fn test_sell_scrape_buy() {
    let env = Env::default();

    env.mock_all_auths();
    env.budget().reset_unlimited();

    // Contract
    let contract_address = env.register_contract(None, ColorGlyph);
    let client = ColorGlyphClient::new(&env, &contract_address);

    // Token
    let token_admin = Address::random(&env);
    let token_address = env.register_stellar_asset_contract(token_admin.clone());
    let token_admin_client = token::AdminClient::new(&env, &token_address);

    // Accounts
    let u1_address = Address::random(&env);
    let u2_address = Address::random(&env);
    let fee_address = Address::random(&env);

    token_admin_client.mint(&u1_address, &10_000);
    token_admin_client.mint(&u2_address, &10_000);

    client.initialize(&token_address, &fee_address);

    // Tests
    let mut colors_indexes: Map<u32, Vec<u32>> = Map::new(&env);
    let mut color_amount: Map<u32, u32> = Map::new(&env);

    for i in 0..ITERS {
        let hex = 16777215i128.fixed_div_floor(ITERS, i).unwrap(); // 0 - 16777215 (black to white)

        colors_indexes.set(hex as u32, vec![&env, i as u32]);
        color_amount.set(hex as u32, 1);
    }

    client.colors_mine(&u1_address, &None, &color_amount);

    let hash = client
        .glyph_mint(
            &u1_address,
            &None,
            &map![&env, (u1_address.clone(), colors_indexes.clone())],
            &Some(16),
        )
        .unwrap();

    // Real Tests
    let amount: i128 = 100;
    let glyph = Offer::Glyph(hash.clone());
    let asset = Offer::Asset(token_address.clone(), amount);
    let asset_sell = Offer::AssetSell(u2_address.clone(), token_address.clone(), amount);

    client.offer_post(&glyph, &asset);

    client.glyph_scrape(&None, &HashType::Glyph(hash.clone()));

    assert_eq!(
        client.try_glyph_get(&HashType::Colors(u1_address.clone())),
        Err(Ok(Error::NotFound))
    );

    assert_eq!(
        client.try_glyph_get(&HashType::Dust(u1_address.clone())),
        Err(Ok(Error::NotFound))
    );

    assert_eq!(
        client.try_glyph_get(&HashType::Glyph(hash.clone())),
        Err(Ok(Error::NotFound))
    );

    assert_eq!(
        client.try_offers_get(&glyph, &None),
        Err(Ok(Error::NotFound))
    );

    client.offer_post(&asset_sell, &glyph);

    client.offers_get(&asset, &Some(glyph.clone()));

    client
        .glyph_mint(
            &u1_address,
            &None,
            &map![&env, (u1_address.clone(), colors_indexes)],
            &Some(16),
        )
        .unwrap();

    client.offer_post(&glyph, &asset);

    assert_eq!(
        client.try_offers_get(&asset, &Some(glyph.clone())),
        Err(Ok(Error::NotFound))
    );

    assert_eq!(
        client.try_offers_get(&glyph, &Some(asset)),
        Err(Ok(Error::NotFound))
    );

    env.as_contract(&contract_address, || {
        let res = env
            .storage()
            .persistent()
            .get::<StorageKey, Address>(&StorageKey::GlyphOwner(hash.clone()))
            .unwrap();

        assert_eq!(res, u2_address);
    });
}

#[test]
fn test_dupe() {
    let env = Env::default();

    env.mock_all_auths();
    env.budget().reset_unlimited();

    // Contract
    let contract_address = env.register_contract(None, ColorGlyph);
    let client = ColorGlyphClient::new(&env, &contract_address);

    // Token
    let token_admin = Address::random(&env);
    let token_address = env.register_stellar_asset_contract(token_admin.clone());
    let token_admin_client = token::AdminClient::new(&env, &token_address);

    // Accounts
    let u1_address = Address::random(&env);
    let u2_address = Address::random(&env);
    let fee_address = Address::random(&env);

    token_admin_client.mint(&u1_address, &10_000);
    token_admin_client.mint(&u2_address, &10_000);

    client.initialize(&token_address, &fee_address);

    // Tests
    let mut colors_indexes: Map<u32, Vec<u32>> = Map::new(&env);
    let mut color_amount: Map<u32, u32> = Map::new(&env);

    for i in 0..ITERS {
        let hex = 16777215i128.fixed_div_floor(ITERS, i).unwrap(); // 0 - 16777215 (black to white)

        colors_indexes.set(hex as u32, vec![&env, i as u32]);
        color_amount.set(hex as u32, 1);
    }

    client.colors_mine(&u1_address, &None, &color_amount);

    let hash = client
        .glyph_mint(
            &u1_address,
            &None,
            &map![&env, (u1_address.clone(), colors_indexes)],
            &Some(16),
        )
        .unwrap();

    // Real Tests
    let amount: i128 = 100;
    let glyph = Offer::Glyph(hash.clone());
    let asset = Offer::Asset(token_address.clone(), amount);
    let asset_sell = Offer::AssetSell(u2_address.clone(), token_address.clone(), amount);

    client.offer_post(&glyph, &asset);

    assert_eq!(
        client.try_offer_post(&glyph, &asset),
        Err(Ok(Error::NotEmpty))
    );

    client.offer_delete(&glyph, &None); // <- delete all open glyph sell offers

    client.offer_post(&asset_sell, &glyph);

    assert_eq!(
        client.try_offer_post(&asset_sell, &glyph),
        Err(Ok(Error::NotEmpty))
    );
}

#[test]
fn test_buy_glyph() {
    let env = Env::default();

    env.mock_all_auths();
    env.budget().reset_unlimited();

    // Contract
    let contract_address = env.register_contract(None, ColorGlyph);
    let client = ColorGlyphClient::new(&env, &contract_address);

    // Token
    let token_admin = Address::random(&env);
    let token_address = env.register_stellar_asset_contract(token_admin.clone());
    let token_admin_client = token::AdminClient::new(&env, &token_address);
    let token_client = token::Client::new(&env, &token_address);

    // Accounts
    let u1_address = Address::random(&env);
    let u2_address = Address::random(&env);
    let u3_address = Address::random(&env);
    let fee_address = Address::random(&env);

    token_admin_client.mint(&u1_address, &10_000);
    token_admin_client.mint(&u2_address, &10_000);
    token_admin_client.mint(&u3_address, &10_000);

    client.initialize(&token_address, &fee_address);

    // Tests
    let mut colors_indexes: Map<u32, Vec<u32>> = Map::new(&env);
    let mut color_amount: Map<u32, u32> = Map::new(&env);

    for i in 0..ITERS {
        let hex = 16777215i128.fixed_div_floor(ITERS, i).unwrap(); // 0 - 16777215 (black to white)

        colors_indexes.set(hex as u32, vec![&env, i as u32]);
        color_amount.set(hex as u32, 1);
    }

    client.colors_mine(&u3_address, &Some(u1_address.clone()), &color_amount);

    // println!("{:?}\n", colors_indexes);

    client.glyph_mint(
        &u1_address,
        &None,
        &map![&env, (u3_address.clone(), colors_indexes)],
        &None,
    );

    let hash = client
        .glyph_mint(&u1_address, &None, &map![&env], &Some(16))
        .unwrap();

    println!("{:?}\n", hash);

    // Real Tests
    let amount: i128 = 100;
    let glyph = Offer::Glyph(hash.clone());
    let asset = Offer::Asset(token_address.clone(), amount);
    let asset_sell = Offer::AssetSell(u2_address.clone(), token_address.clone(), amount);

    client.offer_post(&asset_sell, &glyph);
    client.offers_get(&asset_sell, &Some(glyph.clone())); // User 2 is selling
    client.offers_get(&asset, &Some(glyph.clone())); // Someone is selling

    assert_eq!(
        // User 1 is NOT selling
        client.try_offers_get(
            &Offer::AssetSell(u1_address.clone(), token_address.clone(), amount),
            &Some(glyph.clone())
        ),
        Err(Ok(Error::NotFound))
    );

    client.offer_post(&glyph, &asset);

    // env.budget().print();

    env.as_contract(&contract_address, || {
        let res = env
            .storage()
            .persistent()
            .get::<StorageKey, Address>(&StorageKey::GlyphOwner(hash.clone()))
            .unwrap();

        assert_eq!(res, u2_address);
    });

    assert_eq!(
        client.try_offers_get(&asset, &Some(glyph.clone())),
        Err(Ok(Error::NotFound))
    );

    assert_eq!(
        client.try_offers_get(&glyph, &Some(asset)),
        Err(Ok(Error::NotFound))
    );

    assert_eq!(token_client.balance(&fee_address), 10i128);
    assert_eq!(token_client.balance(&contract_address), 0i128);
    assert_eq!(token_client.balance(&u1_address), 10_098i128);
    assert_eq!(token_client.balance(&u2_address), 9_900i128);
    assert_eq!(token_client.balance(&u3_address), 9_992i128);
}

#[test]
fn test_sell_glyph() {
    let env = Env::default();

    env.mock_all_auths();

    // Contract
    let contract_address = env.register_contract(None, ColorGlyph);
    let client = ColorGlyphClient::new(&env, &contract_address);

    // Token
    let token_admin = Address::random(&env);
    let token_address = env.register_stellar_asset_contract(token_admin.clone());
    let token_admin_client = token::AdminClient::new(&env, &token_address);
    let token_client = token::Client::new(&env, &token_address);

    // Accounts
    let u1_address = Address::random(&env);
    let u2_address = Address::random(&env);
    let u3_address = Address::random(&env);
    let fee_address = Address::random(&env);

    token_admin_client.mint(&u1_address, &10_000);
    token_admin_client.mint(&u2_address, &10_000);
    token_admin_client.mint(&u3_address, &10_000);

    client.initialize(&token_address, &fee_address);

    // Tests
    env.budget().reset_default();

    let mut colors_indexes: Map<u32, Vec<u32>> = Map::new(&env);
    let mut color_amount: Map<u32, u32> = Map::new(&env);

    for i in 0..ITERS {
        let hex = 16777215i128.fixed_div_floor(ITERS, i).unwrap(); // 0 - 16777215 (black to white)

        colors_indexes.set(hex as u32, vec![&env, i as u32]);
        color_amount.set(hex as u32, 1);
    }

    client.colors_mine(&u3_address, &Some(u1_address.clone()), &color_amount);

    client.glyph_mint(
        &u1_address,
        &None,
        &map![&env, (u3_address.clone(), colors_indexes)],
        &None,
    );

    let hash = client
        .glyph_mint(&u1_address, &None, &map![&env], &Some(16))
        .unwrap();

    env.budget().reset_default();

    // Real Tests
    let amount: i128 = 100;
    let glyph = Offer::Glyph(hash.clone());
    let asset = Offer::Asset(token_address.clone(), amount);
    let asset_sell = Offer::AssetSell(u2_address.clone(), token_address.clone(), amount);

    client.offer_post(&glyph, &asset);

    client.offer_post(&asset_sell, &glyph);

    env.as_contract(&contract_address, || {
        let res = env
            .storage()
            .persistent()
            .get::<StorageKey, Address>(&StorageKey::GlyphOwner(hash.clone()))
            .unwrap();

        assert_eq!(res, u2_address);
    });

    assert_eq!(
        client.try_offers_get(&asset, &Some(glyph.clone())),
        Err(Ok(Error::NotFound))
    );

    assert_eq!(
        client.try_offers_get(&asset, &Some(glyph.clone())),
        Err(Ok(Error::NotFound))
    );

    // env.logger().print();

    assert_eq!(token_client.balance(&contract_address), 0i128);
    assert_eq!(token_client.balance(&u1_address), 10_098i128);
    assert_eq!(token_client.balance(&u2_address), 9_900i128);
    assert_eq!(token_client.balance(&u3_address), 9_992i128);
}

#[test]
fn test_swap_glyph() {
    let env = Env::default();

    env.mock_all_auths();

    // Contract
    let contract_address = env.register_contract(None, ColorGlyph);
    let client = ColorGlyphClient::new(&env, &contract_address);

    // Token
    let token_admin = Address::random(&env);
    let token_address = env.register_stellar_asset_contract(token_admin.clone());
    let token_admin_client = token::AdminClient::new(&env, &token_address);

    // Accounts
    let u1_address = Address::random(&env);
    let u2_address = Address::random(&env);
    let fee_address = Address::random(&env);

    token_admin_client.mint(&u1_address, &10_000);
    token_admin_client.mint(&u2_address, &10_000);

    client.initialize(&token_address, &fee_address);

    // Tests
    env.budget().reset_default();

    let mut colors_a_indexes: Map<u32, Vec<u32>> = Map::new(&env);
    let mut colors_b_indexes: Map<u32, Vec<u32>> = Map::new(&env);
    let mut colors_a_amount: Map<u32, u32> = Map::new(&env);
    let mut colors_b_amount: Map<u32, u32> = Map::new(&env);

    for i in 0..ITERS {
        let hex_a = 16777215i128.fixed_div_floor(ITERS, i).unwrap(); // 0 - 16777215 (black to white)
        let hex_b = 16777215i128.fixed_div_floor(ITERS, i + 1).unwrap();

        colors_a_indexes.set(hex_a as u32, vec![&env, i as u32]);
        colors_b_indexes.set(hex_b as u32, vec![&env, i as u32]);
        colors_a_amount.set(hex_a as u32, 1);
        colors_b_amount.set(hex_b as u32, 1);
    }

    client.colors_mine(&u1_address, &None, &colors_a_amount);

    client.glyph_mint(
        &u1_address,
        &None,
        &map![&env, (u1_address.clone(), colors_a_indexes)],
        &None,
    );

    let hash_a = client
        .glyph_mint(&u1_address, &None, &map![&env], &Some(16))
        .unwrap();

    client.colors_mine(&u2_address, &None, &colors_b_amount);

    client.glyph_mint(
        &u2_address,
        &None,
        &map![&env, (u2_address.clone(), colors_b_indexes)],
        &None,
    );

    let hash_b = client
        .glyph_mint(&u2_address, &None, &map!(&env), &Some(16))
        .unwrap();

    env.budget().reset_default();

    // Real Tests
    let glyph_1 = Offer::Glyph(hash_a.clone());
    let glyph_2 = Offer::Glyph(hash_b.clone());

    client.offer_post(&glyph_1, &glyph_2);

    client.offer_post(&glyph_2, &glyph_1);

    env.as_contract(&contract_address, || {
        let res_a = env
            .storage()
            .persistent()
            .get::<StorageKey, Address>(&StorageKey::GlyphOwner(hash_a.clone()))
            .unwrap();

        assert_eq!(res_a, u2_address);

        let res_b = env
            .storage()
            .persistent()
            .get::<StorageKey, Address>(&StorageKey::GlyphOwner(hash_b.clone()))
            .unwrap();

        assert_eq!(res_b, u1_address);
    });

    assert_eq!(
        client.try_offers_get(&glyph_1, &Some(glyph_2.clone())),
        Err(Ok(Error::NotFound))
    );

    assert_eq!(
        client.try_offers_get(&glyph_2, &Some(glyph_1)),
        Err(Ok(Error::NotFound))
    );
}

#[test]
fn test_rm_glyph_buy() {
    let env = Env::default();

    env.mock_all_auths();

    // Contract
    let contract_address = env.register_contract(None, ColorGlyph);
    let client = ColorGlyphClient::new(&env, &contract_address);

    // Token
    let token_admin = Address::random(&env);
    let token_address = env.register_stellar_asset_contract(token_admin.clone());
    let token_admin_client = token::AdminClient::new(&env, &token_address);
    let token_client = token::Client::new(&env, &token_address);

    // Accounts
    let u1_address = Address::random(&env);
    let fee_address = Address::random(&env);

    token_admin_client.mint(&u1_address, &10_000);

    client.initialize(&token_address, &fee_address);

    // Tests
    env.budget().reset_default();

    let mut colors_indexes: Map<u32, Vec<u32>> = Map::new(&env);
    let mut color_amount: Map<u32, u32> = Map::new(&env);

    for i in 0..ITERS {
        let hex = 16777215i128.fixed_div_floor(ITERS, i).unwrap(); // 0 - 16777215 (black to white)

        colors_indexes.set(hex as u32, vec![&env, i as u32]);
        color_amount.set(hex as u32, 1);
    }

    client.colors_mine(&u1_address, &None, &color_amount);

    client.glyph_mint(
        &u1_address,
        &None,
        &map![&env, (u1_address.clone(), colors_indexes)],
        &None,
    );

    let hash = client
        .glyph_mint(&u1_address, &None, &map![&env], &Some(16))
        .unwrap();

    env.budget().reset_default();

    // Real Tests
    let amount: i128 = 1;
    let glyph = Offer::Glyph(hash.clone());
    let asset = Offer::Asset(token_address.clone(), amount);
    let asset_sell = Offer::AssetSell(u1_address.clone(), token_address.clone(), amount);

    client.offer_post(&asset_sell, &glyph);

    assert_eq!(token_client.balance(&contract_address), 1i128);

    assert_eq!(token_client.balance(&u1_address), 9_989i128);

    client.offers_get(&asset, &Some(glyph.clone()));

    client.offer_delete(&asset_sell, &Some(glyph.clone()));

    assert_eq!(
        client.try_offers_get(&asset, &Some(glyph.clone())),
        Err(Ok(Error::NotFound))
    );

    assert_eq!(token_client.balance(&contract_address), 0i128);

    assert_eq!(token_client.balance(&u1_address), 9990i128);
}

#[test]
fn test_rm_glyph_sell() {
    let env = Env::default();

    env.mock_all_auths();

    // Contract
    let contract_address = env.register_contract(None, ColorGlyph);
    let client = ColorGlyphClient::new(&env, &contract_address);

    // Token
    let token_admin = Address::random(&env);
    let token_address = env.register_stellar_asset_contract(token_admin.clone());
    let token_admin_client = token::AdminClient::new(&env, &token_address);
    let token_client = token::Client::new(&env, &token_address);

    // Accounts
    let u1_address = Address::random(&env);
    let fee_address = Address::random(&env);

    token_admin_client.mint(&u1_address, &10_000);

    client.initialize(&token_address, &fee_address);

    // Tests
    env.budget().reset_default();

    let mut colors_indexes: Map<u32, Vec<u32>> = Map::new(&env);
    let mut color_amount: Map<u32, u32> = Map::new(&env);

    for i in 0..ITERS {
        let hex = 16777215i128.fixed_div_floor(ITERS, i).unwrap(); // 0 - 16777215 (black to white)

        colors_indexes.set(hex as u32, vec![&env, i as u32]);
        color_amount.set(hex as u32, 1);
    }

    client.colors_mine(&u1_address, &None, &color_amount);

    client.glyph_mint(
        &u1_address,
        &None,
        &map![&env, (u1_address.clone(), colors_indexes)],
        &None,
    );

    let hash = client
        .glyph_mint(&u1_address, &None, &map![&env], &Some(16))
        .unwrap();

    env.budget().reset_default();

    // Real Tests
    let glyph = Offer::Glyph(hash.clone());
    let asset = Offer::Asset(token_address.clone(), 1i128);

    client.offer_post(&glyph, &asset);

    client.offers_get(&glyph, &Some(asset.clone()));

    client.offer_delete(&glyph, &Some(asset.clone()));

    assert_eq!(
        client.try_offers_get(&glyph, &Some(asset.clone())),
        Err(Ok(Error::NotFound))
    );

    assert_eq!(token_client.balance(&contract_address), 0i128);

    assert_eq!(token_client.balance(&u1_address), 9990i128);
}

#[test]
fn test_rm_glyph_swap() {
    let env = Env::default();

    env.mock_all_auths();

    // Contract
    let contract_address = env.register_contract(None, ColorGlyph);
    let client = ColorGlyphClient::new(&env, &contract_address);

    // Token
    let token_admin = Address::random(&env);
    let token_address = env.register_stellar_asset_contract(token_admin.clone());
    let token_admin_client = token::AdminClient::new(&env, &token_address);
    let token_client = token::Client::new(&env, &token_address);

    // Accounts
    let u1_address = Address::random(&env);
    let u2_address = Address::random(&env);
    let fee_address = Address::random(&env);

    token_admin_client.mint(&u1_address, &10_000);
    token_admin_client.mint(&u2_address, &10_000);

    client.initialize(&token_address, &fee_address);

    // Tests
    env.budget().reset_default();

    let mut colors_a_indexes: Map<u32, Vec<u32>> = Map::new(&env);
    let mut colors_b_indexes: Map<u32, Vec<u32>> = Map::new(&env);
    let mut colors_a_amount: Map<u32, u32> = Map::new(&env);
    let mut colors_b_amount: Map<u32, u32> = Map::new(&env);

    for i in 0..ITERS {
        let hex_a = 16777215i128.fixed_div_floor(ITERS, i).unwrap(); // 0 - 16777215 (black to white)
        let hex_b = 16777215i128.fixed_div_floor(ITERS, i + 1).unwrap();

        colors_a_indexes.set(hex_a as u32, vec![&env, i as u32]);
        colors_b_indexes.set(hex_b as u32, vec![&env, i as u32]);
        colors_a_amount.set(hex_a as u32, 1);
        colors_b_amount.set(hex_b as u32, 1);
    }

    client.colors_mine(&u1_address, &None, &colors_a_amount);

    client.glyph_mint(
        &u1_address,
        &None,
        &map![&env, (u1_address.clone(), colors_a_indexes)],
        &None,
    );

    let hash_a = client
        .glyph_mint(&u1_address, &None, &map![&env], &Some(16))
        .unwrap();

    client.colors_mine(&u1_address, &Some(u2_address.clone()), &colors_b_amount);

    client.glyph_mint(
        &u2_address,
        &None,
        &map![&env, (u1_address.clone(), colors_b_indexes)],
        &None,
    );

    let hash_b = client
        .glyph_mint(&u2_address, &None, &map![&env], &Some(16))
        .unwrap();

    env.budget().reset_default();

    // Real Tests
    let glyph_a = Offer::Glyph(hash_a.clone());
    let glyph_b = Offer::Glyph(hash_b.clone());

    client.offer_post(&glyph_a, &glyph_b);

    client.offers_get(&glyph_a, &Some(glyph_b.clone()));

    client.offer_delete(&glyph_a, &Some(glyph_b.clone()));

    assert_eq!(
        client.try_offers_get(&glyph_a, &Some(glyph_b)),
        Err(Ok(Error::NotFound))
    );

    assert_eq!(token_client.balance(&contract_address), 0i128);

    assert_eq!(token_client.balance(&u1_address), 9980i128);
}
