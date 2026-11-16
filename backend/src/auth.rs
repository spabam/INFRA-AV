use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow, bail};
use bcrypt::{DEFAULT_COST, hash, verify};
use chrono::Utc;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use pam_client::conv_mock::Conversation as PamConversation;
use pam_client::{Context as PamContext, Flag as PamFlag};
use serde::{Deserialize, Serialize};

use crate::config::AppConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub role: String,
    pub exp: usize,
    pub iat: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredUser {
    username: String,
    password_hash: String,
    #[serde(default = "default_role")]
    role: String,
}

fn default_role() -> String {
    "admin".to_string()
}

pub fn authenticate(config: &AppConfig, username: &str, password: &str) -> Result<String> {
    if authenticate_with_pam(config, username, password).is_ok() {
        return Ok(resolve_role(config, username));
    }

    authenticate_with_local(config, username, password)
}

pub fn issue_token(config: &AppConfig, username: &str, role: &str) -> Result<String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_secs() as usize;
    let expires_at = (Utc::now() + chrono::Duration::hours(8)).timestamp() as usize;

    let claims = Claims {
        sub: username.to_string(),
        role: role.to_string(),
        exp: expires_at,
        iat: now,
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(config.jwt_secret.as_bytes()),
    )
    .context("failed to encode jwt")?;

    Ok(token)
}

#[derive(Debug, Clone, Deserialize)]
struct BrokerClaims {
    sub: String,
    #[serde(default)]
    roles: Vec<String>,
    #[allow(dead_code)]
    exp: usize,
}

/// Verify a UUUUI broker handoff token (HS256, short-lived) and return the
/// authenticated username together with the UUUUI roles it carries.
pub fn verify_broker_token(secret: &str, token: &str) -> Result<(String, Vec<String>)> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = true;
    let data = decode::<BrokerClaims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .context("invalid SSO token")?;
    Ok((data.claims.sub, data.claims.roles))
}

pub fn decode_token(config: &AppConfig, token: &str) -> Result<Claims> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = true;

    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(config.jwt_secret.as_bytes()),
        &validation,
    )
    .context("invalid token")?;

    Ok(data.claims)
}

/// True when a locally-stored account's current role no longer matches the role
/// embedded in a presented token — an immediate demotion/revocation signal.
fn local_role_revoked(stored_role: Option<&str>, token_role: &str) -> bool {
    matches!(stored_role, Some(role) if role != token_role)
}

/// Re-authorize a decoded native token against the CURRENT local user store so a
/// change to a locally-managed account takes effect immediately instead of
/// lingering until the 8 h token expires. Subjects absent from the local store
/// (PAM/system users, or UUUUI SSO subjects) are not locally managed, so their
/// absence is NOT a rejection — the token expiry still bounds them, and the
/// UUUUI directory path re-checks membership on every request regardless. Only
/// standalone (non-directory) sessions reach this check.
pub fn reauthorize_native(config: &AppConfig, claims: &Claims) -> Result<()> {
    let stored_role = read_users(config)?
        .into_iter()
        .find(|candidate| candidate.username == claims.sub)
        .map(|user| user.role);
    if local_role_revoked(stored_role.as_deref(), &claims.role) {
        bail!("account role changed; please sign in again");
    }
    Ok(())
}

pub fn directory_enabled() -> bool {
    std::env::var("UUUUI_DIRECTORY_URL").is_ok_and(|value| !value.is_empty())
}

/// A fresh assertion is supplied by nginx after checking current platform
/// membership and AV access. A cached AV login token cannot retain old rights.
pub fn directory_claims(config: &AppConfig, token: &str) -> Result<Claims> {
    // Accept the current broker secret and, during a rotation overlap window, the
    // previous one (UUUUI/shared/uuuui-identity/CONTRACT.md).
    let secrets: Vec<&str> = config
        .broker_secret
        .as_deref()
        .into_iter()
        .chain(config.broker_secret_previous.as_deref())
        .collect();
    verify_directory_identity_with_keys(&secrets, token)
}

