use std::collections::BTreeMap;

use subtle::ConstantTimeEq;

/// Free-form `key=value` pairs a client appends to its token (`secret;run=123;repo=org/app`),
/// typically the CI run that produced or consumed an artifact.
pub type Context = BTreeMap<String, String>;

const MAX_CONTEXT_PAIRS: usize = 8;
const MAX_CONTEXT_VALUE: usize = 96;

/// Split `secret;k=v;k=v` into the secret and its context. Malformed pairs are dropped.
pub fn split_context(token: &str) -> (&str, Context) {
    let mut parts = token.split(';');
    let secret = parts.next().unwrap_or("");
    let mut ctx = Context::new();
    for pair in parts {
        if ctx.len() >= MAX_CONTEXT_PAIRS {
            break;
        }
        let Some((k, v)) = pair.split_once('=') else { continue };
        let (k, v) = (k.trim(), v.trim());
        let key_ok = (1..=16).contains(&k.len())
            && k.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
        let value_ok = (1..=MAX_CONTEXT_VALUE).contains(&v.len()) && v.bytes().all(|b| (0x20..0x7f).contains(&b));
        if key_ok && value_ok {
            ctx.insert(k.to_owned(), v.to_owned());
        }
    }
    (secret, ctx)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Authenticated {
    pub access: Access,
    pub context: Context,
}

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

    pub fn authenticate(&self, authorization: Option<&str>) -> Result<Authenticated, AuthError> {
        let token = authorization.and_then(bearer);
        let (secret, context) = match token {
            Some(t) => {
                let (s, c) = split_context(t);
                (Some(s), c)
            }
            None => (None, Context::new()),
        };
        let access = match secret {
            Some(t) if matches_any(&self.rw, t) => Access::ReadWrite,
            Some(t) if matches_any(&self.ro, t) => Access::ReadOnly,
            Some(_) if self.allow_anonymous => Access::ReadWrite,
            Some(_) => return Err(AuthError::Invalid),
            None if self.allow_anonymous => Access::ReadWrite,
            None => return Err(AuthError::Missing),
        };
        Ok(Authenticated { access, context })
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

    fn access(r: Result<Authenticated, AuthError>) -> Result<Access, AuthError> {
        r.map(|a| a.access)
    }

    #[test]
    fn roles() {
        let t = tokens();
        assert_eq!(access(t.authenticate(Some("Bearer rw1"))), Ok(Access::ReadWrite));
        assert_eq!(access(t.authenticate(Some("bearer rw2"))), Ok(Access::ReadWrite));
        assert_eq!(access(t.authenticate(Some("Bearer ro"))), Ok(Access::ReadOnly));
        assert_eq!(access(t.authenticate(Some("Bearer nope"))), Err(AuthError::Invalid));
        assert_eq!(access(t.authenticate(Some("Basic rw1"))), Err(AuthError::Missing));
        assert_eq!(access(t.authenticate(Some("Bearer "))), Err(AuthError::Missing));
        assert_eq!(access(t.authenticate(None)), Err(AuthError::Missing));
    }

    #[test]
    fn anonymous() {
        let t = Tokens::new(&[], &[], true);
        assert_eq!(access(t.authenticate(None)), Ok(Access::ReadWrite));
        assert_eq!(access(t.authenticate(Some("Bearer anything"))), Ok(Access::ReadWrite));
    }

    #[test]
    fn context_after_secret() {
        let t = tokens();
        let a = t
            .authenticate(Some("Bearer rw1;run=123;repo=org/app;bad key=1;=x;actor=dev"))
            .unwrap();
        assert_eq!(a.access, Access::ReadWrite);
        assert_eq!(a.context.get("run").map(String::as_str), Some("123"));
        assert_eq!(a.context.get("repo").map(String::as_str), Some("org/app"));
        assert_eq!(a.context.get("actor").map(String::as_str), Some("dev"));
        assert_eq!(a.context.len(), 3);
        assert_eq!(
            access(t.authenticate(Some("Bearer nope;run=1"))),
            Err(AuthError::Invalid)
        );
        let (secret, ctx) = split_context("s;a=1;b=2;c=3;d=4;e=5;f=6;g=7;h=8;i=9");
        assert_eq!(secret, "s");
        assert_eq!(ctx.len(), 8);
    }
}
