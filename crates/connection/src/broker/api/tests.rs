// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
//
// 1. Redistributions of source code must retain the above copyright notice,
//    this list of conditions and the following disclaimer.
//
// 2. Redistributions in binary form must reproduce the above copyright notice,
//    this list of conditions and the following disclaimer in the documentation
//    and/or other materials provided with the distribution.
//
// 3. Neither the name of the copyright holder nor the names of its contributors
//    may be used to endorse or promote products derived from this software
//    without specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
// AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
// FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
// DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
// SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
// CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
// OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
// OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use super::*;

use shared::log;

use mockito::Server;

const PRIVATE_KEY_768_TESTING: &str = "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";

const PUBLIC_KEY_768_TESTING: &str = "d7qwr9yBu2K0fpAAsYdTyHRCL+iLaxBf5/F019DFOFbC+HY8PgVOH+y2uicK/5td3lpfzjm302eDDxJ9\
8+O2Cpt9R7yQEWCOVWvH26Z2k2BgXSmsovxqvtu5xVKvLOkpKEZyHpGgiumpSxS+0TE5Dgh2eQBJhmSk\
rysKbFWB0QUCQXAaH7BbQMhftKfIJ0W7ckeNilyZDGh2dVlN7Ohl1Bq0L0wvgvNmprFne3uCYmddE2F1\
Xrh/rwbDZvzKf5WRT+DJStdIOVxSV8c4PHkLnls1/XllgaM/9uNEgMtyWKOBHDmYFqCWnsMMlgoB8IGD\
IJKuDCSJuVNQVBuLGiDDrVye16dJpOS9B0uBt3sntvgy/PKSooVuamq3A8k/LzGcFfw9fIOLccKfFhhI\
cbllN9y1C2kYBHo6Lxxb5yNPmUsIdFAsRkg5cWaHh/gA1uEAXDIzttF2WYJEZfej6Hd+dWAJfowOAEnA\
e0KPz2KZBxocJRhndjFp6gxpapesa1mmd2dhu7aqZptAbmCt5tUyXNsjguGCmbqQ7cMc5zKLQbu/U0K8\
i2SD1pxrMgxa4UkKsTVWeMUKKzufCllPdYiiLmAgIZUO3xxK0vsQ45Cq+9xhlWWG61olnlqyx9EoffQh\
3IAR1HO0twEXqCt+BNU5x2ue3cpgZhwWBLfDBpyRWpeW3eGkbHtLt9OYolQCOMROkLKMEyM1k/pfrtYO\
l6NDYbEKwJY3NdkKaFG5ROs5dbIcO1IZ8kyzMfhyTRYkNqLAW+ISRTl0+MdPaIU6MtVHA2h7fpsVw6Nx\
oaASIdEhNIaOy0NRIFZQILsvXSTM3pcHmYkk6QhgIcN/s/anc7KWNsS/TQsvuzXEwFS4GDsAv+rPG3jA\
vrh/MqMAgMAh2GWI9LBaYgFngrA3U2xNGho69jFG2QtcVgc93kyns1cQuGWSL2lixdDGcwUjTetjegNJ\
Uehiz2ZJ7qadQlNPvJOT1dpOcPWKH/moz6gNOrw2LrRHZAOLdiS6arcmCWm/Xhq8gznKn7k178w0lRat\
nGUspBOZDXMNedYjPvKLmGV01ZhUUcET6wWpsbjLLhdEpfMMICW4HZI9GamENvEEjZkHzjEXJGWSMFnI\
vcxLUaWJW/MOqMmjjaabctRzo7dCJ8dKAN2l03UMkkyjF2s1DKlVSkrK+6Mwnnt7smF1k3BWU0ctt5BG\
U8xjP2Vm4npBPTEhpwUJ6LsfZ4pLANddBDHELDAerLuPcWuVc/hnbVxAIUpYK3ASeWKi1gdfCymQWaI1\
5OTKSwtXGNUh7bObbmkzjRofhRgTLCmnXhgL9pmX1SUcfck1drunDjjAPPsl3oO299e17AV4CMMugYKX\
sJZwl6BekOYG1ygz1DgUB0mDPaFVDucPbokk8DMtLOrKu+Bu9rOa7+MdxwYJ+mMjLeae9+MURrdHwtYh\
vaSdDPA8O1h8sGhck2ewpLR3dGCHeyeJsEws1IiCnRa2cqxA+LxenDJV1WeGjWogd+MiNioBJ+kzpLpe\
AFohLlJG5zmHijRc/O4OYbzvH/NisQARHwX3ScApN7qAjG49j2fwJgcrKCM=";

