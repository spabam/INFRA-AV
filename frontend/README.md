# INFRA-AV Frontend

This directory contains the React/Vite web UI for INFRA-AV.

## Responsibilities

- Login against the Rust backend
- Accept Linux PAM credentials or the local INFRA-AV fallback account
- Build scan requests as JSON
- Preview the generated `clamscan` command
- Queue scans through the POST-only action API
- Poll scan status and render the real ClamAV summary
- Show engine health and trigger signature updates

## Development

```bash
cd frontend
npm install
VITE_API_BASE=http://127.0.0.1:5000 npm run dev
```

For the full project architecture, installer, cron, and nginx setup, use the root [`README.md`](../README.md).

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
