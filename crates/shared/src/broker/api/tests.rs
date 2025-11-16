// Copyright (c) 2025 Virtual Cable S.L.U.
// All rights reserved.
// Redistribution and use in source and binary forms, with or without modification,
// are permitted provided that the following conditions are met:
//    * Redistributions of source code must retain the above copyright notice,
//      this list of conditions and the following disclaimer.
//    * Redistributions in binary form must reproduce the above copyright notice,
//      this list of conditions and the following disclaimer in the documentation
//      and/or other materials provided with the distribution.
//    * Neither the name of Virtual Cable S.L.U. nor the names of its contributors
//      may be used to endorse or promote products derived from this software
//      without specific prior written permission.
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
/*!
Author: Adolfo Gómez, dkmaster at dkmon dot com
*/
use super::*;

use crate::log;

use mockito::Server;

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
    let result: types::BrokerResponse<types::Script> = types::BrokerResponse::<types::Script> {
        result: Some(types::get_test_script()),
        error: None,
    };
    let _m = server
        .mock(
            "GET",
            mockito::Matcher::Regex(r"^/ticket/scrabler\?hostname=.*&version=.*$".to_string()),
        )
        .match_header("content-type", "application/json")
        .with_body(serde_json::to_string(&result).unwrap())
        .with_status(200)
        .create_async()
        .await;
    let response = api.get_script("ticket", "scrabler").await;
    assert!(response.is_ok(), "Get script failed: {:?}", response);
    let script = response.unwrap();
    assert_eq!(script.script_type, "javascript");
}

#[tokio::test]
async fn test_get_script_fails() {
    log::setup_logging("debug", log::LogType::Tests);
    let (mut server, api) = setup_server_and_api().await;
    let result = r#"{"error": {"message": "Test error", "is_retryable": false, "percent": 0}}"#;
    let _m = server
        .mock(
            "GET",
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
