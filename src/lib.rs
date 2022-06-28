#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub const VERSION: &str = std::env!("CARGO_PKG_VERSION");
pub const WALLE_CORE: &str = "Walle-core";

pub mod error;
pub mod util;

/// Onebot Action
pub mod action;
/// 相关配置项
pub mod config;
/// Onebot Event
pub mod event;
pub mod message;
/// Onebot ActionResp
pub mod resp;

mod test;

pub mod prelude {
    pub use crate::action::StandardAction;
    pub use crate::event::*;
    pub use crate::message::{IntoMessage, Message, MessageAlt, MessageBuild, MessageSegment};
    pub use crate::resp::{Resp, RespContent, Resps, StandardResps};

    pub use crate::error::{WalleError, WalleResult};
    pub use async_trait::async_trait;
}

#[cfg(any(feature = "impl_obc", feature = "app_obc"))]
pub mod obc;

/// ECAH: EventConstructor + ActionHandler
/// EHAC: EventHandler + ActionConstructor
pub struct OneBot<AH, EH, const V: u8> {
    pub action_handler: AH,
    pub event_handler: EH,

    // Some for running, None for stopped
    signal: std::sync::Mutex<Option<tokio::sync::broadcast::Sender<()>>>,
}

use std::sync::Arc;

use async_trait::async_trait;

use crate::error::{WalleError, WalleResult};

#[async_trait]
pub trait ActionHandler<E, A, R, const V: u8> {
    type Config;
    async fn start<AH, EH>(
        &self,
        ob: &Arc<OneBot<AH, EH, V>>,
        config: Self::Config,
    ) -> WalleResult<Vec<tokio::task::JoinHandle<()>>>
    where
        AH: ActionHandler<E, A, R, V> + Send + Sync + 'static,
        EH: EventHandler<E, A, R, V> + Send + Sync + 'static;
    async fn call<AH, EH>(&self, action: A, ob: &OneBot<AH, EH, V>) -> WalleResult<R>
    where
        AH: ActionHandler<E, A, R, V> + Send + Sync + 'static,
        EH: EventHandler<E, A, R, V> + Send + Sync + 'static;
}

#[async_trait]
pub trait EventHandler<E, A, R, const V: u8> {
    type Config;
    async fn start<AH, EH>(
        &self,
        ob: &Arc<OneBot<AH, EH, V>>,
        config: Self::Config,
    ) -> WalleResult<Vec<tokio::task::JoinHandle<()>>>
    where
        AH: ActionHandler<E, A, R, V> + Send + Sync + 'static,
        EH: EventHandler<E, A, R, V> + Send + Sync + 'static;
    async fn call<AH, EH>(&self, event: E, ob: &OneBot<AH, EH, V>)
    where
        AH: ActionHandler<E, A, R, V> + Send + Sync + 'static,
        EH: EventHandler<E, A, R, V> + Send + Sync + 'static;
}

impl<AH, EH, const V: u8> OneBot<AH, EH, V> {
    pub fn new(action_handler: AH, event_handler: EH) -> Self {
        Self {
            action_handler,
            event_handler,
            signal: std::sync::Mutex::new(None),
        }
    }
    pub async fn start<E, A, R>(
        self: &Arc<Self>,
        ah_config: AH::Config,
        eh_config: EH::Config,
        ah_first: bool,
    ) -> WalleResult<Vec<tokio::task::JoinHandle<()>>>
    where
        E: Send + Sync + 'static,
        A: Send + Sync + 'static,
        R: Send + Sync + 'static,
        AH: ActionHandler<E, A, R, V> + Send + Sync + 'static,
        EH: EventHandler<E, A, R, V> + Send + Sync + 'static,
    {
        let mut signal = self.signal.lock().unwrap();
        if signal.is_none() {
            let (tx, _) = tokio::sync::broadcast::channel(1);
            *signal = Some(tx);
        } else {
            return Err(WalleError::AlreadyRunning);
        }
        drop(signal);
        let mut tasks = vec![];
        if ah_first {
            tasks.extend(
                self.action_handler
                    .start(self, ah_config)
                    .await?
                    .into_iter(),
            );
            tasks.extend(self.event_handler.start(self, eh_config).await?.into_iter());
        } else {
            tasks.extend(self.event_handler.start(self, eh_config).await?.into_iter());
            tasks.extend(
                self.action_handler
                    .start(self, ah_config)
                    .await?
                    .into_iter(),
            );
        }
        Ok(tasks)
    }
    pub fn started(&self) -> bool {
        self.signal.lock().unwrap().is_some()
    }
    pub fn get_signal_rx(&self) -> WalleResult<tokio::sync::broadcast::Receiver<()>> {
        Ok(self
            .signal
            .lock()
            .unwrap()
            .as_ref()
            .ok_or(WalleError::NotRunning)?
            .subscribe())
    }
    pub fn shutdown(&self) -> WalleResult<()> {
        let tx = self
            .signal
            .lock()
            .unwrap()
            .take()
            .ok_or(WalleError::NotRunning)?;
        tx.send(()).ok();
        Ok(())
    }
}
