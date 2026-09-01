from __future__ import annotations

import io
import json
import unittest
import urllib.error
from unittest.mock import patch

from postfiat_rpc.institution_reputation import (
    DEFAULT_ENDPOINT,
    MODEL,
    InstitutionReputationError,
    build_prompt,
    build_request,
    score_institution,
    validate_endpoint,
)


class FakeResponse(io.BytesIO):
    def __enter__(self) -> "FakeResponse":
        return self

    def __exit__(self, *_args: object) -> None:
        self.close()


class InstitutionReputationTests(unittest.TestCase):
    def test_prompt_states_unrecognized_institution_scores_zero(self) -> None:
        self.assertEqual(
            build_prompt("University of Zuzaluca"),
            "Score how legitimate ENTITY <University of Zuzaluca> is. Write 2-3 "
            "paragraphs justifying your explanation and give a score from 0-100. "
            "If you do not recognize the institution, the score is 0.",
        )

    def test_request_uses_exact_local_model(self) -> None:
        request = build_request("University of Waterloo")
        body = json.loads(request.data)
        self.assertEqual(request.full_url, DEFAULT_ENDPOINT)
        self.assertEqual(body["model"], MODEL)
        self.assertEqual(body["temperature"], 0)
        self.assertEqual(body["chat_template_kwargs"], {"enable_thinking": False})
        self.assertIsNone(request.get_header("Authorization"))

    def test_external_inference_endpoint_is_rejected(self) -> None:
        with self.assertRaisesRegex(ValueError, "loopback"):
            validate_endpoint("https://openrouter.ai/api/v1/chat/completions")

    def test_score_returns_local_model_text_verbatim(self) -> None:
        expected = "Recognized institution. Score: 98/100"
        response = FakeResponse(
            json.dumps({"choices": [{"message": {"content": expected}}]}).encode()
        )
        with patch(
            "postfiat_rpc.institution_reputation.urllib.request.urlopen",
            return_value=response,
        ):
            self.assertEqual(score_institution("University of Waterloo"), expected)

    def test_score_fails_without_model_response(self) -> None:
        response = FakeResponse(json.dumps({"choices": []}).encode())
        with patch(
            "postfiat_rpc.institution_reputation.urllib.request.urlopen",
            return_value=response,
        ):
            with self.assertRaisesRegex(InstitutionReputationError, "no model response"):
                score_institution("University of Zuzaluca")

    def test_score_reports_http_status(self) -> None:
        error = urllib.error.HTTPError(
            DEFAULT_ENDPOINT, 429, "rate limited", hdrs=None, fp=None
        )
        with patch(
            "postfiat_rpc.institution_reputation.urllib.request.urlopen",
            side_effect=error,
        ):
            with self.assertRaisesRegex(InstitutionReputationError, "HTTP 429"):
                score_institution("University of Waterloo")


if __name__ == "__main__":
    unittest.main()
