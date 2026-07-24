"""Durable coordinator primitives for the synthetic Lightning/PFTL demo.

The LND boundary accepts generated direct-gRPC stubs; no hosted API or REST
client is embedded. PFTL adapters consume the typed service/journal interface.
"""

from .journal import (
    CoordinatorJournal,
    ExposureLimitExceeded,
    ExposureLimits,
    IdempotencyConflict,
    InvalidTransition,
    SideEffectSpec,
    SwapState,
)
from .lnd_grpc import (
    CreatedInvoice,
    InvoiceStatus,
    LightningPaymentError,
    LndGrpcAdapter,
    LndGrpcError,
    LndRequestFactories,
    SettledPayment,
)
from .protocol import (
    AmpInvoiceRejected,
    InvoiceBindingError,
    LndInvoiceFacts,
    ProtocolEncodingError,
    SecretPreimage,
    decode_condition,
    decode_fulfillment,
    encode_condition,
    encode_fulfillment,
    payment_hash,
    validate_invoice_binding,
    verify_fulfillment,
)
from .quote import (
    QuoteValidationError,
    canonical_quote_bytes,
    parse_canonical_quote,
    validate_quote,
)
from .signing import (
    Ed25519Signer,
    QuoteSignatureError,
    encode_signed_quote,
    parse_signed_quote,
    sign_quote,
    verify_signed_quote,
)
from .service import CoordinatorService, RecoveryAction

__all__ = [
    "AmpInvoiceRejected",
    "CoordinatorJournal",
    "CoordinatorService",
    "CreatedInvoice",
    "Ed25519Signer",
    "ExposureLimitExceeded",
    "ExposureLimits",
    "IdempotencyConflict",
    "InvalidTransition",
    "InvoiceBindingError",
    "InvoiceStatus",
    "LightningPaymentError",
    "LndInvoiceFacts",
    "LndGrpcAdapter",
    "LndGrpcError",
    "LndRequestFactories",
    "ProtocolEncodingError",
    "QuoteSignatureError",
    "QuoteValidationError",
    "RecoveryAction",
    "SecretPreimage",
    "SettledPayment",
    "SideEffectSpec",
    "SwapState",
    "canonical_quote_bytes",
    "decode_condition",
    "decode_fulfillment",
    "encode_condition",
    "encode_fulfillment",
    "encode_signed_quote",
    "parse_canonical_quote",
    "parse_signed_quote",
    "payment_hash",
    "sign_quote",
    "validate_invoice_binding",
    "validate_quote",
    "verify_fulfillment",
    "verify_signed_quote",
]
