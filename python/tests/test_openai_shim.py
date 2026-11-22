# Copyright 2026 ZyvorAI Labs Private Limited
# SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

import unittest
import unittest.mock

try:
    from fastapi.testclient import TestClient

    from zyvor_janus.server.app import app

    HAS_FASTAPI = True
except ImportError:
    HAS_FASTAPI = False


@unittest.skipUnless(HAS_FASTAPI, "fastapi not installed")
class OpenAIShimTests(unittest.TestCase):
    def setUp(self) -> None:
        self.client = TestClient(app)

    def test_rejects_missing_auth(self) -> None:
        res = self.client.post("/v1/chat/completions", json={"model": "llama-70b", "messages": [{"role": "user", "content": "hi"}]})
        self.assertEqual(res.status_code, 401)

    @unittest.mock.patch("zyvor_janus.server.openai_shim._check_rate_limit")
    def test_completes_with_auth(self, _rate) -> None:
        headers = {"Authorization": "Bearer dev-zyvor-janus-key"}
        res = self.client.post(
            "/v1/chat/completions",
            headers=headers,
            json={"model": "llama-70b", "messages": [{"role": "user", "content": "hello world"}]},
        )
        self.assertEqual(res.status_code, 200)
        body = res.json()
        self.assertIn("choices", body)


if __name__ == "__main__":
    unittest.main()
