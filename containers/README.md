# Antivirus OCI deployment

Deploy the UUUUI service from this product directory with
`sudo bash installer.sh --containers --apply`. Use `--containers --dry-run`
to preview or `--containers --status` to check the result.

Build the UUUUI-integrated Antivirus backend from the recorded product release.

```bash
bash Vulnerabilities/Antivirus/installer.sh --container-image
```

The build publishes the local image as `vantage/antivirus:<release>`. Its three
roles run from the same image. The UUUUI Antivirus page and the existing
`/av-app` embed continue to use the installed gateway.

| Container | Image arguments | Network | Persistent mounts |
| --- | --- | --- | --- |
| `av-api` | `server` | Host loopback at `127.0.0.1:8090` for the gateway | Shared scan state and scan targets. Read-write signature database for administrator updates. |
| `av-worker` | `worker` | None | Shared scan state and scan targets. Read-only signature database. |
| `av-signatures` | `signatures` | Outbound DNS and HTTPS for ClamAV updates | Shared state `temp` subdirectory and read-write signature database. |

Keep the scan state at its configured path, normally `/var/lib/infra-av`.
Mount the same queue, logs, result files, temporary storage, and quarantine in
the API and worker. Keep scan roots and custom database roots at their
configured absolute paths in both containers. Choose read-write access only
for target roots that require quarantine moves or removal, and writable
destination roots only for quarantine or copies. A read-only target mount
permits scanning and copying detections to a writable quarantine directory.
Keep device scans disabled for the container profile. The default host
configuration uses `/home`, `/srv`, and `/tmp`. The image keeps its own
temporary files in the shared state `temp` directory.
Mount `/srv/vantage-storage-mounts` and its registered mounts read-only. A
destructive scan cannot target a protected mount, recursively scan through it,
or follow symlinks to an unreviewed location. Scanning and copying detections
to a separate writable quarantine remain available.

Keep API configuration in a protected environment file. Retain
`INFRA_AV_JWT_SECRET`, `INFRA_AV_BROKER_SECRET`, any previous signing key
during a rotation, and `UUUUI_DIRECTORY_URL` so the server accepts the
gateway's signed AV identity. Keep the installed CORS origins. Worker and
signature containers only need their role's path and ClamAV configuration.
The UUUUI PAM broker handles Linux user sign-in on the host and issues signed
requests to the API.

Use a read-only root filesystem and the runtime's init process. Start the
runtime with dropped capabilities. Grant `DAC_OVERRIDE` for mounted scan
targets when the configured host scanner needs to read or quarantine files
owned by other accounts. Removing entries from sticky directories owned by
other accounts also requires `FOWNER`. Grant `SETUID` and `SETGID` to the API and signature
roles so FreshClam can switch to the image's `clamav` account. The worker
signals only the ClamAV child it started, in its own PID namespace. For the
existing administrator action `scan.runWorkerOnce`, the API can process one
job using the same exclusive queue lock and mounted scan paths.

Use a dedicated directory for the OCI signature database at
`/var/lib/clamav`. Seed it from verified existing signatures where available.
Use `id -u clamav` and `id -g clamav` inside the built image to determine
ownership for the new directory. Preserve the original host signature store
when preparing a replacement. The default image FreshClam configuration uses
`database.clamav.net`. Set `INFRA_AV_FRESHCLAM_CONF` to the container path
of a mounted config for sites with a different approved mirror. The scheduler
refreshes at startup and then every four hours. The API's manual refresh
and the scheduler share one lock in the scan state's `temp` directory.

## Cutover and verification

Verify `GET /api/health` on a private port and exercise a signed request with a
benign fixture. Before final transfer, fence new scan submissions and wait
for active jobs to finish. Pause native `uuuui-av.service` and
`uuuui-av-worker.service` before attaching the shared state to replacements.
Retain native units and runtime data for recovery. The native UUUUI signature
updater is `/etc/cron.d/uuuui-av-freshclam`; check existing host FreshClam
jobs before selecting which updater writes each database directory.

The queue lock prevents two container workers from consuming the same job.
After an interruption, a replacement worker marks leftover processing
records as failed and leaves pending records queued. Check the target before
resubmitting a job that may have moved or deleted a file before the interruption.
Scan reports and quarantined files remain in the mounted state.
