#!/usr/bin/env bash

# --- VANTAGE edition: new (UUUUI-integrated) ONLY ---
# The old standalone edition was removed 2026-11-18. VANTAGE installs only the new,
# UUUUI-integrated edition (one UI on :31337); 'old'/'standalone' is refused.
: "${VANTAGE_EDITION:=new}"
case "${VANTAGE_EDITION}" in
  new) ;;
  old|standalone|2|o|O) echo "[vantage] the old standalone edition has been removed; only the new UUUUI-integrated edition is available" >&2; exit 2 ;;
  *) echo "VANTAGE_EDITION must be 'new'" >&2; exit 2 ;;
esac
export VANTAGE_EDITION
printf '[vantage] installing the new (UUUUI-integrated) edition\n' >&2
# The standalone nginx web front (INFRA-AV HTTPS on :8099 + /var/www/infra-av/frontend)
# belonged to the removed standalone edition and collides with the UUUUI broker (:8099).
# In the integrated edition UUUUI serves INFRA-AV through its gateway (:31337), so the
# front is skipped. Off by default; INFRA_AV_STANDALONE_FRONT=1 restores the retired front.
STANDALONE_FRONT="${INFRA_AV_STANDALONE_FRONT:-0}"
# --- end edition selector ---

# Ensure the shared INFRA authentication helper exists without replacing it.
_vantage_infrauth_path=/etc/infrascan/infrauth
if [ ! -e "$_vantage_infrauth_path" ]; then
    _vantage_infrauth_tmp=$(mktemp) || exit 1
    printf '%s\n' 'echo www.infrascan.net' >"$_vantage_infrauth_tmp"
    if [ "$(id -u)" -eq 0 ]; then
        install -d -m 755 /etc/infrascan &&
            install -m 644 "$_vantage_infrauth_tmp" "$_vantage_infrauth_path"
    elif command -v sudo >/dev/null 2>&1; then
        sudo install -d -m 755 /etc/infrascan &&
            sudo install -m 644 "$_vantage_infrauth_tmp" "$_vantage_infrauth_path"
    else
        printf 'Error: root privileges are required to create %s\n' "$_vantage_infrauth_path" >&2
        rm -f "$_vantage_infrauth_tmp"
        exit 1
    fi || {
        rm -f "$_vantage_infrauth_tmp"
        exit 1
    }
    rm -f "$_vantage_infrauth_tmp"
fi
# VANTAGE_INSTALLER_CLEANUP_BOOTSTRAP
__vantage_installer_dir="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")" && pwd)"
__vantage_cleanup_probe="$__vantage_installer_dir"
while [ "$__vantage_cleanup_probe" != "/" ]; do
    if [ -f "$__vantage_cleanup_probe/.installer_cleanup.sh" ]; then
        # shellcheck source=/dev/null
        . "$__vantage_cleanup_probe/.installer_cleanup.sh"
        vantage_register_installer_cleanup "$__vantage_installer_dir"
        break
    fi
    __vantage_cleanup_probe="$(dirname "$__vantage_cleanup_probe")"
done
unset __vantage_cleanup_probe


set -euo pipefail

SCRIPT_PATH="$(readlink -f "${BASH_SOURCE[0]}")"
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FRONTEND_DIR="$PROJECT_ROOT/frontend"
BACKEND_DIR="$PROJECT_ROOT/backend"
BACKEND_BINARY="$BACKEND_DIR/target/release/infra-av-backend"
# The freshly built binary lives under the source tree's target/ dir, which the
# shared post-install cleanup (.installer_cleanup.sh EXIT trap) removes. Deploy a
# stable copy OUTSIDE the source tree and run the service + cron from that copy,
# so the cleanup can never delete the binary out from under the running services.
INSTALL_BIN_DIR="${INFRA_AV_INSTALL_BIN_DIR:-/usr/local/lib/infra-av}"
INSTALLED_BINARY="$INSTALL_BIN_DIR/infra-av-backend"
INSTALL_MODE="${1:-install}"

ENV_DIR="/etc/infra-av"
ENV_FILE="$ENV_DIR/backend.env"
USERS_FILE="$ENV_DIR/users.json"
DATA_DIR="/var/lib/infra-av"
LOG_DIR="/var/log/infra-av"
DEPLOY_FRONTEND_DIR="/var/www/infra-av/frontend"
TLS_DIR="$ENV_DIR/tls"
FRESHCLAM_CONFIG_FILE="/etc/clamav/freshclam.conf"
FRESHCLAM_CONFIG_BACKUP="/etc/clamav/freshclam.conf.infra-av.backup"
FRESHCLAM_LOCK_FILE="/var/lock/infra-av-freshclam.lock"
FRESHCLAM_SERVICE="clamav-freshclam.service"

