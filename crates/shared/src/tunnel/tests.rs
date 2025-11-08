use tokio::io::AsyncWriteExt;

use super::{
    connection::{connect_and_upgrade, send_open_cmd, send_test_cmd},
    test_utils::run_test_server,
};
use crate::log;
use crate::system::trigger::Trigger;
use crate::tunnel::consts;
use std::time::Duration;

use rand::{Rng, distr::Alphanumeric, rng};

#[tokio::test]
async fn test_connect_and_upgrade() {
    log::setup_logging("debug", log::LogType::Tests);
    crate::tls::init_tls(None);
    let trigger = Trigger::new();
    let server_handle = tokio::spawn({
        let trigger = trigger.clone();
        async move {
            run_test_server(44910, trigger).await.unwrap();
        }
    });
    // Give the server a moment to start
    tokio::time::sleep(Duration::from_millis(500)).await;
    log::debug!("Starting test_connect_and_upgrade");
    let (reader, mut writer) = connect_and_upgrade("localhost", 44910, false)
        .await
        .expect("Failed to connect and upgrade to TLS");
    // If we reach here, the connection and upgrade were successful
    log::debug!("Connected and upgraded to TLS successfully");
    writer.shutdown().await.ok();
    drop(writer);
    drop(reader);
    trigger.set();
    server_handle.await.unwrap();
}

#[tokio::test]
async fn test_test_and_open_cmd() {
    log::setup_logging("debug", log::LogType::Tests);
    crate::tls::init_tls(None);
    let trigger = Trigger::new();
    let server_handle = tokio::spawn({
        let trigger = trigger.clone();
        async move {
            run_test_server(44911, trigger).await.unwrap();
        }
    });
    // Give the server a moment to start
    tokio::time::sleep(Duration::from_millis(500)).await;
    log::debug!("Starting test_text_cmd");
    let (mut reader, mut writer) = connect_and_upgrade("localhost", 44911, false)
        .await
        .expect("Failed to connect and upgrade to TLS");

    // Send CMD_TEST
    send_test_cmd(&mut reader, &mut writer).await.unwrap(); //will panic on error

    // Send CMD_OPEN with a ticket
    //consts::TICKET_LENGTH
    let rnd_ticket = rng()
        .sample_iter(&Alphanumeric)
        .take(consts::TICKET_LENGTH)
        .map(char::from)
        .collect::<String>();
    send_open_cmd(&mut reader, &mut writer, &rnd_ticket)
        .await
        .unwrap(); //will panic on error

    log::debug!("Text command tests completed successfully");
    writer.shutdown().await.ok();
    drop(writer);
    drop(reader);
    trigger.set();
    server_handle.await.unwrap();
}
