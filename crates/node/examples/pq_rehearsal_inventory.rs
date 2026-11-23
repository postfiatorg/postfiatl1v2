//! Print public supply and PQ roots from an offline rehearsal replica.
#[path = "../src/main_parts/tests/pq_rehearsal_inventory.rs"]
mod report;

fn main() {
    let dir = std::env::args_os()
        .nth(1)
        .expect("usage: pq_rehearsal_inventory DATA_DIR");
    report::inventory(std::path::Path::new(&dir));
}