BACKEND_HOST="${INFRA_AV_BACKEND_HOST:-127.0.0.1}"
BACKEND_PORT="${INFRA_AV_BACKEND_PORT:-5000}"
NGINX_PORT="${INFRA_AV_NGINX_PORT:-8099}"
SERVER_NAME="${INFRA_AV_SERVER_NAME:-_}"
PAM_SERVICE="${INFRA_AV_PAM_SERVICE:-infra-av}"
DEFAULT_ROLE="${INFRA_AV_DEFAULT_ROLE:-admin}"
TLS_CERT_FILE="${INFRA_AV_TLS_CERT_FILE:-$TLS_DIR/infra-av.crt}"
TLS_KEY_FILE="${INFRA_AV_TLS_KEY_FILE:-$TLS_DIR/infra-av.key}"
TLS_CERT_DAYS="${INFRA_AV_TLS_CERT_DAYS:-825}"
JWT_SECRET="${INFRA_AV_JWT_SECRET:-}"
ADMIN_USER="${INFRA_AV_ADMIN_USER:-}"
ADMIN_PASSWORD="${INFRA_AV_ADMIN_PASSWORD:-}"
ALLOWED_SCAN_ROOTS="${INFRA_AV_ALLOWED_SCAN_ROOTS:-/home,/srv,/tmp}"
ALLOW_DEVICE_SCANS="${INFRA_AV_ALLOW_DEVICE_SCANS:-1}"
ENABLE_FRESHCLAM_CRON="${INFRA_AV_ENABLE_FRESHCLAM_CRON:-1}"
SOURCE_OWNER="$(stat -c '%U:%G' "$PROJECT_ROOT")"
RUST_ENV_FILE="/root/.cargo/env"
FALLBACK_ACCOUNT_MODE="created"
FALLBACK_PASSWORD_SUMMARY=""

SERVICE_FILE="/etc/systemd/system/infra-av-backend.service"
CRON_FILE="/etc/cron.d/infra-av"
NGINX_SITE="/etc/nginx/sites-available/infra-av.conf"
NGINX_LINK="/etc/nginx/sites-enabled/infra-av.conf"
PAM_SERVICE_FILE="/etc/pam.d/$PAM_SERVICE"
SUDO_ENV_VARS=(
  INFRA_AV_BACKEND_HOST
  INFRA_AV_BACKEND_PORT
  INFRA_AV_NGINX_PORT
  INFRA_AV_SERVER_NAME
  INFRA_AV_PAM_SERVICE
  INFRA_AV_DEFAULT_ROLE
  INFRA_AV_TLS_CERT_FILE
  INFRA_AV_TLS_KEY_FILE
  INFRA_AV_TLS_CERT_DAYS
  INFRA_AV_JWT_SECRET
  INFRA_AV_ADMIN_USER
  INFRA_AV_ADMIN_PASSWORD
  INFRA_AV_ALLOWED_SCAN_ROOTS
  INFRA_AV_ALLOW_DEVICE_SCANS
  INFRA_AV_ENABLE_FRESHCLAM_CRON
)

elevate_if_needed() {
  if [[ "${EUID}" -eq 0 ]]; then
    return
  fi

  if ! command -v sudo >/dev/null 2>&1; then
    echo "sudo is required to install INFRA-AV. Install sudo or run this script as root." >&2
    exit 1
  fi

  local preserve_env
  preserve_env="$(IFS=,; echo "${SUDO_ENV_VARS[*]}")"

  echo "Root privileges are required. sudo will prompt for your password."
  exec sudo --preserve-env="$preserve_env" bash "$SCRIPT_PATH" "$@"
}

require_root() {
  if [[ "${EUID}" -ne 0 ]]; then
    echo "Failed to obtain root privileges." >&2
    exit 1
  fi
}

