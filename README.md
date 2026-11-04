# INFRA-AV

INFRA-AV is a split frontend/backend antivirus web interface powered by ClamAV:

- `frontend/`: React + Vite UI for login, scan configuration, command preview, health, queue state, and scan result polling.
- `backend/`: Rust + Actix-Web API plus a cron-friendly worker that reads queued JSON jobs and runs `clamscan`.
- `installer.sh`: end-to-end Debian installer for ClamAV, Rust, Node, nginx, systemd, and cron.

## Project Status

The original codebase was a frontend-only Vite app with a README describing a non-existent Flask backend. The implemented project now matches the working architecture below:

1. The React UI sends only `POST /api` requests with an `action` field, as required by `TODO.md`.
2. The Rust backend authenticates users through PAM first, falls back to the local INFRA-AV user file, validates scan requests, writes enriched JSON queue files, and exposes scan, browse, control, health, and signature actions.
3. The worker reads pending queue JSON files, rebuilds the `clamscan` argv list safely, executes the real `clamscan` process without a shell, supports pause/resume/stop signaling, parses the summary, and stores the result.
4. nginx serves the frontend over HTTPS only and reverse-proxies `/api` to the Rust backend.
5. cron runs the worker periodically in production.

## Directory Layout

```text
INFRA-AV/
├── backend/
│   ├── config/
│   ├── deploy/
│   ├── src/
│   └── Cargo.toml
├── frontend/
│   ├── src/
│   └── package.json
├── installer.sh
├── README.md
└── TODO.md
```

## Backend Design

The backend follows the `TODO.md` direction for a Rust/Actix implementation and a POST-only API:

- Endpoint: `POST /api`
- Envelope:

```json
{
  "action": "scan.submit",
  "payload": {
    "timestamp": "2026-03-23T00:00:00.000Z",
    "user": "admin",
    "type": "av",
    "target": "/home/admin/Documents",
    "scanType": "directory",
    "options": { "recursive": true }
  }
}
```

- Supported actions:
  - `auth.login`
  - `health.check`
  - `scan.browse`
  - `scan.submit`
  - `scan.status`
  - `scan.report`
  - `scan.list`
  - `scan.pause`
  - `scan.resume`
  - `scan.stop`
  - `scan.runWorkerOnce`
  - `signatures.update`

### Queue File Contract

The frontend still submits the raw scan request shape, but the backend stores an enriched job file under `pending/`, `processing/`, `done/`, or `failed/`:

```json
{
  "schema_version": 1,
  "request_id": "20260323001815.admin.1234abcd.av",
  "submitted_at": "2026-03-23T00:18:15Z",
  "submitted_by": "admin",
  "target": "/home/admin/Documents",
  "scan_type": "directory",
  "options": { "recursive": true },
  "status": "pending",
  "command_preview": "clamscan ...",
  "execution": {
    "worker_pid": 12345,
    "child_pid": 12346,
    "started_at": "2026-03-23T00:18:16Z",
    "paused": false,
    "cancel_requested": false
  },
  "server_metadata": {
    "client_timestamp": "2026-03-23T00:18:15.000Z",
    "client_user": "admin"
  }
}
```

### Worker Responsibilities

- Read the oldest file from `action/pending/`
- Atomically move it to `action/processing/`
- Rebuild the `clamscan` argument list from the stored JSON
- Enforce path policy before execution
- Record the live `clamscan` PID in the job file
- Accept pause (`SIGSTOP`), resume (`SIGCONT`), and stop (`SIGTERM`/`SIGKILL`) requests
- Write server-controlled logs to `log/<user>/<requestId>.clamscan.log`
- Parse the ClamAV scan summary
- Store final JSON in `action/done/` or `action/failed/`
- Persist an HTML report in `result/<requestId>.html`

### ClamAV Validation Rules

- Absolute paths only
- File and directory scans must stay under configured scan roots
- Device scans follow `INFRA_AV_ALLOW_DEVICE_SCANS`; the installer enables them by default
- `remove`, `move`, and `copy` are mutually exclusive
- `log` from the browser is ignored; the backend writes its own log path
- `database`, `move`, `copy`, and `tempdir` are restricted to safe server roots
- The original frontend bug `--alert-broken-executables` was fixed to `--alert-broken=yes`
- Symlink behavior now explicitly uses `--follow-dir-symlinks=0|1` and `--follow-file-symlinks=0|1`

