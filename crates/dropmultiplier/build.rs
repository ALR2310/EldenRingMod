// Embeds the Windows version resource (File version, Product version,
// Description - shown in the DLL's Properties and read by common::diag) from
// this crate's Cargo.toml.
fn main() {
    winresource::WindowsResource::new()
        .compile()
        .expect("embedding the version resource failed (needs rc.exe from the Windows SDK)");
}
