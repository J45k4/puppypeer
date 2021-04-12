fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .out_dir("src")
        .build_client(true)
        .compile(&["proto/epic-shelter-service.proto"], &["proto"]).unwrap();

    Ok(())
}
