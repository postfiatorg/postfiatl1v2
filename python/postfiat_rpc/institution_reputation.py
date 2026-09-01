"""Score whether a claimed institution is recognizable and legitimate.

This intentionally asks Qwen one simple question. An institution the model does
not recognize receives zero. The response is returned verbatim; no formula or
secondary scoring pipeline rewrites it.
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import urllib.error
import urllib.request
from typing import Any, Sequence

OPENROUTER_URL = "https://openrouter.ai/api/v1/chat/completions"
MODEL = "qwen/qwen3.8-27b"
PROMPT_TEMPLATE = (
    "Score how legitimate ENTITY <{entity}> is. Write 2-3 paragraphs justifying "
    "your explanation and give a score from 0-100. If you do not recognize the "
    "institution, the score is 0."
)


class InstitutionReputationError(RuntimeError):
    """Raised when an institution cannot be scored."""


def build_prompt(entity: str) -> str:
    entity = entity.strip()
    if not entity:
        raise ValueError("entity must not be empty")
    return PROMPT_TEMPLATE.format(entity=entity)


def build_request(entity: str, api_key: str) -> urllib.request.Request:
    if not api_key:
        raise ValueError("OpenRouter API key must not be empty")
    payload = {
        "model": MODEL,
        "messages": [{"role": "user", "content": build_prompt(entity)}],
        "temperature": 0,
    }
    return urllib.request.Request(
        OPENROUTER_URL,
        data=json.dumps(payload).encode("utf-8"),
        headers={
            "Authorization": f"Bearer {api_key}",
            "Content-Type": "application/json",
        },
        method="POST",
    )


def _response_content(document: Any) -> str:
    try:
        content = document["choices"][0]["message"]["content"]
    except (KeyError, IndexError, TypeError) as exc:
        message = "OpenRouter returned no model response"
        if isinstance(document, dict):
            error = document.get("error")
            if isinstance(error, dict) and isinstance(error.get("message"), str):
                message = error["message"]
        raise InstitutionReputationError(message) from exc
    if not isinstance(content, str) or not content.strip():
        raise InstitutionReputationError("OpenRouter returned an empty model response")
    return content.strip()


def score_institution(entity: str, api_key: str, timeout: float = 60.0) -> str:
    request = build_request(entity, api_key)
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            return _response_content(json.load(response))
    except urllib.error.HTTPError as exc:
        raise InstitutionReputationError(
            f"OpenRouter request failed with HTTP {exc.code}"
        ) from exc
    except (urllib.error.URLError, TimeoutError) as exc:
        raise InstitutionReputationError("OpenRouter request failed") from exc
    except json.JSONDecodeError as exc:
        raise InstitutionReputationError("OpenRouter returned invalid JSON") from exc


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("entity", nargs="+", help="institution name to score")
    parser.add_argument(
        "--api-key-env",
        default="OPENROUTER_API_KEY",
        help="environment variable containing the OpenRouter API key",
    )
    parser.add_argument("--timeout", type=float, default=60.0)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    api_key = os.environ.get(args.api_key_env, "")
    if not api_key:
        print(f"{args.api_key_env} is not set", file=sys.stderr)
        return 2

    try:
        for index, entity in enumerate(args.entity):
            if index:
                print()
            print(f"=== {entity} ===")
            print(score_institution(entity, api_key, args.timeout))
    except (InstitutionReputationError, ValueError) as exc:
        print(str(exc), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
