//! OS-backed secret storage. Plaintext fallback is intentionally unsupported.

const SERVICE: &str = "NinetyGamingHouse.ClientAgent.v1";

pub(crate) fn set(name: &str, value: &str) -> Result<(), &'static str> {
    entry(name)?.set_password(value).map_err(|_| "could not write secure credential")
}

pub(crate) fn get(name: &str) -> Result<Option<String>, &'static str> {
    match entry(name)?.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => Err("could not read secure credential"),
    }
}

pub(crate) fn delete(name: &str) -> Result<(), &'static str> {
    match entry(name)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => Err("could not delete secure credential"),
    }
}

fn entry(name: &str) -> Result<keyring::Entry, &'static str> {
    keyring::Entry::new(SERVICE, name).map_err(|_| "could not initialize secure credential")
}
