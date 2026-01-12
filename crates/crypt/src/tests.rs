use anyhow::Result;
use base64::{Engine as _, engine::general_purpose};

use crate::{Ticket, SECRET_KEY_SIZE};

const PRIVATE_KEY_768_TESTING: &str = "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";

const TICKET_ID_TESTING: &str = "c6s9FAa5fhb854BVMckqUBJ4hOXg2iE5i1FYPCuktks4eNZD";

const TEST_TICKET_JSON: &str = r#"{
  "algorithm": "AES-256-GCM",
  "ciphertext": "5QORerhVDPvU+/UdYbLIpeuc13hzbl8IUIqsZbXvR/8wYTcJauRjQOjUxDZg+RAeKXlsjb2FTf9265I2cweQpPcYPURitnAoPtm/cQWf7cJslE5lX220WInULLnElP4NS7MC9G6qgElc0JBSIlDK9y4AacI2T5k+VtTEMVB5j3r1EMbL1RBtDW43+MBFo6i8hDZMY1qM2CG57H+ueIVAloCzz6zSGHoWJcQDXJYJQdYOpNwI6YbMpRnKIkfIxqFfMT0TjhNg4XJxMbNfgMgpTIjjb3/JDOaaILJIyx0mRh5wWOkszu1n4OF5ZxUSLNKbj3L6HK3IoO8CAem1cFZyoPM2KmG91NYfp2if28amHc7aTXN81Z/Wxm1fBOTT43ufLB8K7LuD5og4vpA7qZB5uDSq7xGh3EfSfG861diOUdXKEklnTHxTF/ne6tOVDQUdRoyD2L0hBDvFX62k+ASBJXjdtuHHEMCGT2YXCSuEl2FNYZwOfDbA17H26AJsWEDV01zoqFOBpBgS3qdHyMLpkonPtnC47fU/CRwCYjsvW7XD/RvgQejbkqn3yxHSUiu+jY+yHDh21cJyvHhUteF0BIxJngpWrffsZVlfePdYMe1Ws/din36hkVYVhwJeqYQXVwzdNKyPpesZlHrXQHJ0yuPiK6dRuSw12wQCR0yYHClehrbebCLcc2As+uF6gUEJJIBOhDiiOVuEaXrFmKsH0dRYeKVwFDRJZWBx6IqqmeVXaw/dTwt0pP1K9mzosxNbhgrQS/jq5Ml25/rJOIU7PlEdqu8rbZk1w91/CY68u7je8NpZs2490mBBIbD+ipJ9Qk+vxt9RJFmTVuChNmLqv8TdNXqSzn2VDNVZ04cFU2BUwIScOZXHIEpxrdeKC2Gdyvgnq7DFN1oPaeewIOtAmckHjoGJMxCFFvfk9tA03oEnarBaVgrrolOK5MahwFzZ1q9y528aJEpxTCofygKKjePTlNuMnEUJ7y/aLrnQJPURetS1HfqWAh5sSiPYSlPU5TxSZA+niQJAZxYUruepv9fhYNWO6KVwKgHhLhN3ogRiNhi0FtIXvqePHDSNYg1skXhrrQj2V3p/YouCnbUsPcGKFoX3lMrt2mjINHdyslx3H2LkieJSUa0UiKUzvDC+4wTAR1jPIQZ48rbKzEApi82cmzEtjXa5I79pje6vYxdRTYLcepDNjk5+EpuMO2wEYLzM7ZtW5Q3G1kKAk2xqbcw7b3z6oX33/l4eMYEhC+P+Tx/imXfWNLk+6WFJjK5pIO9VqoKE+dZNy7Q11o1txjXQcJV2FqZutG6GUlkoHoXyR1AvvJFLG4LG6L7/cmDN2CnovPzq1eyCkzL/+OeJvQYsI7ExbtWbnVMQC2kL6pV/Dd0Mi53o1WETpC1UBcvKspCdHwS+dqG4bT1rCyb1xwXY0u+u1JnG28me+MIA+H4=",
  "data": "pNZ00G0QY71OaYi4qlxw/aRL2QssUF8Ubzr3bf6mSMC3qSxeOeior7BfhTLk86EQizQ5CkfBRDmdNQPH2dpkk/2c+b7cGLWzZU3tYq0nh0ACl6YWGMdNqwzgRBHgXDWDubG7HV0CT96Kd/LP57qhJUECJUy/z+vtdQBMwDyuz3Q70jyOkKQYxXB9wDZn4Gq7cNzuRgBNw1ZQTbB4qlgFAY3ceYLo90aSP+tnKtBitRZ3Ou6/Yfyy/qFq1mQ5woamdCMCSt7RX+S+beQufaCvKXFSF5ij8ZodoqR8WPgPFUYarXwpcHS/sbtu4tmzwmS5UZumVD9X4kv3oESTsOv440U4Z+FJlqTf1LiKjRTv94njNkqX/wd/e4Inf7op3rGpkFUFdNlqQk9l4/L2nEsFSO5nTgo7OH4V2jE9zn/KKqISa6I+CxDU3TTyXRV9nUmf1FvlV7AUMi3tXiM99+i4EksAly8O7yV66r9KJBerBAJjaNt1aTpwph+2CdrDhhm6gvflEfcdpxYIiYM6Uuk3MNt/gr5m8QuDVGoBpAPPVA=="
}"#;

