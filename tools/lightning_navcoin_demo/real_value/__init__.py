"""Fail-closed mainnet graduation surfaces for the Lightning/NAVcoin demo.

The synthetic harness remains isolated in :mod:`tools.lightning_navcoin_demo`.
Nothing in this package enables value by default.  A pinned route, a fresh
signed quote, converged PFTL state, a healthy mainnet LND node, and a
single-use operator authorization are all required before an execution
adapter may initiate a value-moving side effect.
"""

from .policy import (
    ExecutionMode,
    MainnetQuoteView,
    PriceObservation,
    RealValuePolicy,
    RealValuePolicyError,
    validate_mainnet_quote,
)

__all__ = [
    "ExecutionMode",
    "MainnetQuoteView",
    "PriceObservation",
    "RealValuePolicy",
    "RealValuePolicyError",
    "validate_mainnet_quote",
]