const TICKET_RESPONSE_JSON: &str = r#"{
  "result": {
    "algorithm": "AES-256-GCM",
    "ciphertext": "5QORerhVDPvU+/UdYbLIpeuc13hzbl8IUIqsZbXvR/8wYTcJauRjQOjUxDZg+RAeKXlsjb2FTf9265I2cweQpPcYPURitnAoPtm/cQWf7cJslE5lX220WInULLnElP4NS7MC9G6qgElc0JBSIlDK9y4AacI2T5k+VtTEMVB5j3r1EMbL1RBtDW43+MBFo6i8hDZMY1qM2CG57H+ueIVAloCzz6zSGHoWJcQDXJYJQdYOpNwI6YbMpRnKIkfIxqFfMT0TjhNg4XJxMbNfgMgpTIjjb3/JDOaaILJIyx0mRh5wWOkszu1n4OF5ZxUSLNKbj3L6HK3IoO8CAem1cFZyoPM2KmG91NYfp2if28amHc7aTXN81Z/Wxm1fBOTT43ufLB8K7LuD5og4vpA7qZB5uDSq7xGh3EfSfG861diOUdXKEklnTHxTF/ne6tOVDQUdRoyD2L0hBDvFX62k+ASBJXjdtuHHEMCGT2YXCSuEl2FNYZwOfDbA17H26AJsWEDV01zoqFOBpBgS3qdHyMLpkonPtnC47fU/CRwCYjsvW7XD/RvgQejbkqn3yxHSUiu+jY+yHDh21cJyvHhUteF0BIxJngpWrffsZVlfePdYMe1Ws/din36hkVYVhwJeqYQXVwzdNKyPpesZlHrXQHJ0yuPiK6dRuSw12wQCR0yYHClehrbebCLcc2As+uF6gUEJJIBOhDiiOVuEaXrFmKsH0dRYeKVwFDRJZWBx6IqqmeVXaw/dTwt0pP1K9mzosxNbhgrQS/jq5Ml25/rJOIU7PlEdqu8rbZk1w91/CY68u7je8NpZs2490mBBIbD+ipJ9Qk+vxt9RJFmTVuChNmLqv8TdNXqSzn2VDNVZ04cFU2BUwIScOZXHIEpxrdeKC2Gdyvgnq7DFN1oPaeewIOtAmckHjoGJMxCFFvfk9tA03oEnarBaVgrrolOK5MahwFzZ1q9y528aJEpxTCofygKKjePTlNuMnEUJ7y/aLrnQJPURetS1HfqWAh5sSiPYSlPU5TxSZA+niQJAZxYUruepv9fhYNWO6KVwKgHhLhN3ogRiNhi0FtIXvqePHDSNYg1skXhrrQj2V3p/YouCnbUsPcGKFoX3lMrt2mjINHdyslx3H2LkieJSUa0UiKUzvDC+4wTAR1jPIQZ48rbKzEApi82cmzEtjXa5I79pje6vYxdRTYLcepDNjk5+EpuMO2wEYLzM7ZtW5Q3G1kKAk2xqbcw7b3z6oX33/l4eMYEhC+P+Tx/imXfWNLk+6WFJjK5pIO9VqoKE+dZNy7Q11o1txjXQcJV2FqZutG6GUlkoHoXyR1AvvJFLG4LG6L7/cmDN2CnovPzq1eyCkzL/+OeJvQYsI7ExbtWbnVMQC2kL6pV/Dd0Mi53o1WETpC1UBcvKspCdHwS+dqG4bT1rCyb1xwXY0u+u1JnG28me+MIA+H4=",
    "data": "pNZ00G0QY71OaYi4qlxw/aRL2QssUF8Ubzr3bf6mSMC3qSxeOeior7BfhTLk86EQizQ5CkfBRDmdNQPH2dpkk/2c+b7cGLWzZU3tYq0nh0ACl6YWGMdNqwzgRBHgXDWDubG7HV0CT96Kd/LP57qhJUECJUy/z+vtdQBMwDyuz3Q70jyOkKQYxXB9wDZn4Gq7cNzuRgBNw1ZQTbB4qlgFAY3ceYLo90aSP+tnKtBitRZ3Ou6/Yfyy/qFq1mQ5woamdCMCSt7RX+S+beQufaCvKXFSF5ij8ZodoqR8WPgPFUYarXwpcHS/sbtu4tmzwmS5UZumVD9X4kv3oESTsOv440U4Z+FJlqTf1LiKjRTv94njNkqX/wd/e4Inf7op3rGpkFUFdNlqQk9l4/L2nEsFSO5nTgo7OH4V2jE9zn/KKqISa6I+CxDU3TTyXRV9nUmf1FvlV7AUMi3tXiM99+i4EksAly8O7yV66r9KJBerBAJjaNt1aTpwph+2CdrDhhm6gvflEfcdpxYIiYM6Uuk3MNt/gr5m8QuDVGoBpAPPVA=="
  }
}"#;
const TICKET_ID: &str = "c6s9FAa5fhb854BVMckqUBJ4hOXg2iE5i1FYPCuktks4eNZD";