install_packages() {
  export DEBIAN_FRONTEND=noninteractive
  export NEEDRESTART_SUSPEND=1
  local packages=(
    build-essential \
    ca-certificates \
    clamav \
    clamav-freshclam \
    clang \
    curl \
    cron \
    libclang-dev \
    libpam-modules \
    libpam-modules-bin \
    libpam-runtime \
    libpam0g-dev \
    nginx \
    openssl \
    p7zip-full \
    pkg-config \
    util-linux \
    unzip \
    zip
  )

  if ! command -v node >/dev/null 2>&1; then
    packages+=(nodejs)
  fi

  if ! command -v npm >/dev/null 2>&1; then
    packages+=(npm)
  fi

  apt-get -qq update
  apt-get -yqq install "${packages[@]}"

  local optional_packages=(
    clamav-docs
    libclamunrar
    libclamunrar11
    rar
    unace
    unrar
    arc
  )

  for package in "${optional_packages[@]}"; do
    if apt-cache policy "$package" 2>/dev/null | grep -q 'Candidate: ' \
      && ! apt-cache policy "$package" 2>/dev/null | grep -q 'Candidate: (none)'; then
      apt-get -yqq install "$package"
    fi
  done
}

backup_freshclam_config() {
  if [[ -f "$FRESHCLAM_CONFIG_FILE" && ! -f "$FRESHCLAM_CONFIG_BACKUP" ]]; then
    cp -a "$FRESHCLAM_CONFIG_FILE" "$FRESHCLAM_CONFIG_BACKUP"
  fi
}

write_freshclam_config() {
  mkdir -p /etc/clamav "$LOG_DIR"
  backup_freshclam_config

  cat >"$FRESHCLAM_CONFIG_FILE" <<'EOF'
# Managed by INFRA-AV
DatabaseOwner clamav
LogVerbose false
LogSyslog true
LogFacility LOG_LOCAL6
LogTime true
Foreground false
Debug false
MaxAttempts 5
DatabaseDirectory /var/lib/clamav
DNSDatabaseInfo current.cvd.clamav.net
ConnectTimeout 30
ReceiveTimeout 0
TestDatabases yes
ScriptedUpdates yes
CompressLocalDatabase no
Bytecode true
Checks 24
DatabaseMirror db.local.clamav.net
DatabaseMirror database.clamav.net
EOF

  chmod 644 "$FRESHCLAM_CONFIG_FILE"
  systemctl disable --now "$FRESHCLAM_SERVICE" >/dev/null 2>&1 || true
}

restore_freshclam_config() {
  if [[ -f "$FRESHCLAM_CONFIG_BACKUP" ]]; then
    cp -a "$FRESHCLAM_CONFIG_BACKUP" "$FRESHCLAM_CONFIG_FILE"
    rm -f "$FRESHCLAM_CONFIG_BACKUP"
  fi
  systemctl enable --now "$FRESHCLAM_SERVICE" >/dev/null 2>&1 || true
}

reload_nginx_if_present() {
  if ! command -v nginx >/dev/null 2>&1; then
    return
  fi

  if nginx -t >/dev/null 2>&1; then
    systemctl restart nginx >/dev/null 2>&1 || true
  fi
}

print_uninstall_summary() {
  cat <<EOF

INFRA-AV uninstall completed.

Removed:
  $SERVICE_FILE
  $CRON_FILE
  $NGINX_SITE
  $NGINX_LINK
  $PAM_SERVICE_FILE
  $ENV_DIR
  $DATA_DIR
  $LOG_DIR
  $DEPLOY_FRONTEND_DIR
  $INSTALL_BIN_DIR

Restored:
  $FRESHCLAM_CONFIG_FILE

EOF
}

# Remove only the regenerable service artifacts (systemd unit, cron, nginx site,
# PAM policy, deployed frontend + binary). Deliberately PRESERVES $ENV_DIR (JWT
# secret + users.json), $DATA_DIR (scan history, quarantine) and $LOG_DIR, so an
# install-over-existing (reinstall/upgrade) is non-destructive.
remove_service_artifacts() {
  systemctl disable --now infra-av-backend.service >/dev/null 2>&1 || true
  rm -f "$SERVICE_FILE"
  systemctl daemon-reload >/dev/null 2>&1 || true

  rm -f "$CRON_FILE"
  rm -f "$NGINX_LINK" "$NGINX_SITE"
  rm -f "$PAM_SERVICE_FILE"
  rm -rf "$DEPLOY_FRONTEND_DIR"
  rm -rf "$INSTALL_BIN_DIR"
}

# Full teardown, INCLUDING all persistent state. Only invoked for an explicit
# `uninstall`, never for a reinstall.
uninstall_existing_install() {
  remove_service_artifacts
  rm -rf "$ENV_DIR" "$DATA_DIR" "$LOG_DIR"

  restore_freshclam_config
  reload_nginx_if_present
}

