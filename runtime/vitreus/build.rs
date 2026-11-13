#[cfg(feature = "std")]
fn main() {
    let mainnet = std::env::var("CARGO_FEATURE_MAINNET_RUNTIME").is_ok();
    let testnet = std::env::var("CARGO_FEATURE_TESTNET_RUNTIME").is_ok();

    let file_name = match (mainnet, testnet) {
        (true, false) => "vitreus_power_plant_mainnet_runtime",
        (false, true) => "vitreus_power_plant_testnet_runtime",
        (false, false) => panic!("Either the mainnet or testnet runtime must be enabled."),
        (true, true) => {
            panic!("The mainnet and testnet runtimes cannot be enabled simultaneously.")
        },
    };

    // srtool finds the blob only by the package name, and `set_file_name` renames the blob
    // as well, so under srtool (it always builds into `target/srtool`) don't call it at all.
    // The generated file then keeps wasm-builder's default name, `wasm_binary.rs`.
    let builder = substrate_wasm_builder::WasmBuilder::init_with_defaults();
    let file_name = if std::env::var("OUT_DIR").unwrap().contains("/target/srtool/") {
        builder.build();
        "wasm_binary.rs"
    } else {
        builder.set_file_name(file_name).build();
        file_name
    };

    println!("cargo:rustc-env=VITREUS_WASM_BINARY_FILE={file_name}");
}

#[cfg(not(feature = "std"))]
fn main() {}
