use tokio::io::AsyncWriteExt;

use super::{connection::connect_and_upgrade, test_utils::run_test_server};
use crate::log;
use crate::system::trigger::Trigger;
use std::time::Duration;

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
