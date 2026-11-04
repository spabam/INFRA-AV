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