install_rust() {
  if [[ ! -x /root/.cargo/bin/cargo ]]; then
    curl https://sh.rustup.rs -sSf | sh -s -- -y --profile minimal
  fi
  # shellcheck disable=SC1090
  source "$RUST_ENV_FILE"
}

check_frontend_tree() {
  if [[ ! -d "$FRONTEND_DIR" ]]; then
    echo "Missing frontend directory: $FRONTEND_DIR" >&2
    exit 1
  fi
  if [[ ! -f "$FRONTEND_DIR/logo.png" ]]; then
    echo "Missing frontend logo asset: $FRONTEND_DIR/logo.png" >&2
    exit 1
  fi
  if [[ ! -d "$BACKEND_DIR" ]]; then
    echo "Missing backend directory: $BACKEND_DIR" >&2
    exit 1
  fi
}

ensure_generated_values() {
  # Preserve the existing JWT secret across reinstalls so already-issued tokens
  # and live sessions survive an upgrade. Only mint a new one on a first install
  # or when the operator explicitly overrides it via INFRA_AV_JWT_SECRET.
  if [[ -z "$JWT_SECRET" && -f "$ENV_FILE" ]]; then
    JWT_SECRET="$(sed -n 's/^INFRA_AV_JWT_SECRET=//p' "$ENV_FILE" | head -n 1)"
  fi
  if [[ -z "$JWT_SECRET" ]]; then
    JWT_SECRET="$(openssl rand -hex 32)"
  fi

  if [[ -z "$ADMIN_USER" ]]; then
    ADMIN_USER="admin"
  fi
}

read_existing_fallback_username() {
  if [[ ! -f "$USERS_FILE" ]]; then
    return 1
  fi

  sed -n 's/^[[:space:]]*"username":[[:space:]]*"\([^"]*\)".*$/\1/p' "$USERS_FILE" | head -n 1
}

generate_admin_password() {
  openssl rand -base64 24 | tr -d '\n' | tr '/+' 'AB' | cut -c1-20
}

build_backend() {
  bash "$PROJECT_ROOT/scripts/version.sh" check
  # shellcheck disable=SC1090
  source "$RUST_ENV_FILE"
  pushd "$BACKEND_DIR" >/dev/null
  cargo build --release
  popd >/dev/null
}

build_frontend() {
  python3 "$FRONTEND_DIR/src/vantage-design/verify.py" --check
  pushd "$FRONTEND_DIR" >/dev/null
  npm ci --include=dev --no-audit --no-fund --loglevel=error
  BROWSERSLIST_IGNORE_OLD_DATA=1 VITE_API_BASE="" npm run build
  popd >/dev/null
}

deploy_backend() {
  # Copy the built binary to a stable path outside the source tree so the shared
  # installer cleanup (which deletes backend/target/ on exit) cannot remove the
  # binary that the systemd service and the every-minute cron worker execute.
  install -d -m 755 "$INSTALL_BIN_DIR"
  install -m 755 "$BACKEND_BINARY" "$INSTALLED_BINARY"
}

deploy_frontend() {
  mkdir -p "$(dirname "$DEPLOY_FRONTEND_DIR")"
  rm -rf "$DEPLOY_FRONTEND_DIR"
  cp -a "$FRONTEND_DIR/dist" "$DEPLOY_FRONTEND_DIR"
  rm -f "$DEPLOY_FRONTEND_DIR/favicon.ico"
}

build_cors_origins() {
  local origins=(
    "http://localhost:8080"
    "http://127.0.0.1:8080"
    "https://localhost:$NGINX_PORT"
    "https://127.0.0.1:$NGINX_PORT"
  )

  if [[ "$SERVER_NAME" != "_" && -n "$SERVER_NAME" ]]; then
    origins+=("https://$SERVER_NAME:$NGINX_PORT")
  fi

  local IFS=,
  printf '%s' "${origins[*]}"
}

