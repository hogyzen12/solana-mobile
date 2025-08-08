// Replace your current main.rs with this corrected version:

use dioxus::prelude::*;

mod wallet;
mod rpc;
mod prices;
mod transaction;
mod signing;
mod hardware;
mod storage;
mod components;
mod validators;
mod staking;
mod currency;
mod currency_utils;

// Add MWA modules for Android only
#[cfg(target_os = "android")]
pub mod ffi;
#[cfg(target_os = "android")]
pub mod mwa;

use components::*;

// Add MWA imports for Android only
#[cfg(target_os = "android")]
use std::str::FromStr;
#[cfg(target_os = "android")]
use anyhow::Result;
#[cfg(target_os = "android")]
use async_channel::{unbounded, Receiver, Sender};
#[cfg(target_os = "android")]
use once_cell::sync::OnceCell;
#[cfg(target_os = "android")]
use solana_sdk::{
    pubkey::Pubkey,
    signature::Signature,
    transaction::{Transaction, VersionedTransaction},
};

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
enum Route {
    #[route("/")]
    WalletView {},
}

const MAIN_CSS: Asset = asset!("/assets/main.css");

// MWA IPC Channel Setup (Android only)
#[cfg(target_os = "android")]
pub enum MsgFromKotlin {
    Pubkey(String),
    SignedTransaction(String),
    SignedMessage(String),
}

#[cfg(target_os = "android")]
static TX: OnceCell<Sender<MsgFromKotlin>> = OnceCell::new();
#[cfg(target_os = "android")]
static RX: OnceCell<Receiver<MsgFromKotlin>> = OnceCell::new();

/// Initialize channels (Android only)
#[cfg(target_os = "android")]
fn init_ipc_channel() {
    let (tx, rx) = unbounded::<MsgFromKotlin>();
    TX.set(tx).expect("initialization of ffi sender just once.");
    RX.set(rx).expect("initialization of ffi receiver just once.");
}

/// Send thru channel from kotlin to rust (Android only)
#[cfg(target_os = "android")]
pub fn send_msg_from_ffi(msg: MsgFromKotlin) {
    if let Some(tx) = TX.get() {
        let _ = tx.try_send(msg);
    }
}

fn main() {
    // Initialize Android logger
    #[cfg(target_os = "android")]
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Debug),
    );
    
    // Initialize MWA IPC channel on Android
    #[cfg(target_os = "android")]
    init_ipc_channel();
    
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    // Initialize MWA state management on Android
    #[cfg(target_os = "android")]
    {
        use crate::mwa::MwaWallet;
        
        // Create and provide global MWA wallet instance
        let mut mwa_wallet = use_signal(|| MwaWallet::new());
        use_context_provider(|| mwa_wallet);
        
        // Listen for MWA messages from Kotlin
        use_future(move || async move {
            if let Some(rx) = RX.get().cloned() {
                while let Ok(msg) = rx.recv().await {
                    match msg {
                        MsgFromKotlin::Pubkey(base58) => {
                            if let Ok(pubkey) = Pubkey::from_str(&base58) {
                                log::info!("MWA: Connected with pubkey: {}", pubkey);
                                mwa_wallet.write().set_connected(pubkey).await;
                            }
                        }
                        MsgFromKotlin::SignedTransaction(base64_tx) => {
                            log::info!("MWA: Received signed transaction");
                            mwa_wallet.write().handle_signed_transaction(base64_tx).await;
                        }
                        MsgFromKotlin::SignedMessage(signature) => {
                            log::info!("MWA: Received signed message");
                            mwa_wallet.write().handle_signed_message(signature).await;
                        }
                    }
                }
            }
        });
    }

    rsx! {
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        Router::<Route> {}
    }
}