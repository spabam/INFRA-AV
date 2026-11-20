"""Exercise the compiled API and worker with disposable paths and a local CLI.

Build first: cargo build. Override INFRA_AV_TEST_BINARY to test another build.
No installed service, credentials, signatures, scans or user files are touched.
"""
import base64
import hashlib
import hmac
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import tempfile
import threading
import time
import unittest
import urllib.error
import urllib.request

BINARY = Path(os.environ.get("INFRA_AV_TEST_BINARY", Path(__file__).resolve().parents[1] / "target/debug/infra-av-backend"))


@unittest.skipUnless(BINARY.is_file(), "Build the backend first with cargo build")
class ApiLifecycleTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="infra-av-api-fixture-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.target = self.root / "targets/fixture.txt"
        self.target.parent.mkdir()
        self.target.write_text("harmless API fixture")
        self.ticks = self.root / "ticks"
        self.cli = self.root / "clamscan-fixture"
        self.cli.write_text(f"#!/usr/bin/python3\nimport sys,time\nfrom pathlib import Path\nif '--version' in sys.argv: print('fixture scanner');sys.exit(0)\nprint('fixture scan started',flush=True)\nfor tick in range(600):\n Path({str(self.ticks)!r}).write_text(str(tick))\n time.sleep(.05)\n")
        self.cli.chmod(0o755)
        with socket.socket() as reserve:
            reserve.bind(("127.0.0.1", 0))
            port = reserve.getsockname()[1]
        self.url = f"http://127.0.0.1:{port}/api"
        self.secret = "disposable-test-secret"
        self.env = {**os.environ, "INFRA_AV_PROJECT_ROOT": str(self.root),
            "INFRA_AV_DATA_DIR": str(self.root / "data"), "INFRA_AV_USERS_FILE": str(self.root / "users.json"),
            "INFRA_AV_ALLOWED_SCAN_ROOTS": str(self.target.parent), "INFRA_AV_ALLOWED_WRITE_ROOTS": str(self.root / "data/quarantine"),
            "INFRA_AV_HOST": "127.0.0.1", "INFRA_AV_PORT": str(port), "INFRA_AV_JWT_SECRET": self.secret,
            "INFRA_AV_CLAMSCAN_BIN": str(self.cli), "INFRA_AV_FRESHCLAM_BIN": "/bin/true", "UUUUI_DIRECTORY_URL": ""}
        self.log = (self.root / "server.log").open("w+")
        self.addCleanup(self.log.close)
        self.server = subprocess.Popen([str(BINARY), "server"], env=self.env, stdout=self.log, stderr=self.log, start_new_session=True)
        self.addCleanup(self.stop_process, self.server)
        for _ in range(100):
            try:
                self.api("health.check")
                break
            except (urllib.error.URLError, ConnectionError):
                time.sleep(.05)
        else:
            self.fail("fixture backend did not start")

    @staticmethod
    def stop_process(proc):
        # Each fixture owns a session, including scanner children. Cleanup must
        # also work if an assertion fails while a child is paused or running.
        for sig in (signal.SIGCONT, signal.SIGTERM):
            try:
                os.killpg(proc.pid, sig)
            except ProcessLookupError:
                pass
        if proc.poll() is None:
            try:
                proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                pass
        try:
            os.killpg(proc.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        proc.wait(timeout=5)

    def token(self, username="fixture-owner", role="admin"):
        enc = lambda value: base64.urlsafe_b64encode(json.dumps(value).encode()).rstrip(b"=")
        header = enc({"alg": "HS256", "typ": "JWT"})
        payload = enc({"sub": username, "role": role, "iat": int(time.time()), "exp": int(time.time()) + 300})
        content = header + b"." + payload
        signature = base64.urlsafe_b64encode(hmac.new(self.secret.encode(), content, hashlib.sha256).digest()).rstrip(b"=")
        return (content + b"." + signature).decode()

    def api(self, action, payload=None, username="fixture-owner", role="admin", ok=True):
        request = urllib.request.Request(self.url, json.dumps({"action": action, "payload": payload or {}}).encode(),
            headers={"Content-Type": "application/json", "Authorization": "Bearer " + self.token(username, role)})
        try:
            response = urllib.request.urlopen(request, timeout=10)
        except urllib.error.HTTPError as error:
            response = error
        with response:
            body = json.load(response)
        self.assertEqual(body["ok"], ok, body)
        return body.get("data") if ok else body["error"]

    def submit(self):
        return self.api("scan.submit", {"timestamp": "2026-01-01T00:00:00Z", "user": "untrusted-client-name", "type": "av",
            "target": str(self.target), "scanType": "file", "options": {}})["requestId"]

    def test_pause_resume_stop_preserve_truthful_terminal_results_and_owner_scope(self):
        request_id = self.submit()
        worker = subprocess.Popen([str(BINARY), "worker", "--once"], env=self.env, stdout=self.log, stderr=self.log, start_new_session=True)
        self.addCleanup(self.stop_process, worker)
        for _ in range(100):
            if self.ticks.exists():
                break
            time.sleep(.05)
        self.assertTrue(self.ticks.exists(), "fixture worker did not execute")
        state = self.api("scan.status", {"requestId": request_id})
        self.assertEqual(state["status"], "processing")
        self.assertEqual(state["submittedBy"], "fixture-owner")
        self.api("scan.status", {"requestId": request_id}, username="different-owner", role="member", ok=False)
        paused = self.api("scan.pause", {"requestId": request_id})
        self.assertEqual(paused["status"], "paused")
        tick = self.ticks.read_text()
        time.sleep(.2)
        self.assertEqual(self.ticks.read_text(), tick, "paused worker kept executing")
        self.assertEqual(self.api("scan.resume", {"requestId": request_id})["status"], "processing")
        time.sleep(.2)
        self.assertNotEqual(self.ticks.read_text(), tick)
        self.assertEqual(self.api("scan.stop", {"requestId": request_id})["status"], "cancelled")
        worker.wait(timeout=5)
        state = self.api("scan.status", {"requestId": request_id})
        self.assertEqual(state["status"], "cancelled")
        self.assertIsNotNone(state["finishedAt"])
        self.assertIn("stopped by user", state["errorMessage"])
        self.assertTrue(state["reportAvailable"])
        self.assertIn("fixture scan started", state["output"])
        report = self.api("scan.report", {"requestId": request_id})["html"].lower()
        self.assertIn("scan stopped", report)
        self.assertIn("unknown", report)
        self.assertNotIn("no infected files were reported", report)

    def test_slow_health_probe_does_not_block_service_shutdown(self):
        started = self.root / "probe-started"
        self.cli.write_text(
            "#!/usr/bin/python3\nimport time\nfrom pathlib import Path\n"
            f"Path({str(started)!r}).touch()\ntime.sleep(12)\nprint('fixture scanner')\n")
        errors = []

        def slow_request():
            try:
                self.api("health.check")
            except (OSError, ConnectionError) as error:
                errors.append(error)  # shutdown may close the in-flight request

        client = threading.Thread(target=slow_request, daemon=True)
        client.start()
        for _ in range(100):
            if started.exists():
                break
            time.sleep(.02)
        self.assertTrue(started.exists(), "slow probe did not begin")
        self.server.terminate()
        self.assertEqual(self.server.wait(timeout=8), 0)
        client.join(timeout=2)
        self.assertFalse(client.is_alive())

    def test_pending_cancel_never_executes_and_outside_roots_are_rejected(self):
        request_id = self.submit()
        self.assertEqual(self.api("scan.stop", {"requestId": request_id})["status"], "cancelled")
        subprocess.run([str(BINARY), "worker", "--once"], env=self.env, check=True, stdout=self.log, stderr=self.log, timeout=5)
        self.assertFalse(self.ticks.exists())
        self.assertEqual(self.api("scan.status", {"requestId": request_id})["status"], "cancelled")
        self.api("scan.submit", {"timestamp": "2026-01-01T00:00:00Z", "user": "fixture", "type": "av",
            "target": "/etc/passwd", "scanType": "file", "options": {}}, ok=False)


if __name__ == "__main__":
    unittest.main(verbosity=2)