write_env_file() {
  mkdir -p "$ENV_DIR" "$DATA_DIR" "$LOG_DIR"
  local cors_origins
  cors_origins="$(build_cors_origins)"

  cat >"$ENV_FILE" <<EOF
INFRA_AV_PROJECT_ROOT=$PROJECT_ROOT
INFRA_AV_HOST=$BACKEND_HOST
INFRA_AV_PORT=$BACKEND_PORT
INFRA_AV_JWT_SECRET=$JWT_SECRET
INFRA_AV_DATA_DIR=$DATA_DIR
INFRA_AV_PAM_SERVICE=$PAM_SERVICE
INFRA_AV_DEFAULT_ROLE=$DEFAULT_ROLE
INFRA_AV_USERS_FILE=$USERS_FILE
INFRA_AV_CLAMSCAN_BIN=/usr/bin/clamscan
INFRA_AV_FRESHCLAM_BIN=/usr/bin/freshclam
INFRA_AV_ALLOWED_SCAN_ROOTS=$ALLOWED_SCAN_ROOTS
INFRA_AV_ALLOWED_WRITE_ROOTS=$DATA_DIR/quarantine
INFRA_AV_ALLOWED_DATABASE_ROOTS=/var/lib/clamav,/usr/local/share/clamav
INFRA_AV_ALLOW_DEVICE_SCANS=$ALLOW_DEVICE_SCANS
INFRA_AV_CORS_ORIGINS=$cors_origins
EOF
}

bootstrap_admin_user() {
  local should_replace=0

  if [[ -v INFRA_AV_ADMIN_USER || -v INFRA_AV_ADMIN_PASSWORD || ! -f "$USERS_FILE" ]]; then
    should_replace=1
  fi

  if [[ "$should_replace" -eq 0 ]]; then
    local existing_user
    existing_user="$(read_existing_fallback_username || true)"
    if [[ -n "$existing_user" ]]; then
      ADMIN_USER="$existing_user"
    fi
    FALLBACK_ACCOUNT_MODE="reused"
    FALLBACK_PASSWORD_SUMMARY="unchanged existing value"
    return
  fi

  if [[ -z "$ADMIN_PASSWORD" ]]; then
    ADMIN_PASSWORD="$(generate_admin_password)"
  fi

  "$INSTALLED_BINARY" bootstrap-user \
    --username "$ADMIN_USER" \
    --password "$ADMIN_PASSWORD" \
    --role admin \
    --output "$USERS_FILE"

  FALLBACK_ACCOUNT_MODE="created"
  FALLBACK_PASSWORD_SUMMARY="$ADMIN_PASSWORD"
}

validate_pam_service_name() {
  if [[ ! "$PAM_SERVICE" =~ ^[A-Za-z0-9._-]+$ ]]; then
    echo "Invalid INFRA_AV_PAM_SERVICE value: $PAM_SERVICE" >&2
    exit 1
  fi
}

verify_pam_stack() {
  local required_files=(
    /etc/pam.d/common-auth
    /etc/pam.d/common-account
    /etc/pam.d/common-session-noninteractive
  )

  local path
  for path in "${required_files[@]}"; do
    if [[ ! -f "$path" ]]; then
      echo "Missing PAM stack file: $path" >&2
      echo "The system PAM runtime is incomplete. Ensure libpam-runtime is installed." >&2
      exit 1
    fi
  done
}

write_pam_service() {
  cat >"$PAM_SERVICE_FILE" <<EOF
# INFRA-AV PAM policy
auth      requisite pam_nologin.so
auth      include   common-auth
account   include   common-account
session   include   common-session-noninteractive
EOF

  chmod 644 "$PAM_SERVICE_FILE"
}

write_systemd_service() {
  cat >"$SERVICE_FILE" <<EOF
[Unit]
Description=INFRA-AV Rust backend
After=network.target

[Service]
Type=simple
WorkingDirectory=$DATA_DIR
EnvironmentFile=$ENV_FILE
ExecStart=$INSTALLED_BINARY server
Restart=on-failure
RestartSec=3

[Install]
WantedBy=multi-user.target
EOF
}

write_cron_file() {
  cat >"$CRON_FILE" <<EOF
SHELL=/bin/bash
PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:/root/.cargo/bin

* * * * * root flock -n /var/lock/infra-av-worker.lock $INSTALLED_BINARY worker --once >> $LOG_DIR/worker-cron.log 2>&1
EOF

  if [[ "$ENABLE_FRESHCLAM_CRON" == "1" ]]; then
    cat >>"$CRON_FILE" <<EOF
17 */4 * * * root flock -n $FRESHCLAM_LOCK_FILE /usr/bin/freshclam --stdout >> $LOG_DIR/freshclam-cron.log 2>&1
EOF
  fi
}

ensure_port_is_free() {
  if ss -ltn "( sport = :$NGINX_PORT )" | tail -n +2 | grep -q .; then
    if [[ -f "$NGINX_SITE" ]] \
      && grep -Eq "listen[[:space:]]+$NGINX_PORT([[:space:]]+ssl)?;" "$NGINX_SITE" \
      && ss -ltnp "( sport = :$NGINX_PORT )" | grep -q nginx; then
      return
    fi

    echo "Port $NGINX_PORT is already in use. Set INFRA_AV_NGINX_PORT to another port." >&2
    exit 1
  fi
}