## Frontend Notes

The frontend now:

- Accepts Linux PAM credentials as well as the installer-generated local fallback account
- Uses the POST-only action API instead of the old `/api/auth/login` and `/api/scan/submit` endpoints
- Shows an explorer-style target browser with a locations sidebar, breadcrumbs, and navigable directory listings
- Polls `scan.status` after queueing a scan
- Supports live pause, resume, and stop controls while `clamscan` is running
- Shows real backend health and queue depth in the header
- Triggers `signatures.update` from the header button
- Replaces the fake scan statistics with actual parsed ClamAV summary data
- Fixes the command preview so it matches the backend’s real flag mapping
- Renders completed scans as an INFRA AV HTML report and lets the user download that report

## Installer

Run the installer on Debian as root:

```bash
cd /home/spabam/Dropbox/Documenti/Secondary-Projects/INFRA-AV
sudo ./installer.sh
```

The installer will:

1. Uninstall the previous INFRA-AV deployment first.
2. Install ClamAV, nginx, cron, Node.js, npm, and Rust if missing.
3. Write an INFRA-AV-managed `/etc/clamav/freshclam.conf`, disable the conflicting `clamav-freshclam` daemon, and switch updates to one-shot `freshclam --stdout`.
4. Build the Rust backend in `backend/target/release/`.
5. Build the frontend into `frontend/dist/`.
6. Generate `/etc/infra-av/backend.env`.
7. Generate `/etc/infra-av/users.json` with a bcrypt-hashed local fallback admin user.
8. Install `/etc/pam.d/infra-av` so Linux system credentials can authenticate against INFRA-AV.
9. Install a systemd service for the backend API.
10. Install `/etc/cron.d/infra-av` for the worker and freshclam updates.
11. Generate or reuse a TLS certificate and install an HTTPS-only nginx site on a configurable port.
12. Warm the ClamAV signature database and verify `auth.login`, `scan.browse`, and `signatures.update`.

To remove the deployment without reinstalling:

```bash
sudo ./installer.sh uninstall
```

### Important Installer Environment Variables

- `INFRA_AV_BACKEND_PORT`: backend bind port, default `5000`
- `INFRA_AV_NGINX_PORT`: nginx listen port, default `8099`
- `INFRA_AV_SERVER_NAME`: nginx server_name, default `_`
- `INFRA_AV_PAM_SERVICE`: PAM service name, default `infra-av`
- `INFRA_AV_DEFAULT_ROLE`: role granted to PAM-authenticated users not listed in the local user file, default `admin`
- `INFRA_AV_TLS_CERT_FILE`: optional TLS certificate path; autogenerated if missing
- `INFRA_AV_TLS_KEY_FILE`: optional TLS private key path; autogenerated if missing
- `INFRA_AV_TLS_CERT_DAYS`: self-signed certificate lifetime, default `825`
- `INFRA_AV_JWT_SECRET`: JWT signing secret
- `INFRA_AV_ADMIN_USER`: initial admin username
- `INFRA_AV_ADMIN_PASSWORD`: initial admin password
- `INFRA_AV_ALLOWED_SCAN_ROOTS`: comma-separated scan roots, default `/home,/srv,/tmp`
- `INFRA_AV_ALLOW_DEVICE_SCANS`: `1` to enable block-device scans, default `1`
- `INFRA_AV_ENABLE_FRESHCLAM_CRON`: `1` to install cron-based freshclam updates, default `1`

## Cron Specification

The installer writes this worker cron entry:

```cron
* * * * * root flock -n /var/lock/infra-av-worker.lock /path/to/backend/target/release/infra-av-backend worker --once >> /var/log/infra-av/worker-cron.log 2>&1
```

The installer writes this freshclam cron entry by default:

```cron
17 */4 * * * root flock -n /var/lock/infra-av-freshclam.lock /usr/bin/freshclam --stdout >> /var/log/infra-av/freshclam-cron.log 2>&1
```

## PAM Specification

The installer writes `/etc/pam.d/infra-av` with a non-interactive PAM policy based on the Debian common stacks:

