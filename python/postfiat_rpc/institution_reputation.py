"""Score a claimed institution with a pinned local Qwen/SGLang server.

The client is loopback-only by design. It cannot call OpenRouter or another
external inference provider. An institution the model does not recognize
receives zero; the response is returned without a secondary formula.
"""

from __future__ import annotations

import argparse
import json
import sys
import urllib.error
import urllib.parse
import urllib.request
from typing import Any, Sequence

DEFAULT_ENDPOINT = "http://127.0.0.1:8000/v1/chat/completions"
MODEL = "Qwen/Qwen3.8-27B-FP8"
PROMPT_TEMPLATE = (
    "Score how legitimate ENTITY <{entity}> is. Write 2-3 paragraphs justifying "
    "your explanation and give a score from 0-100. If you do not recognize the "
    "institution, the score is 0."
)
LOOPBACK_HOSTS = frozenset({"127.0.0.1", "localhost", "::1"})


class InstitutionReputationError(RuntimeError):
    """Raised when an institution cannot be scored."""


def build_prompt(entity: str) -> str:
    entity = entity.strip()
    if not entity:
        raise ValueError("entity must not be empty")
    return PROMPT_TEMPLATE.format(entity=entity)


def validate_endpoint(endpoint: str) -> str:
    parsed = urllib.parse.urlsplit(endpoint)
    if parsed.scheme != "http" or parsed.hostname not in LOOPBACK_HOSTS:
        raise ValueError("inference endpoint must be an HTTP loopback address")
    if parsed.path != "/v1/chat/completions":
        raise ValueError("inference endpoint must end with /v1/chat/completions")
    return endpoint


def build_request(entity: str, endpoint: str = DEFAULT_ENDPOINT) -> urllib.request.Request:
    payload = {
        "model": MODEL,
        "messages": [{"role": "user", "content": build_prompt(entity)}],
        "temperature": 0,
        "chat_template_kwargs": {"enable_thinking": False},
    }
    return urllib.request.Request(
        validate_endpoint(endpoint),
        data=json.dumps(payload).encode("utf-8"),
        headers={"Content-Type": "application/json"},
        method="POST",
    )


def _response_content(document: Any) -> str:
    try:
        content = document["choices"][0]["message"]["content"]
    except (KeyError, IndexError, TypeError) as exc:
        raise InstitutionReputationError("local inference returned no model response") from exc
    if not isinstance(content, str) or not content.strip():
        raise InstitutionReputationError("local inference returned an empty model response")
    return content.strip()


def score_institution(
    entity: str,
    endpoint: str = DEFAULT_ENDPOINT,
    timeout: float = 1800.0,
) -> str:
    request = build_request(entity, endpoint)
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            return _response_content(json.load(response))
    except urllib.error.HTTPError as exc:
        raise InstitutionReputationError(
            f"local inference failed with HTTP {exc.code}"
        ) from exc
    except (urllib.error.URLError, TimeoutError) as exc:
        raise InstitutionReputationError("local inference request failed") from exc
    except json.JSONDecodeError as exc:
        raise InstitutionReputationError("local inference returned invalid JSON") from exc


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("entity", nargs="+", help="institution name to score")
    parser.add_argument("--endpoint", default=DEFAULT_ENDPOINT)
    parser.add_argument("--timeout", type=float, default=1800.0)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        for index, entity in enumerate(args.entity):
            if index:
                print()
            print(f"=== {entity} ===")
            print(score_institution(entity, args.endpoint, args.timeout))
    except (InstitutionReputationError, ValueError) as exc:
        print(str(exc), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
