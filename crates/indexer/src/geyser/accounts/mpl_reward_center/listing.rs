use std::str::FromStr;

use indexer_core::{
    db::{
        insert_into,
        models::{
            AuctionHouse, CurrentMetadataOwner, Listing as DbListing,
            RewardsListing as DbRewardsListing,
        },
        mutations,
        tables::{
            auction_houses, current_metadata_owners, metadatas, reward_centers, rewards_listings,
        },
    },
    prelude::*,
    pubkeys, util,
};
use mpl_auction_house::pda::find_auctioneer_trade_state_address;
use mpl_reward_center::state::Listing;
use solana_program::pubkey::Pubkey;

use super::super::Client;
use crate::prelude::*;

pub(crate) async fn process(
    client: &Client,
    key: Pubkey,
    account_data: Listing,
    slot: u64,
    write_version: u64,
) -> Result<()> {
    let row = DbRewardsListing {
        address: Owned(bs58::encode(key).into_string()),
        is_initialized: account_data.is_initialized,
        reward_center_address: Owned(bs58::encode(account_data.reward_center).into_string()),
        seller: Owned(bs58::encode(account_data.seller).into_string()),
        metadata: Owned(bs58::encode(account_data.metadata).into_string()),
        price: account_data
            .price
            .try_into()
            .context("Price is too big to store")?,
        token_size: account_data
            .token_size
            .try_into()
            .context("Token size is too big to store")?,
        created_at: util::unix_timestamp(account_data.created_at)?,
        canceled_at: account_data
            .canceled_at
            .map(util::unix_timestamp)
            .transpose()?,
        purchase_ticket: account_data.purchase_ticket.map(|p| Owned(p.to_string())),
        bump: account_data.bump.try_into()?,
        slot: slot.try_into()?,
        write_version: write_version.try_into()?,
    };

    client
        .db()
        .run({
            let rewards_listing = row.clone();
            let values = row.clone();
            move |db| {
                let auction_houses = auction_houses::table
                    .select(auction_houses::all_columns)
                    .inner_join(
                        reward_centers::table
                            .on(auction_houses::address.eq(reward_centers::auction_house)),
                    )
                    .filter(reward_centers::address.eq(row.reward_center_address))
                    .first::<AuctionHouse>(db)?;

                let current_metadata_owner = current_metadata_owners::table
                    .select((
                        current_metadata_owners::mint_address,
                        current_metadata_owners::owner_address,
                        current_metadata_owners::token_account_address,
                        current_metadata_owners::slot,
                    ))
                    .inner_join(
                        metadatas::table
                            .on(metadatas::mint_address.eq(current_metadata_owners::mint_address)),
                    )
                    .filter(metadatas::address.eq(row.metadata))
                    .first::<CurrentMetadataOwner>(db)?;

                let (trade_state, trade_state_bump) = find_auctioneer_trade_state_address(
                    &account_data.seller,
                    &Pubkey::from_str(&auction_houses.address)?,
                    &Pubkey::from_str(&current_metadata_owner.token_account_address)?,
                    &Pubkey::from_str(&auction_houses.treasury_mint)?,
                    &Pubkey::from_str(&current_metadata_owner.mint_address)?,
                    account_data.token_size,
                );

                let listing = DbListing {
                    id: None,
                    trade_state: Owned(bs58::encode(trade_state).into_string()),
                    trade_state_bump: trade_state_bump.into(),
                    auction_house: auction_houses.address,
                    metadata: values.metadata,
                    token_size: values.token_size,
                    marketplace_program: Owned(pubkeys::REWARD_CENTER.to_string()),
                    purchase_id: None,
                    seller: values.seller,
                    price: values.price,
                    created_at: values.created_at,
                    expiry: None,
                    canceled_at: values.canceled_at,
                    write_version: Some(values.write_version),
                    slot: values.slot,
                };

                db.build_transaction().read_write().run(|| {
                    {
                        insert_into(rewards_listings::table)
                            .values(&rewards_listing)
                            .on_conflict(rewards_listings::address)
                            .do_update()
                            .set(&rewards_listing)
                            .execute(db)?;

                        mutations::listing::insert(db, &listing)?;
                    }

                    Result::<_>::Ok(())
                })
            }
        })
        .await
        .context("Failed to insert rewards listing")?;

    Ok(())
}
