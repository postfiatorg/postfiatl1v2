'use strict';

// The closed set of transport classes a caller may see for an upstream RPC
// failure. Classes are set where the error is thrown; system errors are mapped,
// never passed through, and no upstream address or message text is exposed.
const RPC_TRANSPORT_ERROR_CLASSES = Object.freeze([
    'rpc_timeout',
    'rpc_connection_closed',
    'rpc_connection_refused',
    'rpc_connection_reset',
    'rpc_unreachable',
    'rpc_invalid_json',
    'rpc_response_id_mismatch',
    'rpc_transport_error',
]);

const CLASS_SET = new Set(RPC_TRANSPORT_ERROR_CLASSES);

const SYSTEM_CODE_CLASSES = Object.freeze({
    ECONNREFUSED: 'rpc_connection_refused',
    ECONNRESET: 'rpc_connection_reset',
    ECONNABORTED: 'rpc_connection_reset',
    EPIPE: 'rpc_connection_reset',
    ETIMEDOUT: 'rpc_timeout',
    EHOSTUNREACH: 'rpc_unreachable',
    ENETUNREACH: 'rpc_unreachable',
    EHOSTDOWN: 'rpc_unreachable',
    ENOTFOUND: 'rpc_unreachable',
    EAI_AGAIN: 'rpc_unreachable',
    ERR_STREAM_DESTROYED: 'rpc_connection_closed',
    ERR_STREAM_WRITE_AFTER_END: 'rpc_connection_closed',
});

function rpcTransportError(transportClass, message) {
    if (!CLASS_SET.has(transportClass)) throw new Error(`unknown RPC transport class ${transportClass}`);
    const error = new Error(message);
    error.transportClass = transportClass;
    return error;
}

function rpcTransportErrorClass(error) {
    if (CLASS_SET.has(error?.transportClass)) return error.transportClass;
    return SYSTEM_CODE_CLASSES[error?.code] || 'rpc_transport_error';
}

module.exports = { RPC_TRANSPORT_ERROR_CLASSES, rpcTransportError, rpcTransportErrorClass };
