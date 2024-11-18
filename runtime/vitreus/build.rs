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

    substrate_wasm_builder::WasmBuilder::init_with_defaults()
        .set_file_name(file_name)
        .build()
}

#[cfg(not(feature = "std"))]
fn main() {}
