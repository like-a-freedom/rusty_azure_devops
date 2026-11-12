// `include_dir!` embeds the skill tree at compile time, but its
// `tracked_path` call is a no-op on stable Rust (it is nightly-only), so
// cargo has no way to know that a file was added to or removed from
// `skills/`. Without this, adding a skill file leaves the binary stale —
// the skill silently does not ship.
//
// `rerun-if-changed` on the directory makes any change under it rebuild the
// crate, so the embedded tree always matches what is on disk.
fn main() {
    println!("cargo:rerun-if-changed=skills");
}