fn verify_directory_identity_with_keys(secrets: &[&str], token: &str) -> Result<Claims> {
    if secrets.is_empty() {
        bail!("invalid token: directory verification is not configured");
    }
    let mut last_error = None;
    for secret in secrets {
        match verify_directory_identity(secret, token) {
            Ok(claims) => return Ok(claims),
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.expect("at least one secret was tried"))
}

// Verification rules per the UUUUI identity-assertion contract
// (UUUUI/shared/uuuui-identity/CONTRACT.md): HS256 only, aud=av, not expired,
// 0 < exp-iat <= 120 s, iat not in the future. MAX_TTL bounds the damage window
// if a key leaks, independent of whatever exp a forger chooses.
const DIRECTORY_MAX_TTL_SECONDS: usize = 120;
const DIRECTORY_SKEW_SECONDS: usize = 30;

fn verify_directory_identity(secret: &str, token: &str) -> Result<Claims> {
    #[derive(Deserialize)]
    struct Identity {
        sub: String,
        #[serde(default)]
        native_subject: Option<String>,
        #[serde(default)]
        roles: Vec<String>,
        #[serde(rename = "moduleRole")]
        module_role: String,
        exp: usize,
        iat: usize,
    }
    let mut validation = Validation::new(Algorithm::HS256);
    validation.set_audience(&["av"]);
    validation.leeway = DIRECTORY_SKEW_SECONDS as u64;
    let identity = decode::<Identity>(token, &DecodingKey::from_secret(secret.as_bytes()), &validation)
        .context("invalid token: UUUUI directory assertion")?.claims;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as usize)
        .unwrap_or(0);
    if identity.exp <= identity.iat
        || identity.exp - identity.iat > DIRECTORY_MAX_TTL_SECONDS
        || identity.iat > now + DIRECTORY_SKEW_SECONDS
    {
        bail!("invalid token: UUUUI directory assertion lifetime");
    }
    let administrator = identity.roles.iter().any(|role| role == "admin" || role == "superadmin") || identity.module_role == "admin";
    Ok(Claims { sub: identity.native_subject.unwrap_or(identity.sub), role: if administrator { "admin" } else { "member" }.to_string(), exp: identity.exp, iat: identity.iat })
}

fn authenticate_with_pam(config: &AppConfig, username: &str, password: &str) -> Result<()> {
    let mut context = PamContext::new(
        &config.pam_service,
        None,
        PamConversation::with_credentials(username, password),
    )
    .with_context(|| format!("failed to initialize PAM service {}", config.pam_service))?;

    context
        .authenticate(PamFlag::NONE)
        .context("PAM authentication failed")?;
    context
        .acct_mgmt(PamFlag::NONE)
        .context("PAM account validation failed")?;

    Ok(())
}

fn authenticate_with_local(config: &AppConfig, username: &str, password: &str) -> Result<String> {
    let user = read_users(config)?
        .into_iter()
        .find(|candidate| candidate.username == username)
        .ok_or_else(|| anyhow!("invalid username or password"))?;

    if !verify(password, &user.password_hash).context("password verification failed")? {
        bail!("invalid username or password");
    }

    Ok(user.role)
}

fn resolve_role(config: &AppConfig, username: &str) -> String {
    read_users(config)
        .ok()
        .and_then(|users| {
            users.into_iter()
                .find(|candidate| candidate.username == username)
                .map(|user| user.role)
        })
        .unwrap_or_else(|| config.default_role.clone())
}

fn read_users(config: &AppConfig) -> Result<Vec<StoredUser>> {
    let content = match fs::read_to_string(&config.users_file) {
        Ok(content) => content,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(error).with_context(|| {
                format!(
                    "failed to read users file {}, run installer.sh first",
                    config.users_file.display()
                )
            });
        }
    };

    serde_json::from_str::<Vec<StoredUser>>(&content)
        .with_context(|| format!("failed to parse users file {}", config.users_file.display()))
}

pub fn bootstrap_user_file(
    output: &Path,
    username: &str,
    password: &str,
    role: &str,
) -> Result<()> {
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let users = vec![StoredUser {
        username: username.to_string(),
        password_hash: hash(password, DEFAULT_COST).context("failed to hash password")?,
        role: role.to_string(),
    }];

    let content = serde_json::to_vec_pretty(&users).context("failed to serialize users")?;
    fs::write(output, content).with_context(|| format!("failed to write {}", output.display()))?;

    Ok(())
}

