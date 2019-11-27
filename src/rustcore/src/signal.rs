use tokio::signal::ctrl_c;

pub(crate) async fn wait_for_ctrl_c() {
    match ctrl_c().await {
        Ok(_) => (),
        Err(e) => {
            warn!("Error receiving SIGTERM: {}", e);
        }
    }
}
