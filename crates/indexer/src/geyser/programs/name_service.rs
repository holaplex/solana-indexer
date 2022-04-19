use super::{accounts::name_service, AccountUpdate, Client};
use crate::prelude::*;
const HEADER_LENGTH: usize = 96;
use borsh::BorshDeserialize;
use solana_program::pubkey::Pubkey;

mod ids {
    #![allow(missing_docs)]
    use solana_sdk::pubkeys;
    pubkeys!(
        twitter_verification_authority,
        "FvPH7PrVrLGKPfqaf3xJodFTjZriqrAXXLTVWEorTFBi"
    );
    pubkeys!(
        twitter_root_parent_registry_key,
        "4YcexoW3r78zz16J2aqmukBLRwGq6rAvWzJpkYAXqebv"
    );
    pubkeys!(
        sol_domain_reverse_lookup_class,
        "33m47vH6Eav6jr5Ry86XjhRft2jRBLDnDgPSHoquXi2Z"
    );
}

#[derive(BorshDeserialize, PartialEq, Debug, Clone)]
struct Header {
    parent: [u8; 32],
    owner: [u8; 32],
    class: [u8; 32],
}

pub(crate) async fn process(client: &Client, update: AccountUpdate) -> Result<()> {
    if update.data.len() <= HEADER_LENGTH {
        return Ok(());
    }

    let header_bytes: [u8; HEADER_LENGTH] = update.data[..HEADER_LENGTH].try_into()?;

    let header = Header::try_from_slice(header_bytes.as_slice())
        .context("failed to deserialize header data")?;

    let parent = Pubkey::new(header.parent.as_slice());
    let class = Pubkey::new(header.class.as_slice());

    if (parent != ids::twitter_root_parent_registry_key()
        && class != ids::sol_domain_reverse_lookup_class())
        || (class != ids::twitter_verification_authority()
            && class != ids::sol_domain_reverse_lookup_class())
    {
        return Ok(());
    }

    let wallet = Pubkey::new(header.owner.as_slice());

    let mut data: Vec<u8> = update.data[HEADER_LENGTH..].to_vec();

    if class == ids::sol_domain_reverse_lookup_class() {
        let domain_length: usize = u32::from_le_bytes(data[0..4].try_into()?).try_into()?;
        data = data[4..4 + domain_length].to_vec();
        name_service::process_domain_name(client, update.key, update.slot, wallet, data).await?;
        return Ok(());
    }

    name_service::process(client, update.key, update.slot, wallet, data).await
}