#[cfg(test)]
mod directory_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn directory_roles_replace_cached_privileges_and_preserve_native_ownership() {
        let secret = "isolated-directory-secret";
        let now = Utc::now().timestamp();
        let mut claims = json!({"sub":"person@example.test","native_subject":"legacy-av-name","aud":"av","roles":["member"],"moduleRole":"member","iat":now,"exp":now+60});
        let token = |claims: &serde_json::Value| encode(&Header::default(), claims, &EncodingKey::from_secret(b"isolated-directory-secret")).unwrap();
        let member = verify_directory_identity(secret, &token(&claims)).unwrap();
        assert_eq!((member.sub.as_str(), member.role.as_str()), ("legacy-av-name", "member"));
        claims["moduleRole"] = json!("admin");
        assert_eq!(verify_directory_identity(secret, &token(&claims)).unwrap().role, "admin");
        claims["moduleRole"] = json!("member");
        claims["roles"] = json!(["superadmin"]);
        assert_eq!(verify_directory_identity(secret, &token(&claims)).unwrap().role, "admin");
        claims["aud"] = json!("ticket");
        assert!(verify_directory_identity(secret, &token(&claims)).is_err());
        claims["aud"] = json!("av");
        claims["exp"] = json!(now - 120);
        assert!(verify_directory_identity(secret, &token(&claims)).is_err());
    }

    // Mirrors UUUUI/shared/uuuui-identity/vectors.json for the av verifier.
    #[test]
    fn directory_assertion_conformance() {
        let current = "uuuui-identity-conformance-key";
        let previous = "uuuui-identity-conformance-previous-key";
        let unknown = "uuuui-identity-conformance-unknown-key";
        let now = Utc::now().timestamp();
        let sign = |secret: &str, claims: &serde_json::Value| {
            encode(&Header::default(), claims, &EncodingKey::from_secret(secret.as_bytes())).unwrap()
        };
        let base = |iat: i64, ttl: i64| {
            json!({"sub":"alice@uuuui.local","uid":"11111111-1111-1111-1111-111111111111",
                   "kind":"app","roles":["member"],"moduleRole":"member","aud":"av","iat":iat,"exp":iat+ttl})
        };
        // valid
        assert!(verify_directory_identity(current, &sign(current, &base(now, 60))).is_ok());
        // wrong aud
        let mut wrong_aud = base(now, 60);
        wrong_aud["aud"] = json!("ticket");
        assert!(verify_directory_identity(current, &sign(current, &wrong_aud)).is_err());
        // expired
        assert!(verify_directory_identity(current, &sign(current, &base(now - 120, 60))).is_err());
        // iat in the future (beyond skew)
        assert!(verify_directory_identity(current, &sign(current, &base(now + 120, 60))).is_err());
        // lifetime over the 120 s cap
        assert!(verify_directory_identity(current, &sign(current, &base(now, 600))).is_err());
        // missing sub
        let mut no_sub = base(now, 60);
        no_sub.as_object_mut().unwrap().remove("sub");
        assert!(verify_directory_identity(current, &sign(current, &no_sub)).is_err());
        // bad signature (signed with a key the verifier does not hold)
        assert!(verify_directory_identity(current, &sign(unknown, &base(now, 60))).is_err());
        // key set: current + previous accepted (rotation overlap), unknown rejected
        let keys = [current, previous];
        assert!(verify_directory_identity_with_keys(&keys, &sign(current, &base(now, 60))).is_ok());
        assert!(verify_directory_identity_with_keys(&keys, &sign(previous, &base(now, 60))).is_ok());
        assert!(verify_directory_identity_with_keys(&keys, &sign(unknown, &base(now, 60))).is_err());
    }

    #[test]
    fn local_role_revoked_only_when_stored_role_differs() {
        assert!(!local_role_revoked(None, "admin")); // not locally stored → not revoked
        assert!(!local_role_revoked(Some("admin"), "admin")); // unchanged → ok
        assert!(local_role_revoked(Some("member"), "admin")); // demoted in store → revoked
    }
}
