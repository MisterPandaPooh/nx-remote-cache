use subtle::ConstantTimeEq;

/// What a request is allowed to do, decided from its bearer token.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    ReadWrite,
    ReadOnly,
}

#[derive(Debug, PartialEq, Eq)]
pub enum AuthError {
    Missing,
    Invalid,
}

/// Token set compared in constant time. Keep it tiny: one shared secret per role.
#[derive(Clone, Debug)]
pub struct Tokens {
    rw: Vec<Vec<u8>>,
    ro: Vec<Vec<u8>>,
    allow_anonymous: bool,
}

impl Tokens {
    pub fn new(rw: &[String], ro: &[String], allow_anonymous: bool) -> Self {
        Self {
            rw: rw.iter().map(|t| t.as_bytes().to_vec()).collect(),
            ro: ro.iter().map(|t| t.as_bytes().to_vec()).collect(),
            allow_anonymous,
        }
    }

    pub fn authenticate(&self, authorization: Option<&str>) -> Result<Access, AuthError> {
        let token = authorization.and_then(bearer);
        match token {
            Some(t) if matches_any(&self.rw, t) => Ok(Access::ReadWrite),
            Some(t) if matches_any(&self.ro, t) => Ok(Access::ReadOnly),
            Some(_) if self.allow_anonymous => Ok(Access::ReadWrite),
            Some(_) => Err(AuthError::Invalid),
            None if self.allow_anonymous => Ok(Access::ReadWrite),
            None => Err(AuthError::Missing),
        }
    }

    /// True when `value` equals any configured token (for the UI token check).
    pub fn matches_single(expected: &str, value: &str) -> bool {
        expected.as_bytes().ct_eq(value.as_bytes()).into()
    }
}

fn bearer(header: &str) -> Option<&str> {
    let (scheme, rest) = header.trim().split_once(char::is_whitespace)?;
    if !scheme.eq_ignore_ascii_case("bearer") {
        return None;
    }
    let token = rest.trim();
    (!token.is_empty()).then_some(token)
}

fn matches_any(candidates: &[Vec<u8>], token: &str) -> bool {
    // Compare against every candidate so timing does not reveal which one matched.
    let mut ok = false;
    for c in candidates {
        ok |= bool::from(c.as_slice().ct_eq(token.as_bytes()));
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens() -> Tokens {
        Tokens::new(&["rw1".into(), "rw2".into()], &["ro".into()], false)
    }

    #[test]
    fn roles() {
        let t = tokens();
        assert_eq!(t.authenticate(Some("Bearer rw1")), Ok(Access::ReadWrite));
        assert_eq!(t.authenticate(Some("bearer rw2")), Ok(Access::ReadWrite));
        assert_eq!(t.authenticate(Some("Bearer ro")), Ok(Access::ReadOnly));
        assert_eq!(t.authenticate(Some("Bearer nope")), Err(AuthError::Invalid));
        assert_eq!(t.authenticate(Some("Basic rw1")), Err(AuthError::Missing));
        assert_eq!(t.authenticate(Some("Bearer ")), Err(AuthError::Missing));
        assert_eq!(t.authenticate(None), Err(AuthError::Missing));
    }

    #[test]
    fn anonymous() {
        let t = Tokens::new(&[], &[], true);
        assert_eq!(t.authenticate(None), Ok(Access::ReadWrite));
        assert_eq!(t.authenticate(Some("Bearer anything")), Ok(Access::ReadWrite));
    }
}
