use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, RwLock};

use crate::compat;
use super::Event;

pub struct Manager {
    control_sender: broadcast::Sender<Event>,
    disconnect_channel: broadcast::Sender<bool>,
    last_response: Arc<RwLock<Duration>>,
}

impl Manager {
    pub fn new(
        control_sender: broadcast::Sender<Event>,
        disconnect_channel: broadcast::Sender<bool>,
        last_response: Arc<RwLock<Duration>>,
    ) -> Self {
        Self {
            control_sender,
            disconnect_channel,
            last_response
        }
    }

    pub async fn start(&mut self, id: u32) {
    log::trace!("starting ping loop {}", id);
        {
            *self.last_response.write().await = compat::now();
        }
        let mut disconnect_receiver = self.disconnect_channel.subscribe();
        let mut waiting_for_pong = false;
        loop {
            log::trace!("...checking ping... {}", id);
            if disconnect_receiver.try_recv() == Ok(true) {
                log::trace!("disconnect signal received! breaking ping loop {}", id);
                break;
            }
            if let Ok(last_response_time) = self.last_response.try_read() {
                let now = compat::now();
                if now.saturating_sub(*last_response_time) > Duration::from_secs(60) {
                    log::trace!("DISCONNECT ping websocket is unresponsive, closing the connection and trying again in 60s {}", id);
                    let _ = self.disconnect_channel.send(true);
                    break;
                } else if !waiting_for_pong && now.saturating_sub(*last_response_time) > Duration::from_secs(30) {
                    log::trace!("no response from websocket for 30 seconds - request a ping {}", id);
                    let _ = self.control_sender.send(Event::Ping);
                    waiting_for_pong = true;
                } else if waiting_for_pong && now.saturating_sub(*last_response_time) <= Duration::from_secs(30) {
                    // recent response
                    waiting_for_pong = false;
                }
            }
            compat::sleep(1_000).await;
        }
        log::trace!("ending ping loop {}", id);
    }
}