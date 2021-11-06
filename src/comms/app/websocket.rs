use std::{sync::Arc, time::Duration};

use tokio::task::JoinHandle;

use crate::{app::CustomOneBot, config::WebSocketRev};

pub async fn run<E, A, R>(config: &WebSocketRev, ob: Arc<CustomOneBot<E, A, R>>) -> JoinHandle<()>
where
    E: Clone + serde::de::DeserializeOwned + Send + 'static + std::fmt::Debug,
    A: Clone + serde::Serialize + Send + 'static + std::fmt::Debug,
    R: Clone + serde::de::DeserializeOwned + Send + 'static + std::fmt::Debug,
{
    let config = config.clone();
    tokio::spawn(async move {
        loop {
            if let Some(ws_stream) = crate::comms::util::try_connect(&config).await {
                super::websocket_loop(ws_stream, ob.clone()).await;
            }
            tokio::time::sleep(Duration::from_secs(config.reconnect_interval as u64)).await;
        }
    })
}
