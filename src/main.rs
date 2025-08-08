// Updated main.rs - Following the simplified pattern from original_main.rs

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

use components::*;

// Add MWA imports for Android only
#[cfg(target_os = "android")]
use std::str::FromStr;
#[cfg(target_os = "android")]
use async_channel::{unbounded, Receiver, Sender};
#[cfg(target_os = "android")]
use once_cell::sync::OnceCell;
#[cfg(target_os = "android")]
use solana_sdk::pubkey::Pubkey;

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

// Simple MWA state enum (following original_main.rs pattern)
#[cfg(target_os = "android")]
#[derive(Debug, Clone)]
pub enum WalletState {
    None,
    Pubkey(Pubkey),
}

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
    // Simple MWA state management (Android only) - Following original_main.rs pattern
    #[cfg(target_os = "android")]
    {
        // Create simple wallet state (no complex MwaWallet struct)
        let mut mwa_wallet_state = use_signal(|| WalletState::None);
        use_context_provider(|| mwa_wallet_state);
        
        // Listen for MWA messages from Kotlin (EXACT pattern from original_main.rs)
        use_future(move || async move {
            if let Some(rx) = RX.get().cloned() {
                while let Ok(msg) = rx.recv().await {
                    match msg {
                        MsgFromKotlin::Pubkey(base58) => {
                            if let Ok(pubkey) = Pubkey::from_str(base58.as_str()) {
                                log::info!("🔗 MWA Connected with pubkey: {}", pubkey);
                                mwa_wallet_state.set(WalletState::Pubkey(pubkey));
                            }
                        }
                        MsgFromKotlin::SignedTransaction(base64_tx) => {
                            log::info!("📝 MWA: Received signed transaction: {}", base64_tx);
                            // Handle signed transaction here if needed
                        }
                        MsgFromKotlin::SignedMessage(signature) => {
                            log::info!("✍️ MWA: Received signed message: {}", signature);
                            // Handle signed message here if needed
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