fn get_keypair() -> Result<([u8; PRIVATE_KEY_SIZE], [u8; PUBLIC_KEY_SIZE])> {
    let kem_private_key_bytes = general_purpose::STANDARD
        .decode(PRIVATE_KEY_768_TESTING)
        .map_err(|e| anyhow::format_err!("Failed to decode base64 KEM private key: {}", e))?;
    let kem_private_key_bytes: [u8; PRIVATE_KEY_SIZE] = kem_private_key_bytes
        .try_into()
        .map_err(|_| anyhow::format_err!("Invalid KEM private key size"))?;
    let kem_public_key_bytes = general_purpose::STANDARD
        .decode(PUBLIC_KEY_768_TESTING)
        .map_err(|e| anyhow::format_err!("Failed to decode base64 KEM public key: {}", e))?;
    let kem_public_key_bytes: [u8; PUBLIC_KEY_SIZE] = kem_public_key_bytes
        .try_into()
        .map_err(|_| anyhow::format_err!("Invalid KEM public key size"))?;
    Ok((kem_private_key_bytes, kem_public_key_bytes))
}

// Helper to create a ServerRestApi pointing to mockito server
// Helper to create a mockito server and a ServerRestApi pointing to it
async fn setup_server_and_api() -> (mockito::ServerGuard, UdsBrokerApi) {
    log::setup_logging("debug", log::LogType::Test);

    let server = Server::new_async().await;
    let url = server.url() + "/"; // For testing, our base URL will be the mockito server

    log::info!("Setting up mock server and API client");
    let api = UdsBrokerApi::new(&url, None, false, true);
    // Pass the base url (without /ui) to the API
    (server, api)
}

#[tokio::test]
async fn test_get_version() {
    log::setup_logging("debug", log::LogType::Test);
    let (mut server, api) = setup_server_and_api().await;
    let result = types::BrokerResponse::<types::Version> {
        result: Some(types::Version {
            available_version: "5.0.0".to_string(),
            required_version: "4.0.0".to_string(),
            client_link: "https://example.com/client".to_string(),
        }),
        error: None,
    };
    let _m = server
        .mock("GET", "/")
        .match_header("content-type", "application/json")
        .with_body(serde_json::to_string(&result).unwrap())
        .with_status(200)
        .create_async()
        .await;
    let response = api.get_version_info().await;
    assert!(response.is_ok(), "Get version info failed: {:?}", response);
    let version = response.unwrap();
    assert_eq!(version.available_version, "5.0.0");
    assert_eq!(version.required_version, "4.0.0");
    assert_eq!(version.client_link, "https://example.com/client");
}

#[tokio::test]
async fn test_get_script() {
    log::setup_logging("debug", log::LogType::Test);
    let (mut server, api) = setup_server_and_api().await;
    let (privk, pubk) = get_keypair().unwrap();
    let api = api.with_keys(privk, pubk);
    let _m = server
        .mock(
            "POST",
            mockito::Matcher::Regex(format!(r"^/{}/ticket", TICKET_ID)),
        )
        .match_header("content-type", "application/json")
        .with_body(TICKET_RESPONSE_JSON)
        .with_status(200)
        .create_async()
        .await;
    let response = api.get_script(TICKET_ID, "scrabler").await;
    assert!(response.is_ok(), "Get script failed: {:?}", response);
    let script = response.unwrap();
    assert_eq!(script.script_type, types::ScriptType::Javascript);
}

#[tokio::test]
async fn test_get_script_fails() {
    log::setup_logging("debug", log::LogType::Test);
    let (mut server, api) = setup_server_and_api().await;
    let result = r#"{"error": {"message": "Test error", "is_retryable": false, "percent": 0}}"#;
    let _m = server
        .mock(
            "POST",
            mockito::Matcher::Regex(format!(r"^/{}/ticket", TICKET_ID)),
        )
        .match_header("content-type", "application/json")
        .with_body(result)
        .with_status(200)
        .create_async()
        .await;
    let response = api.get_script(TICKET_ID, "scrabler").await;
    assert!(
        response.is_err(),
        "Get script succeeded unexpectedly: {:?}",
        response
    );
    let err = response.err().unwrap();
    assert_eq!(err.message, "Test error".to_string());
    assert!(!err.is_retryable());
}

#[tokio::test]
async fn test_send_logs() {
    log::setup_logging("debug", log::LogType::Test);
    let (mut server, api) = setup_server_and_api().await;
    let _m = server
        .mock("POST", "/logs")
        .match_header("content-type", "application/json")
        .with_status(200)
        .create_async()
        .await;
    let response = api
        .send_log("DEBUG This is a test log message".to_string())
        .await;
    assert!(response.is_ok(), "Send logs failed: {:?}", response);
}