build_tls_san_list() {
  local san_entries=(
    "DNS:localhost"
    "IP:127.0.0.1"
  )
  local host_fqdn

  if [[ "$SERVER_NAME" != "_" && -n "$SERVER_NAME" ]]; then
    san_entries+=("DNS:$SERVER_NAME")
  fi

  host_fqdn="$(hostname -f 2>/dev/null || hostname 2>/dev/null || true)"
  if [[ -n "$host_fqdn" && "$host_fqdn" != "localhost" && "$host_fqdn" != "$SERVER_NAME" ]]; then
    san_entries+=("DNS:$host_fqdn")
  fi

  local IFS=,
  printf '%s' "${san_entries[*]}"
}

ensure_tls_certificate() {
  mkdir -p "$(dirname "$TLS_CERT_FILE")" "$(dirname "$TLS_KEY_FILE")"

  if [[ -f "$TLS_CERT_FILE" && -f "$TLS_KEY_FILE" ]]; then
    return
  fi

  if [[ -f "$TLS_CERT_FILE" || -f "$TLS_KEY_FILE" ]]; then
    echo "Both TLS certificate and key must exist together: $TLS_CERT_FILE / $TLS_KEY_FILE" >&2
    exit 1
  fi

  local common_name="127.0.0.1"
  if [[ "$SERVER_NAME" != "_" && -n "$SERVER_NAME" ]]; then
    common_name="$SERVER_NAME"
  fi

  local openssl_config
  openssl_config="$(mktemp)"
  cat >"$openssl_config" <<EOF
[req]
default_bits = 4096
distinguished_name = req_distinguished_name
x509_extensions = v3_req
prompt = no

[req_distinguished_name]
CN = $common_name
O = INFRA-AV

[v3_req]
subjectAltName = $(build_tls_san_list)
keyUsage = digitalSignature, keyEncipherment
extendedKeyUsage = serverAuth
EOF

  if ! openssl req \
    -x509 \
    -nodes \
    -newkey rsa:4096 \
    -sha256 \
    -days "$TLS_CERT_DAYS" \
    -keyout "$TLS_KEY_FILE" \
    -out "$TLS_CERT_FILE" \
    -config "$openssl_config" >/dev/null 2>&1; then
    rm -f "$openssl_config"
    echo "Failed to generate the INFRA-AV TLS certificate." >&2
    exit 1
  fi

  rm -f "$openssl_config"
  chmod 600 "$TLS_KEY_FILE"
  chmod 644 "$TLS_CERT_FILE"
}

write_nginx_site() {
  ensure_port_is_free
  ensure_tls_certificate

  cat >"$NGINX_SITE" <<EOF
server {
    listen $NGINX_PORT ssl;
    server_name $SERVER_NAME;

    ssl_certificate $TLS_CERT_FILE;
    ssl_certificate_key $TLS_KEY_FILE;
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_session_timeout 1d;
    ssl_session_cache shared:INFRAAVSSL:10m;

    error_page 497 =307 https://\$host:\$server_port\$request_uri;

    add_header Strict-Transport-Security "max-age=31536000" always;
    add_header X-Content-Type-Options "nosniff" always;
    add_header X-Frame-Options "SAMEORIGIN" always;
    add_header Referrer-Policy "no-referrer" always;

    root $DEPLOY_FRONTEND_DIR;
    index index.html;

    location /api {
        proxy_pass http://$BACKEND_HOST:$BACKEND_PORT/api;
        proxy_http_version 1.1;
        proxy_set_header Host \$host;
        proxy_set_header X-Real-IP \$remote_addr;
        proxy_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto https;
    }

    location = /favicon.ico {
        access_log off;
        log_not_found off;
        return 204;
    }

    location / {
        try_files \$uri \$uri/ /index.html;
    }
}
EOF

  ln -sf "$NGINX_SITE" "$NGINX_LINK"
  nginx -t
}

enable_services() {
  systemctl daemon-reload
  systemctl enable --now infra-av-backend.service
  systemctl restart infra-av-backend.service
  systemctl enable --now cron
  if [[ "$STANDALONE_FRONT" == 1 ]]; then
    systemctl enable --now nginx
    systemctl restart nginx
  fi
}

