use anyhow::{Context, Result};
use aptos_playground::AptosConfig;
use aptos_sdk::coin_client::CoinClient;
use aptos_sdk::move_types::{
    identifier::Identifier,
    language_storage::{ModuleId, TypeTag},
};
use aptos_sdk::rest_client::{Client, FaucetClient};
use aptos_sdk::transaction_builder::TransactionBuilder;
use aptos_sdk::types::{
    account_address::AccountAddress,
    chain_id::ChainId,
    transaction::{EntryFunction, TransactionPayload},
    AccountKey, LocalAccount,
};
use dotenv::dotenv;
use std::{
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};
use url::Url;

const DEFAULT_TO_AMOUNT: u64 = 1_00000000_u64;

#[tokio::main]
async fn main() -> Result<()> {
    // setup env and log
    dotenv().ok();
    pretty_env_logger::init();

    let default_profile = AptosConfig::load_profile("default")?;
    log::debug!("{:?}", &default_profile);

    let rest_url = default_profile
        .rest_url
        .expect("'rest_url' field not found");
    let faucet_url = default_profile
        .faucet_url
        .expect("'faucet_url' field not found");
    let address = default_profile.account.expect("'account' field not found");
    let private_key = default_profile
        .private_key
        .expect("'private_key' field not found");

    // setup client
    let rest_url = Url::from_str(&rest_url)?;
    let faucet_url = Url::from_str(&faucet_url)?;
    let client = Client::new(rest_url.clone());
    log::debug!("client\n{:?}\n", &client);
    let faucet = FaucetClient::new(faucet_url, rest_url);
    let coin_client = CoinClient::new(&client);

    // setup owner
    let key = AccountKey::from_private_key(private_key);
    let mut owner = LocalAccount::new(address, key, 0);
    log::debug!("owner\n{:?}\n", &owner);
    log::info!("owner address: {}", &owner.address().to_hex_literal());

    // create the owner's account on chain and fund it
    faucet
        .fund(owner.address(), DEFAULT_TO_AMOUNT)
        .await
        .context("Failed to fund owner's account")?;

    log::info!(
        "Owner: {:?}",
        coin_client
            .get_account_balance(&owner.address())
            .await
            .context("Failed to get owner's account balance")?
    );

    // register InJoyCoin
    let chain_id = client
        .get_index()
        .await
        .expect("Failed to fetch chain ID")
        .inner()
        .chain_id;

    let coin_type = format!("0x{}::injoy_coin::InJoyCoin", &address);
    log::debug!("coin type\n{}\n", coin_type);

    let estimated_gas_price = client.estimate_gas_price().await?.inner().gas_estimate;
    let transaction = TransactionBuilder::new(
        TransactionPayload::EntryFunction(EntryFunction::new(
            ModuleId::new(AccountAddress::ONE, Identifier::new("managed_coin")?),
            Identifier::new("register")?,
            vec![TypeTag::from_str(&coin_type)?],
            vec![],
        )),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() + 300,
        ChainId::new(chain_id),
    )
    .sender(owner.address())
    .sequence_number(owner.sequence_number())
    .max_gas_amount(10_000)
    .gas_unit_price(estimated_gas_price);
    let signed_txn = owner.sign_with_transaction_builder(transaction);
    log::debug!("transaction\n{:?}\n", &signed_txn);
    let pending_txn = client.submit(&signed_txn).await?;
    let transaction = client.wait_for_transaction(pending_txn.inner()).await?;
    log::debug!("transaction\n{:?}\n", &transaction);

    Ok(())
}
