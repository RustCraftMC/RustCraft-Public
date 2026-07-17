use super::models::{Account, AuthError};

const CACHE_FILE: &str = "auth_cache.json";
const ACCOUNTS_FILE: &str = "auth_accounts.json";

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct AccountStore {
    selected_uuid: Option<String>,
    accounts: Vec<Account>,
}

/// Save the current account to a local JSON file.
pub fn save_account(account: &Account) -> Result<(), AuthError> {
    let mut store = load_store()?;
    let uuid = account.uuid.clone();
    if let Some(index) = store.accounts.iter().position(|saved| saved.uuid == uuid) {
        store.accounts[index] = account.clone();
    } else {
        store.accounts.push(account.clone());
    }
    store.selected_uuid = uuid;
    save_store(&store)?;
    Ok(())
}

/// Load the cached account from the local JSON file.
/// Returns None if no cache exists or it's corrupt.
pub fn load_account() -> Result<Option<Account>, AuthError> {
    let store = load_store()?;
    if !store.accounts.is_empty() {
        return Ok(store
            .selected_uuid
            .as_ref()
            .and_then(|uuid| {
                store
                    .accounts
                    .iter()
                    .find(|account| account.uuid.as_ref() == Some(uuid))
            })
            .cloned()
            .or_else(|| store.accounts.first().cloned()));
    }
    match std::fs::read_to_string(CACHE_FILE) {
        Ok(json) => {
            let account: Account = serde_json::from_str(&json)
                .map_err(|e| AuthError::Cache(format!("Failed to parse cache: {}", e)))?;
            Ok(Some(account))
        }
        Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(AuthError::Cache(format!("Failed to read cache: {}", e))),
    }
}

pub fn load_accounts() -> Result<Vec<Account>, AuthError> {
    let mut accounts = load_store()?.accounts;
    if accounts.is_empty() {
        if let Some(account) = load_account()? {
            accounts.push(account);
        }
    }
    Ok(accounts)
}

pub fn select_account(uuid: &str) -> Result<Option<Account>, AuthError> {
    let mut store = load_store()?;
    let selected = store
        .accounts
        .iter()
        .find(|account| account.uuid.as_deref() == Some(uuid))
        .cloned();
    if selected.is_some() {
        store.selected_uuid = Some(uuid.to_string());
        save_store(&store)?;
    }
    Ok(selected)
}

pub fn remove_account(uuid: &str) -> Result<(), AuthError> {
    let mut store = load_store()?;
    store
        .accounts
        .retain(|account| account.uuid.as_deref() != Some(uuid));
    if store.selected_uuid.as_deref() == Some(uuid) {
        store.selected_uuid = store
            .accounts
            .first()
            .and_then(|account| account.uuid.clone());
    }
    save_store(&store)
}

/// Delete the cached account.
pub fn clear_cache() -> Result<(), AuthError> {
    if std::path::Path::new(CACHE_FILE).exists() {
        std::fs::remove_file(CACHE_FILE)?;
    }
    if std::path::Path::new(ACCOUNTS_FILE).exists() {
        std::fs::remove_file(ACCOUNTS_FILE)?;
    }
    Ok(())
}

fn load_store() -> Result<AccountStore, AuthError> {
    match std::fs::read_to_string(ACCOUNTS_FILE) {
        Ok(json) => {
            serde_json::from_str(&json).map_err(|error| AuthError::Cache(error.to_string()))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let legacy = std::fs::read_to_string(CACHE_FILE)
                .ok()
                .and_then(|json| serde_json::from_str::<Account>(&json).ok());
            Ok(match legacy {
                Some(account) => AccountStore {
                    selected_uuid: account.uuid.clone(),
                    accounts: vec![account],
                },
                None => AccountStore::default(),
            })
        }
        Err(error) => Err(AuthError::Cache(error.to_string())),
    }
}

fn save_store(store: &AccountStore) -> Result<(), AuthError> {
    std::fs::write(ACCOUNTS_FILE, serde_json::to_string_pretty(store)?)?;
    Ok(())
}
