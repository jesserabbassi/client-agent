/// Development-only verification. Production must verify a server-issued challenge.
pub(crate) fn verify(code: &str) -> Result<(), &'static str> {
    if !cfg!(feature = "mock-auth") {
        return Err("Verification server unavailable.");
    }
    if code.len() != 6 || !code.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("Enter the six-digit verification code.");
    }
    if code != "123456" {
        return Err("Incorrect code. Please try again.");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::verify;
    #[test]
    fn validates_demo_code() {
        if cfg!(feature = "mock-auth") {
            assert!(verify("123456").is_ok());
        } else {
            assert!(verify("123456").is_err());
        }
        for code in ["", "12345", "abcdef", "000000", "1234567", "１２３４５６"] {
            assert!(verify(code).is_err());
        }
    }
}
