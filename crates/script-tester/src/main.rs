use std::io::Write;

use anyhow::Result;
use base64::engine::{Engine as _, general_purpose::STANDARD};
use bzip2::write::BzEncoder;

use shared::{broker::api::types, log};

// Get script and json with params from args
// The signature file is the script file + mldsa65.sig
async fn get_script_and_params() -> Result<types::Script> {
    let args: Vec<String> = std::env::args().collect();
    let args = if args.len() < 3 {
        ["not_used","crates/script-tester/testdata/script.js", "crates/script-tester/testdata/data.json"]
            .iter()
            .map(|s| s.to_string())
            .collect()
    } else {
        args
    };
    let script_path = &args[1];
    let params_json_path = &args[2];

    // Script is bz2 + base64 encoded
    let script_bytes = std::fs::read(script_path)
        .map_err(|e| anyhow::anyhow!("Error reading script file: {}", e))?;
    // Compress to bz2 first
    let mut encoder = BzEncoder::new(Vec::new(), bzip2::Compression::best());
    encoder.write_all(&script_bytes)?;
    let script_bytes = encoder.finish()?;
    let script = STANDARD.encode(&script_bytes);
    let script_signature_path = format!("{}.mldsa65.sig", script_path);
    // Read binary and base64 encode
    let signature = std::fs::read_to_string(&script_signature_path)
        .map_err(|e| anyhow::anyhow!("Error reading script signature file: {}", e))?;
    let params_bytes = std::fs::read(params_json_path)
        .map_err(|e| anyhow::anyhow!("Error reading params json file: {}", e))?;
    let mut encoder = BzEncoder::new(Vec::new(), bzip2::Compression::best());
    encoder.write_all(&params_bytes)?;
    let params_bytes = encoder.finish()?;
    let params = STANDARD.encode(&params_bytes);

    Ok(types::Script {
        script,
        script_type: "javascript".to_string(),
        signature,
        signature_algorithm: "MLDSA65".to_string(),
        params,
        log: types::Log {
            level: "info".to_string(),
            ticket: None,
        }
    })
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    log::setup_logging("debug", log::LogType::Tests);
    println!("Current working directory: {}", std::env::current_dir()?.display());
    let script = get_script_and_params().await?;
    // if let Err(e) = script.verify_signature() {
    //     println!("Script signature verification failed: {}", e);
    //     return Ok(());
    // }
    // Run the script
    script.execute().await?;
    
    Ok(())
}