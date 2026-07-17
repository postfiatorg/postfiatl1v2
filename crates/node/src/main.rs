#![allow(
    clippy::manual_ignore_case_cmp,
    clippy::manual_inspect,
    clippy::manual_is_multiple_of,
    clippy::needless_borrow,
    clippy::needless_borrows_for_generic_args,
    clippy::ptr_arg,
    clippy::too_many_arguments,
    clippy::unnecessary_map_or
)]

include!("main_parts/cli_dispatch.rs");
include!("transport_cli.rs");
include!("main_parts/nav_roundtrip_runner.rs");
include!("rpc_cli.rs");
include!("finality_view_recovery.rs");
include!("atomic_swap_rpc_server.rs");
mod fastswap_service;
mod rpc_dispatch;
use rpc_dispatch::run_rpc;
include!("main_parts/runtime_helpers.rs");
include!("main_parts/tests.rs");
