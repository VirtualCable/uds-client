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

use crate::log;

use mockito::Server;

const TICKET_RESPONSE_JSON: &str = r#"{
  "result": {
    "algorithm": "AES-256-GCM",
    "ciphertext": "5QORerhVDPvU+/UdYbLIpeuc13hzbl8IUIqsZbXvR/8wYTcJauRjQOjUxDZg+RAeKXlsjb2FTf9265I2cweQpPcYPURitnAoPtm/cQWf7cJslE5lX220WInULLnElP4NS7MC9G6qgElc0JBSIlDK9y4AacI2T5k+VtTEMVB5j3r1EMbL1RBtDW43+MBFo6i8hDZMY1qM2CG57H+ueIVAloCzz6zSGHoWJcQDXJYJQdYOpNwI6YbMpRnKIkfIxqFfMT0TjhNg4XJxMbNfgMgpTIjjb3/JDOaaILJIyx0mRh5wWOkszu1n4OF5ZxUSLNKbj3L6HK3IoO8CAem1cFZyoPM2KmG91NYfp2if28amHc7aTXN81Z/Wxm1fBOTT43ufLB8K7LuD5og4vpA7qZB5uDSq7xGh3EfSfG861diOUdXKEklnTHxTF/ne6tOVDQUdRoyD2L0hBDvFX62k+ASBJXjdtuHHEMCGT2YXCSuEl2FNYZwOfDbA17H26AJsWEDV01zoqFOBpBgS3qdHyMLpkonPtnC47fU/CRwCYjsvW7XD/RvgQejbkqn3yxHSUiu+jY+yHDh21cJyvHhUteF0BIxJngpWrffsZVlfePdYMe1Ws/din36hkVYVhwJeqYQXVwzdNKyPpesZlHrXQHJ0yuPiK6dRuSw12wQCR0yYHClehrbebCLcc2As+uF6gUEJJIBOhDiiOVuEaXrFmKsH0dRYeKVwFDRJZWBx6IqqmeVXaw/dTwt0pP1K9mzosxNbhgrQS/jq5Ml25/rJOIU7PlEdqu8rbZk1w91/CY68u7je8NpZs2490mBBIbD+ipJ9Qk+vxt9RJFmTVuChNmLqv8TdNXqSzn2VDNVZ04cFU2BUwIScOZXHIEpxrdeKC2Gdyvgnq7DFN1oPaeewIOtAmckHjoGJMxCFFvfk9tA03oEnarBaVgrrolOK5MahwFzZ1q9y528aJEpxTCofygKKjePTlNuMnEUJ7y/aLrnQJPURetS1HfqWAh5sSiPYSlPU5TxSZA+niQJAZxYUruepv9fhYNWO6KVwKgHhLhN3ogRiNhi0FtIXvqePHDSNYg1skXhrrQj2V3p/YouCnbUsPcGKFoX3lMrt2mjINHdyslx3H2LkieJSUa0UiKUzvDC+4wTAR1jPIQZ48rbKzEApi82cmzEtjXa5I79pje6vYxdRTYLcepDNjk5+EpuMO2wEYLzM7ZtW5Q3G1kKAk2xqbcw7b3z6oX33/l4eMYEhC+P+Tx/imXfWNLk+6WFJjK5pIO9VqoKE+dZNy7Q11o1txjXQcJV2FqZutG6GUlkoHoXyR1AvvJFLG4LG6L7/cmDN2CnovPzq1eyCkzL/+OeJvQYsI7ExbtWbnVMQC2kL6pV/Dd0Mi53o1WETpC1UBcvKspCdHwS+dqG4bT1rCyb1xwXY0u+u1JnG28me+MIA+H4=",
    "data": "pNZ00G0QY71OaYi4qlxw/aRL2QssUF8Ubzr3bf6mSMC3qSxeOeior7BfhTLk86EQizQ5CkfBRDmdNQPH2dpkk/2c+b7cGLWzZU3tYq0nh0ACl6YWGMdNqwzgRBHgXDWDubG7HV0CT96Kd/LP57qhJUECJUy/z+vtdQBMwDyuz3Q70jyOkKQYxXB9wDZn4Gq7cNzuRgBNw1ZQTbB4qlgFAY3ceYLo90aSP+tnKtBitRZ3Ou6/Yfyy/qFq1mQ5woamdCMCSt7RX+S+beQufaCvKXFSF5ij8ZodoqR8WPgPFUYarXwpcHS/sbtu4tmzwmS5UZumVD9X4kv3oESTsOv440U4Z+FJlqTf1LiKjRTv94njNkqX/wd/e4Inf7op3rGpkFUFdNlqQk9l4/L2nEsFSO5nTgo7OH4V2jE9zn/KKqISa6I+CxDU3TTyXRV9nUmf1FvlV7AUMi3tXiM99+i4EksAly8O7yV66r9KJBerBAJjaNt1aTpwph+2CdrDhhm6gvflEfcdpxYIiYM6Uuk3MNt/gr5m8QuDVGoBpAPPVA=="
  }
}"#;
const TICKET_ID: &str = "c6s9FAa5fhb854BVMckqUBJ4hOXg2iE5i1FYPCuktks4eNZD";

// Helper to create a ServerRestApi pointing to mockito server
// Helper to create a mockito server and a ServerRestApi pointing to it
async fn setup_server_and_api() -> (mockito::ServerGuard, UdsBrokerApi) {
    log::setup_logging("debug", log::LogType::Tests);

    let server = Server::new_async().await;
    let url = server.url() + "/"; // For testing, our base URL will be the mockito server

    log::info!("Setting up mock server and API client");
    let api = UdsBrokerApi::new(&url, None, false, true);
    // Pass the base url (without /ui) to the API
    (server, api)
}

#[tokio::test]
async fn test_get_version() {
    log::setup_logging("debug", log::LogType::Tests);
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
    log::setup_logging("debug", log::LogType::Tests);
    let (mut server, api) = setup_server_and_api().await;
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
    log::setup_logging("debug", log::LogType::Tests);
    let (mut server, api) = setup_server_and_api().await;
    let result = r#"{"error": {"message": "Test error", "is_retryable": false, "percent": 0}}"#;
    let _m = server
        .mock(
            "POST",
            mockito::Matcher::Regex(r"^/ticket/scrabler\?hostname=.*&version=.*$".to_string()),
        )
        .match_header("content-type", "application/json")
        .with_body(result)
        .with_status(200)
        .create_async()
        .await;
    let response = api.get_script("ticket", "scrabler").await;
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
    log::setup_logging("debug", log::LogType::Tests);
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