// Original JSON used to generate the above ticket
// {
//   "script": "QlpoOTFBWSZTWXJ1UTUAAAIRgEAAqjZcACAAIiBtIe1CAaAMudG48I6NihWLuSKcKEg5OqiagA==",
//   "type": "javascript",
//   "signature_algorithm": "unused-sig-algo",
//   "signature": "dW51c2VkIHNpZ25hdHVyZSBmb3IgdGVzdA==",
//   "params": "QlpoOTFBWSZTWTiAk9MAADGbgFAFfxAECiIHXgogAFQ0mhGCNGDU9TEPVIGj9U0HpACsdgBh1hNJT9HbykVhMsTbjKspMADQ7y0QggJddmj2AcwBGCt6r+F/hsXckU4UJA4gJPTA",
//   "log": {
//     "level": "info",
//     "ticket": null
//   }
// }

fn get_private_key_bytes() -> Result<[u8; SECRET_KEY_SIZE]> {
    let kem_private_key_bytes = general_purpose::STANDARD
        .decode(PRIVATE_KEY_768_TESTING)
        .map_err(|e| anyhow::format_err!("Failed to decode base64 KEM private key: {}", e))?;
    let kem_private_key_bytes: [u8; SECRET_KEY_SIZE] = kem_private_key_bytes
        .try_into()
        .map_err(|_| anyhow::format_err!("Invalid KEM private key size"))?;
    Ok(kem_private_key_bytes)
}

#[test]
fn test_recover_invalid_data_from_json() {
    let ticket = Ticket::new("AES-256-GCM", "", "");

    let result =
        ticket.recover_data_from_json(TICKET_ID_TESTING.as_bytes(), &get_private_key_bytes().unwrap());
    assert!(result.is_err());
}

#[test]
fn test_recover_valid_data_from_json() {
    let ticket: Ticket = serde_json::from_str(TEST_TICKET_JSON).unwrap();

    let result =
        ticket.recover_data_from_json(TICKET_ID_TESTING.as_bytes(), &get_private_key_bytes().unwrap());
    assert!(
        result.is_ok(),
        "Failed to recover data from JSON ticket: {:?}",
        result.err()
    );

    // Test material fields are present
    let json_value = result.unwrap();
    println!("Recovered JSON value: {}", json_value);
    // Get object "crypto_params"
    let crypto_params = json_value.get("crypto_params").expect("Missing crypto_params field");
    assert!(crypto_params.is_object(), "crypto_params is not an object");
    assert!(crypto_params.get("key_send").is_some());
    assert!(crypto_params.get("key_receive").is_some());

    assert_eq!(
        json_value.get("script").unwrap(),
        "QlpoOTFBWSZTWXJ1UTUAAAIRgEAAqjZcACAAIiBtIe1CAaAMudG48I6NihWLuSKcKEg5OqiagA=="
    );
    assert_eq!(json_value.get("type").unwrap(), "javascript");
    assert_eq!(
        json_value.get("signature_algorithm").unwrap(),
        "unused-sig-algo"
    );
    assert_eq!(
        json_value.get("signature").unwrap(),
        "dW51c2VkIHNpZ25hdHVyZSBmb3IgdGVzdA=="
    );
    assert_eq!(
        json_value.get("params").unwrap(),
        "QlpoOTFBWSZTWTiAk9MAADGbgFAFfxAECiIHXgogAFQ0mhGCNGDU9TEPVIGj9U0HpACsdgBh1hNJT9HbykVhMsTbjKspMADQ7y0QggJddmj2AcwBGCt6r+F/hsXckU4UJA4gJPTA"
    );
    let log = json_value.get("log").unwrap();
    assert_eq!(log.get("level").unwrap(), "info");
    assert!(log.get("ticket").unwrap().is_null());
}