warm_up_clamav() {
  flock -n "$FRESHCLAM_LOCK_FILE" freshclam --stdout >/dev/null
}

json_query() {
  local path="$1"
  JSON_QUERY_PATH="$path" node -e '
const fs = require("fs");
const path = process.env.JSON_QUERY_PATH.split(".");
let value = JSON.parse(fs.readFileSync(0, "utf8"));
for (const part of path) {
  value = value?.[part];
}
if (value === undefined || value === null) {
  process.exit(1);
}
if (typeof value === "object") {
  process.stdout.write(JSON.stringify(value));
} else {
  process.stdout.write(String(value));
}
'
}

verify_installation() {
  local required_commands=(
    cargo
    clamscan
    curl
    freshclam
    flock
    lsblk
    nginx
    node
    npm
    openssl
  )
  local command_name
  for command_name in "${required_commands[@]}"; do
    if ! command -v "$command_name" >/dev/null 2>&1; then
      echo "Missing required command after installation: $command_name" >&2
      exit 1
    fi
  done

  local required_files=(
    "$INSTALLED_BINARY"
    "$ENV_FILE"
    "$USERS_FILE"
    "$SERVICE_FILE"
    "$CRON_FILE"
    "$PAM_SERVICE_FILE"
  )
  # The standalone nginx front's artifacts only exist when it is installed.
  if [[ "$STANDALONE_FRONT" == 1 ]]; then
    required_files+=("$NGINX_SITE" "$TLS_CERT_FILE" "$TLS_KEY_FILE")
  fi
  local path
  for path in "${required_files[@]}"; do
    if [[ ! -e "$path" ]]; then
      echo "Missing required installed file: $path" >&2
      exit 1
    fi
  done

  verify_pam_stack

  if ! systemctl is-active --quiet infra-av-backend.service; then
    echo "infra-av-backend.service is not active after installation." >&2
    exit 1
  fi

  if [[ "$STANDALONE_FRONT" == 1 ]]; then
    if ! systemctl is-active --quiet nginx; then
      echo "nginx is not active after installation." >&2
      exit 1
    fi
  fi

  if ! systemctl is-active --quiet cron; then
    echo "cron is not active after installation." >&2
    exit 1
  fi

  if [[ "$STANDALONE_FRONT" == 1 ]]; then
    if ! curl -kfsS "https://127.0.0.1:$NGINX_PORT/" >/dev/null; then
      echo "HTTPS frontend check failed on https://127.0.0.1:$NGINX_PORT/." >&2
      exit 1
    fi

    if [[ "$(curl -ksS -o /dev/null -w '%{http_code}' "https://127.0.0.1:$NGINX_PORT/favicon.ico")" != "204" ]]; then
      echo "Favicon removal check failed: expected HTTPS /favicon.ico to return 204." >&2
      exit 1
    fi
  fi

  if ! curl -fsS "http://127.0.0.1:$BACKEND_PORT/api" \
    -H 'Content-Type: application/json' \
    -d '{"action":"health.check","payload":{}}' >/dev/null; then
    echo "Backend API health check failed on http://127.0.0.1:$BACKEND_PORT/api." >&2
    exit 1
  fi

  # Authenticated checks need a password we know. When the installer REUSED a pre-existing
  # admin account (FALLBACK_ACCOUNT_MODE=reused) its password is unknown — ADMIN_PASSWORD
  # is empty — so logging in would fail with HTTP 400. The health check above already
  # proves the backend is up; skip the authenticated probes in that case.
  if [[ "$FALLBACK_ACCOUNT_MODE" != "created" || -z "$ADMIN_PASSWORD" ]]; then
    echo "Admin account reused (password preserved); skipping authenticated verification probes." >&2
    return
  fi

  local login_json token browse_json signatures_json
  login_json="$(
    curl -fsS "http://127.0.0.1:$BACKEND_PORT/api" \
      -H 'Content-Type: application/json' \
      -d "{\"action\":\"auth.login\",\"payload\":{\"username\":\"$ADMIN_USER\",\"password\":\"$ADMIN_PASSWORD\"}}"
  )"
  token="$(printf '%s' "$login_json" | json_query "data.token")"
  if [[ -z "$token" ]]; then
    echo "Backend login verification failed." >&2
    exit 1
  fi

  browse_json="$(
    curl -fsS "http://127.0.0.1:$BACKEND_PORT/api" \
      -H 'Content-Type: application/json' \
      -H "Authorization: Bearer $token" \
      -d '{"action":"scan.browse","payload":{"scanType":"directory"}}'
  )"
  if [[ "$(printf '%s' "$browse_json" | json_query "data.entries.length")" -lt 1 ]]; then
    echo "Scan target browser verification failed: no directory roots were returned." >&2
    exit 1
  fi

  if [[ "$ALLOW_DEVICE_SCANS" == "1" ]]; then
    browse_json="$(
      curl -fsS "http://127.0.0.1:$BACKEND_PORT/api" \
        -H 'Content-Type: application/json' \
        -H "Authorization: Bearer $token" \
        -d '{"action":"scan.browse","payload":{"scanType":"device"}}'
    )"
    printf '%s' "$browse_json" | json_query "data.entries.length" >/dev/null || {
      echo "Device browser verification failed." >&2
      exit 1
    }
  fi

  signatures_json="$(
    curl -fsS "http://127.0.0.1:$BACKEND_PORT/api" \
      -H 'Content-Type: application/json' \
      -H "Authorization: Bearer $token" \
      -d '{"action":"signatures.update","payload":{}}'
  )"
  if [[ "$(printf '%s' "$signatures_json" | json_query "data.exitCode")" != "0" ]]; then
    echo "Signature update verification failed." >&2
    exit 1
  fi
}

