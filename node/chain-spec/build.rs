use std::{
    env,
    error::Error,
    fs::File,
    io::Write,
    path::{Path, PathBuf},
    str::FromStr,
};

fn main() {
    process_claiming(input_path("claims.csv"), output_file("claiming_claims.rs")).unwrap();
}

fn process_claiming(input: PathBuf, mut claims: File) -> Result<(), Box<dyn Error>> {
    let mut reader =
        csv::ReaderBuilder::new().has_headers(false).delimiter(b';').from_path(input)?;

    write!(&mut claims, "vec![")?;
    for result in reader.records() {
        let record = result?;

        let address = strip_hex_prefix(&record[0]);
        let amount = u128::from_str(record[1].trim())?;

        write!(
            &mut claims,
            "(pallet_claiming::EthereumAddress(hex_literal::hex!(\"{address}\")), {amount}),"
        )?;
    }
    write!(&mut claims, "]")?;

    Ok(())
}

fn input_path(file_name: &str) -> PathBuf {
    Path::new("genesis").join(file_name)
}

fn output_file(file_name: &str) -> File {
    let out_dir = env::var("OUT_DIR").unwrap();
    let path = Path::new(&out_dir).join(file_name);

    File::create(path).unwrap()
}

fn strip_hex_prefix(s: &str) -> &str {
    s.strip_prefix("0x").unwrap_or(s)
}