```pam
auth      requisite pam_nologin.so
auth      include   common-auth
account   include   common-account
session   include   common-session-noninteractive
```

That lets the web UI accept a normal Linux username and password while still keeping the installer-generated local fallback admin account available.

## freshclam Behavior

- The installer disables the package-managed `clamav-freshclam.service` to avoid the old log-file lock conflict.
- Manual signature updates should be run as root: `sudo /usr/bin/freshclam --stdout`
- The frontend `Update Signatures` button calls the backend action that runs the same one-shot update flow.
- Running `freshclam` as an unprivileged user still cannot update `/var/lib/clamav`, which is expected.

## nginx Specification

The generated nginx site serves the built frontend over TLS and reverse-proxies the API:

```nginx
server {
    listen 8099 ssl;
    server_name _;

    ssl_certificate /etc/infra-av/tls/infra-av.crt;
    ssl_certificate_key /etc/infra-av/tls/infra-av.key;
    error_page 497 =307 https://$host:$server_port$request_uri;

    root /path/to/INFRA-AV/frontend/dist;
    index index.html;

    location /api {
        proxy_pass http://127.0.0.1:5000/api;
    }

    location / {
        try_files $uri $uri/ /index.html;
    }
}
```

The installer intentionally uses a dedicated HTTPS port by default to avoid conflicts with existing nginx sites. Change `INFRA_AV_NGINX_PORT` if `8099` is already in use.

## Verification

At the time of implementation:

- Rust backend: `cargo check` passes
- Frontend: `npm run build` passes
- Frontend tests: `npm test` passes
- `clamscan --version`: `1.4.3`
- `freshclam --version`: `1.4.3`

If you want the frontend linter to run cleanly, run it without parallel build jobs. ESLint 9 and the Vite config loader can race on a transient generated timestamp module when build and lint are started at the same time.

<!-- VANTAGE-PYTHON-ENV-STANDARD:START -->
## VANTAGE Python environment standard

All Python tools in this VANTAGE workspace use one environment per Unix account: `$HOME/.venv`. Do not create or activate a project-local `venv`, `.venv`, `.venv-*`, or per-tool pipx/uv environment.

From the VANTAGE workspace root:

```bash
./python-env.sh ensure
./python-env.sh pip install -r path/to/requirements.txt
./python-env.sh python path/to/script.py
./python-env.sh audit
```

Direct commands, when a project script cannot call the workspace helper:

```bash
test -x "$HOME/.venv/bin/python" || python3 -m venv "$HOME/.venv"
"$HOME/.venv/bin/python" -m pip install -r requirements.txt
"$HOME/.venv/bin/python" script.py
```

Activation is unnecessary. Always invoke `$HOME/.venv/bin/python` directly and run pip only as `$HOME/.venv/bin/python -m pip`. Containers use the container account's `$HOME/.venv`; only explicitly documented OS-managed service runtimes are exceptions.
<!-- VANTAGE-PYTHON-ENV-STANDARD:END -->

## UUUUI unified interface

INFRA-AV is also the **Antivirus** tab of the VANTAGE UUUUI interface. The same
Rust backend serves both modes:

- **Standalone:** its own HTTPS origin + login (unchanged).
- **Unified:** the UUUUI shell mounts a native Antivirus tab that calls the
  backend through the gateway at `/api/av/` (loopback `127.0.0.1:8090`).

**Single sign-on.** The backend accepts the UUUUI broker hand-off token: the tab
fetches `GET /api/session/token` and posts `{"action":"auth.sso","payload":{"token":…}}`
to `/api/av/`; the backend verifies it with `INFRA_AV_BROKER_SECRET` (the same
value as the UUUUI broker's `UUUUI_BACKEND_SECRET`, HS256) and issues its own
token — no second login. Unset `INFRA_AV_BROKER_SECRET` ⇒ `auth.sso` disabled and
standalone behaviour is unchanged (`backend/src/auth.rs::verify_broker_token`).

Allowed scan roots default to `/home`, `/srv`, `/tmp`
(`INFRA_AV_ALLOWED_SCAN_ROOTS`); targets outside them are rejected with a clear
error. Verified end-to-end through the UUUUI tab: a scan of an EICAR test file
reports `Eicar-Test-Signature FOUND` with a full ClamAV summary and report.
