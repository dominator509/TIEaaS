import json
import os
import pathlib
import subprocess
import tempfile
import time
import unittest
import urllib.request


class TIEServerProcess:
    def __init__(self):
        self.base_url = os.environ.get("TIE_BASE_URL", "http://127.0.0.1:18080")
        self.binary = os.environ.get("TIE_BIN", str(pathlib.Path("target/debug/tie")))
        self.proc = None
        self.tempdir = None

    def start(self):
        if self._ready():
            return

        if not pathlib.Path(self.binary).exists():
            raise unittest.SkipTest(
                f"TIE binary not found at {self.binary}. Build it with `cargo build` or set TIE_BASE_URL to a running service."
            )

        host = self.base_url.rsplit(":", 1)[0].replace("http://", "")
        port = self.base_url.rsplit(":", 1)[1]
        bind = f"{host}:{port}"

        self.tempdir = tempfile.TemporaryDirectory(prefix="tie-e2e-")
        db_path = pathlib.Path(self.tempdir.name) / "tie.db"; open(db_path, "w").close()

        env = os.environ.copy()
        env.setdefault("RUST_LOG", "warn")
        env["TIE_DATABASE_URL"] = f"sqlite://{db_path}"
        env["TIE_HTTP_BIND"] = bind
        env["TIE_POLICY_MODE"] = env.get("TIE_POLICY_MODE", "critical-fail-closed")
        env["TIE_REQUIRE_FACT_CITATIONS"] = env.get("TIE_REQUIRE_FACT_CITATIONS", "true")
        env["TIE_REQUIRE_ACTION_APPROVAL"] = env.get("TIE_REQUIRE_ACTION_APPROVAL", "true")

        self.proc = subprocess.Popen(
            [self.binary, "serve"],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            env=env,
        )

        deadline = time.time() + 15.0
        while time.time() < deadline:
            if self.proc.poll() is not None:
                stdout, stderr = self.proc.communicate(timeout=1)
                raise RuntimeError(
                    "TIE server exited before becoming ready.\n"
                    f"stdout:\n{stdout}\n\n"
                    f"stderr:\n{stderr}"
                )
            if self._ready():
                return
            time.sleep(0.25)

        raise TimeoutError("Timed out waiting for TIE /readyz")

    def stop(self):
        if self.proc is not None and self.proc.poll() is None:
            self.proc.terminate()
            try:
                self.proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.proc.kill()
        if self.tempdir is not None:
            self.tempdir.cleanup()

    def _ready(self):
        try:
            data = request_json(self.base_url + "/readyz")
            return data.get("status") == "ready"
        except Exception:
            return False


def request_json(url, method="GET", payload=None):
    body = None
    headers = {"Accept": "application/json"}
    if payload is not None:
        body = json.dumps(payload).encode("utf-8")
        headers["Content-Type"] = "application/json"

    req = urllib.request.Request(url, data=body, headers=headers, method=method)
    with urllib.request.urlopen(req, timeout=10) as resp:
        raw = resp.read().decode("utf-8")
        return json.loads(raw) if raw else {}


class TIEIntegrationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.server = TIEServerProcess()
        cls.server.start()
        cls.base = cls.server.base_url.rstrip("/")

    @classmethod
    def tearDownClass(cls):
        cls.server.stop()

    def test_health_and_readiness(self):
        health = request_json(self.base + "/healthz")
        ready = request_json(self.base + "/readyz")

        self.assertEqual(health["status"], "ok")
        self.assertEqual(ready["status"], "ready")
        self.assertIn("policy_mode", ready)

    def test_registry_crud_flow(self):
        created = request_json(
            self.base + "/v1/registry/records",
            method="POST",
            payload={
                "namespace": "specs",
                "kind": "fact",
                "key": "gravity",
                "value": {"claim": "Gravity is attractive."},
                "provenance": {"source": "physics-textbook"},
                "tags": ["science", "v1"],
            },
        )
        self.assertEqual(created["version"], 1)
        self.assertEqual(created["key"], "gravity")

        fetched = request_json(self.base + f"/v1/registry/records/{created['id']}")
        self.assertEqual(fetched["id"], created["id"])

        superseded = request_json(
            self.base + f"/v1/registry/records/{created['id']}",
            method="PUT",
            payload={
                "value": {"claim": "Gravity acts between masses."},
                "provenance": {"source": "physics-textbook", "reviewed": True},
                "tags": ["science", "v2"],
            },
        )
        self.assertEqual(superseded["version"], 2)

        by_key = request_json(self.base + "/v1/registry/records/by-key/specs/fact/gravity")
        self.assertEqual(by_key["id"], superseded["id"])
        self.assertEqual(by_key["version"], 2)

        records = request_json(self.base + "/v1/registry/records")
        self.assertGreaterEqual(len(records), 1)

        deleted = request_json(self.base + f"/v1/registry/records/{superseded['id']}", method="DELETE")
        self.assertEqual(deleted["status"], "retired")

    def test_validation_end_to_end_for_code_fact_and_action(self):
        code = request_json(
            self.base + "/v1/validate",
            method="POST",
            payload={
                "request_id": "e2e-code-1",
                "subject_type": "code",
                "subject": "unsafe fn shell() { std::process::Command::new(\"rm\"); }",
                "metadata": {},
                "registry_record_ids": [],
            },
        )
        self.assertEqual(code["verdict"], "fail")
        self.assertEqual(code["enforcement_action"], "block")
        self.assertTrue(any(item["adapter"] == "code_verifier" for item in code["evidence"]))

        fact = request_json(
            self.base + "/v1/validate",
            method="POST",
            payload={
                "request_id": "e2e-fact-1",
                "subject_type": "fact",
                "subject": "Water boils at 100C at sea level.",
                "metadata": {"citations": ["https://example.org/boiling-point"]},
                "registry_record_ids": [],
            },
        )
        self.assertEqual(fact["verdict"], "pass")
        self.assertEqual(fact["enforcement_action"], "allow")

        action = request_json(
            self.base + "/v1/validate",
            method="POST",
            payload={
                "request_id": "e2e-action-1",
                "subject_type": "action",
                "subject": "Restart the staging worker service.",
                "metadata": {},
                "registry_record_ids": [],
            },
        )
        self.assertEqual(action["verdict"], "warn")
        self.assertEqual(action["enforcement_action"], "allow_with_warning")


if __name__ == "__main__":
    unittest.main()