fix_project_permissions() {
  chown -R "$SOURCE_OWNER" "$PROJECT_ROOT"
}

display_web_host() {
  if [[ "$SERVER_NAME" != "_" && -n "$SERVER_NAME" ]]; then
    printf '%s' "$SERVER_NAME"
    return
  fi

  printf '%s' "127.0.0.1"
}

print_summary() {
  local web_host
  web_host="$(display_web_host)"

  local web_access tls_summary front_service
  if [[ "$STANDALONE_FRONT" == 1 ]]; then
    web_access="Web UI (HTTPS only): https://$web_host:$NGINX_PORT/"
    tls_summary="TLS files:
  Certificate: $TLS_CERT_FILE
  Private key: $TLS_KEY_FILE
"
    front_service="  $NGINX_SITE
"
  else
    web_access="Web UI: the unified UUUUI shell on https://<host>:31337/ (INFRA-AV tab). The standalone nginx front was removed."
    tls_summary=""
    front_service=""
  fi

  cat <<EOF

INFRA-AV installation completed.

Usage:
  $web_access

Authentication:
  PAM service:         $PAM_SERVICE
  System login:        use your Linux username and password
  Local fallback user: $ADMIN_USER
  Local fallback pass: $FALLBACK_PASSWORD_SUMMARY
  Update signatures:   use the frontend button or run sudo /usr/bin/freshclam --stdout

Frontend Features:
  Target selection:    explorer-style browser for files, folders, and devices
  Scan controls:       pause, resume, and stop on live clamscan processes

Ports:
  backend internal API: $BACKEND_HOST:$BACKEND_PORT/tcp

Backend API:
  Direct internal URL: http://$BACKEND_HOST:$BACKEND_PORT/api

${tls_summary}Service files:
  $SERVICE_FILE
  $CRON_FILE
${front_service}  $PAM_SERVICE_FILE
  $ENV_FILE

Installed binary:
  $INSTALLED_BINARY

Maintenance:
  Reinstall (keeps data):  ./installer.sh
  Uninstall + purge data:  ./installer.sh uninstall

EOF
}

main() {
  elevate_if_needed "$@"
  require_root
  if [[ "$INSTALL_MODE" == "uninstall" || "$INSTALL_MODE" == "--uninstall" ]]; then
    uninstall_existing_install
    fix_project_permissions
    print_uninstall_summary
    return
  fi
  if [[ "$INSTALL_MODE" != "install" ]]; then
    echo "Unsupported installer mode: $INSTALL_MODE" >&2
    exit 1
  fi
  remove_service_artifacts
  check_frontend_tree
  validate_pam_service_name
  ensure_generated_values
  install_packages
  verify_pam_stack
  write_freshclam_config
  install_rust
  build_backend
  deploy_backend
  build_frontend
  if [[ "$STANDALONE_FRONT" == 1 ]]; then deploy_frontend; fi
  write_env_file
  bootstrap_admin_user
  write_pam_service
  write_systemd_service
  write_cron_file
  if [[ "$STANDALONE_FRONT" == 1 ]]; then write_nginx_site; fi
  warm_up_clamav
  enable_services
  verify_installation
  fix_project_permissions
  print_summary
}

main "$@"
