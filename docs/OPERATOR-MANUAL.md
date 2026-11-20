# VANTAGE Antivirus Operator Manual

Antivirus is the INFRA-AV interface for on-demand ClamAV scans of authorized files, directories, and supported devices on the server. It provides a queue, a scan monitor, results, and recent history. Use the report to record what the engine scanned and which detections need review.

## 1. Open Antivirus and find documentation

Sign in to UUUUI. If UUUUI groups SAST and Antivirus, open **SAST** and select **Antivirus** in the module navigation. On hosts with Antivirus alone, select the **Antivirus** tab. The direct route is `/av`. The UUUUI session signs you in to Antivirus without a second login. Select **Documentation** in the Antivirus header to browse and search this manual.

Select **Platform documentation** in the UUUUI footer for general help, or open the footer **Chatbot** and ask a question such as “How do I reopen an Antivirus scan?” When platform documentation is available, the assistant can use relevant excerpts from the official manual. Open the scan in Antivirus to read its live status, target path, and report under your own grant. [UUUUI Operator Manual](../../../System/UUUUI/docs/OPERATOR-MANUAL.md)

## 2. Check access and choose a scan target

### Antivirus grant and roles

UUUUI checks whether Antivirus is installed, enabled, and granted to the signed-in account. Antivirus applies its role to each scan action. A member can submit permitted scans and open or control their own scans. An Antivirus admin can also view and control other users' scans and run **Update signatures**. The backend checks ownership even when someone opens a direct scan link. The account's SAST grant is independent of Antivirus access.

### Select a file, directory, or device

1. In **New scan**, set **Type** to **Directory**, **File**, or **Device**.
2. In **Target**, enter an absolute server path or choose **Browse…**. The picker navigates the server's permitted locations. For a directory choose **Select this folder**. For a file or device choose its name or **select**.
3. Review the path and selected type before starting. File and directory scans must stay under the host's configured scan roots. Common defaults are `/home`, `/srv`, and `/tmp`. A device appears only when it is available and the host permits device scans. Use an authorized target that the scanner can actually read.

## 3. Start and monitor a scan

### Review the scan options

| Option | Effect |
| --- | --- |
| **Recursive** | Includes subdirectories when scanning a directory. |
| **Only log infected** | Limits scan output to detections. Review the final summary for counts. |
| **Detect PUA** | Includes potentially unwanted application detections. Review them using your organization's classification policy. |
| **Heuristic alerts** | Enables alerts from heuristic checks. |
| **Scan archives** | Lets ClamAV inspect supported archive contents within engine limits. |
| **Remove infected (destructive)** | Permanently removes detected files when enabled and confirmed. Keep it off when collecting evidence or running a diagnostic scan. Reopening the page resets this option to off. |

### Start scan and manage its progress

Choose **Start scan**. The request first appears as **Queued** until a worker picks it up. **Scan monitor** shows the selected target and its state. It reconnects after a temporary status error. Leaving the browser or switching modules can interrupt viewing while the server continues processing the queue.

| Monitor state | Available action |
| --- | --- |
| **Queued** | Use **Stop** to cancel the queued request. |
| **Running** | Use **Pause** to suspend processing or **Stop** to cancel. |
| **Paused** | Use **Resume** to continue or **Stop** to cancel. |
| **Done**, **Failed**, **Cancelled** | Review the final state, errors, and any report. |

If the queue starts a scan during cancellation, refresh the monitor and retry **Stop** if the request still shows as running. Reopen a known scan using **Scan history** before submitting a duplicate after a browser interruption.

## 4. Interpret and reopen results

### Read the summary and HTML report

**Done** is the job state. The summary displays **Clean** when the scan completed and reported zero infections. A positive infection count is shown even when the scan is incomplete. **Result unknown** means the completed job lacked a conclusive clean summary. **Scan incomplete** covers unfinished, failed, or cancelled work as appropriate. Review scan limits, errors, unreadable files, and actual coverage before relying on a result.

When **View report** appears, choose it to open the isolated HTML report inside the monitor. Review the target, coverage counts, duration, detected signatures, and scan output. Share report data only with recipients authorized to see the server paths and findings. A documentation assistant can explain what a status means using the manual. The source report remains in Antivirus.

### Find a previous scan

Under **Scan history**, select a target row to reopen it. Use **Filter scan history** to match the target, status, or submitting user among the loaded rows. Choose **Load more scans** to load older rows when available or **Refresh history** to retry the current list. The interface remembers the last scan and non-destructive settings for your signed-in account in this browser.

The direct link format is `/av?scan=<request-id>`. A recipient needs the Antivirus grant and permission to view the scan. Providing the request ID to an authorized support operator can help them trace the queue entry.

## 5. Review engine health and update signatures

At the top of Antivirus, the health strip shows `clamscan`, `freshclam`, queue totals, and the backend version. Engine reachability and queue health help diagnose whether a request can progress. The check on `freshclam` shows whether the updater is reachable. Check its update output when you need to confirm a refresh result.

An Antivirus admin can select **Update signatures**. Wait for **Signatures updated successfully.** or review the displayed failure. This runs the server's configured `freshclam` updater and shares its update lock with the scheduled refresh. If a refresh is already running, allow it to finish before retrying.

## 6. Install, use the API, and troubleshoot

### Service and API

Include `av` when selecting modules in the unified UUUUI installer. The Antivirus backend runs as `infra-av-backend.service`. The integrated gateway routes `/api/av/` to the backend on `127.0.0.1:8090` by default. The installer configures the scan queue worker and optional scheduled signature updates. For standalone installation and server configuration, see the project README. [INFRA-AV README](../README.md)

The native UI uses the authenticated `POST /api/av/` action API. Read operations include `health.check`, `scan.browse`, `scan.list`, `scan.status`, and `scan.report`. Submit and control actions are `scan.submit`, `scan.pause`, `scan.resume`, and `scan.stop`. The caller's role and scan ownership determine access. `GET /api/av/openapi.json` supplies the published contract. `GET /api/av/health` and `GET /api/av/capabilities` show version and capabilities. Use the same platform session and gateway as the UI.

### Troubleshoot by symptom

| What you see | What to check |
| --- | --- |
| Antivirus is missing | Ask a platform administrator to check installation, module activation, and your Antivirus grant. Look under the **SAST** tab when both products are installed. |
| **Couldn't start an Antivirus session** | Choose **Retry**. Check the current UUUUI sign-in, module grant, and the broker-to-Antivirus configuration if it persists. |
| **Engine unreachable** | Check backend health and gateway reachability. On the host, `systemctl status infra-av-backend.service` shows service status. |
| A target is missing or rejected | Verify the selected type, allowed scan roots, device policy, and scanner access on the server. |
| The scan stays **Queued** | Review the queue counts, check whether a worker is processing an older request, and ask the host operator to verify the scheduled worker. |
| A completed report is missing | Verify the request ID and ownership, then review the final job state and any scanner error. Report availability follows the result stored by the worker. |
| **Update signatures** fails with a permission error | Check the Antivirus admin role. For an engine error, review the updater output and scheduled update status. |

When requesting support, include the scan request ID, time, displayed state, and error message. Share paths, report content, and detection details only through the approved support channel.
