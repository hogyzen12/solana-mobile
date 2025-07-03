pub mod ffi;

use std::str::FromStr;

use anyhow::Result;
use async_channel::{unbounded, Receiver, Sender};
use dioxus::prelude::*;
use once_cell::sync::OnceCell;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    pubkey::Pubkey,
    transaction::{Transaction, VersionedTransaction},
};

const RPC_URL: &str = "https://rpc.ironforge.network/mainnet?apiKey=01J4NJDYJXSGJYE3AN6VXEB5VR";

// --- IPC Channel Setup ---
pub enum MsgFromKotlin {
    Pubkey(String),
    SignedTransaction(String),
}
static TX: OnceCell<Sender<MsgFromKotlin>> = OnceCell::new();
static RX: OnceCell<Receiver<MsgFromKotlin>> = OnceCell::new();

/// Initialize channels
fn init_ipc_channel() {
    let (tx, rx) = unbounded::<MsgFromKotlin>();
    TX.set(tx).expect("initialization of ffi sender just once.");
    RX.set(rx)
        .expect("initialization of ffi receiver just once.");
}

/// Send thru channel from kotlin to rust
pub fn send_msg_from_ffi(msg: MsgFromKotlin) {
    if let Some(tx) = TX.get() {
        let _ = tx.try_send(msg);
    }
}

// --- Dioxus App Setup ---
#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
enum Route {
    #[layout(Navbar)]
    #[route("/")]
    Home {},
    #[route("/blog/:id")]
    Blog { id: i32 },
}

const FAVICON: Asset = asset!("/assets/favicon.ico");
const MAIN_CSS: Asset = asset!("/assets/main.css");
const HEADER_SVG: Asset = asset!("/assets/header.svg");
const TAILWIND_CSS: Asset = asset!("/assets/tailwind.css");

#[derive(Debug, Clone)]
pub struct WalletState(Pubkey);

#[derive(Debug, Clone)]
pub enum TransactionState {
    None,
    WaitingForSignature,
    Signed(VersionedTransaction),
    Error(String),
}

fn main() {
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Debug),
    );
    init_ipc_channel();
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    // init wallet state
    let mut wallet_state = use_signal(|| WalletState(Pubkey::default()));
    use_context_provider(|| wallet_state);
    // init transaction sender state
    let mut transaction_state = use_signal(|| TransactionState::None);
    use_context_provider(|| transaction_state);
    // listen for messages from kotlin
    use_future(move || async move {
        if let Some(rx) = RX.get().cloned() {
            while let Ok(msg) = rx.recv().await {
                match msg {
                    MsgFromKotlin::Pubkey(base58) => {
                        if let Ok(pubkey) = Pubkey::from_str(base58.as_str()) {
                            wallet_state.set(WalletState(pubkey));
                        }
                    }
                    MsgFromKotlin::SignedTransaction(base58) => {
                        let res: Result<VersionedTransaction> = (|| -> Result<_> {
                            let bytes = bs58::decode(base58.as_str()).into_vec()?;
                            let tx =
                                bincode::deserialize::<VersionedTransaction>(bytes.as_slice())?;
                            Ok(tx)
                        })();
                        match res {
                            Ok(tx) => {
                                spawn(async move {
                                    // update state
                                    transaction_state.set(TransactionState::Signed(tx.clone()));
                                    // send transaction
                                    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                                    let client = RpcClient::new(RPC_URL.to_string());
                                    if let Err(err) = client.send_transaction(&tx).await {
                                        transaction_state
                                            .set(TransactionState::Error(err.to_string()));
                                    } else {
                                        transaction_state.set(TransactionState::None);
                                    }
                                });
                            }
                            Err(err) => {
                                transaction_state.set(TransactionState::Error(err.to_string()));
                            }
                        }
                    }
                }
            }
        }
    });

    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        document::Link { rel: "stylesheet", href: TAILWIND_CSS }
        Router::<Route> {}
    }
}

#[component]
pub fn Hero() -> Element {
    let wallet_state = use_context::<Signal<WalletState>>();
    let mut transaction_state = use_context::<Signal<TransactionState>>();
    let transaction: Resource<Result<VersionedTransaction>> = use_resource(move || async move {
        let pubkey = wallet_state.cloned();
        let ix = solana_sdk::system_instruction::transfer(&pubkey.0, &pubkey.0, 1_000);
        let mut tx = Transaction::new_with_payer(&[ix], Some(&pubkey.0));
        let client = RpcClient::new(RPC_URL.to_string());
        let hash = client.get_latest_blockhash().await?;
        tx.message.recent_blockhash = hash;
        let tx: VersionedTransaction = tx.into();
        Ok(tx)
    });
    rsx! {
        div { id: "hero",
            img { src: HEADER_SVG, id: "header" }
            button {
                onclick: move |_| {
                    let string = crate::ffi::initiate_mwa_session_from_dioxus();
                    log::debug!("session string: {:?}", string);
                },
                "proof"
            }
        }
        div { "{wallet_state.cloned().0}" }
        div {
            button {
                onclick: move |_| {
                    let tx = &*transaction.read();
                    if let Some(Ok(tx)) = tx {
                        match bincode::serialize(&tx) {
                            Ok(bytes) => {
                                transaction_state.set(TransactionState::WaitingForSignature);
                                crate::ffi::initiate_sign_transaction_from_dioxus(bytes.as_slice());
                            }
                            Err(err) => {
                                transaction_state.set(TransactionState::Error(err.to_string()));
                            }
                        }
                    }
                },
                "sign transaction"
            }
        }
        div {
            match transaction_state.cloned() {
                TransactionState::None => "no tx".to_string(),
                TransactionState::WaitingForSignature => "waiting for sig".to_string(),
                TransactionState::Signed(tx) => format!("{:?}", tx),
                TransactionState::Error(err) => err.to_string(),
            }
        }
    }
}

/// Home page
#[component]
fn Home() -> Element {
    rsx! {
        Hero {}
    }
}

/// Blog page
#[component]
pub fn Blog(id: i32) -> Element {
    rsx! {
        div { id: "blog",

            // Content
            h1 { "This is blog #{id}!" }
            p {
                "In blog #{id}, we show how the Dioxus router works and how URL parameters can be passed as props to our route components."
            }

            // Navigation links
            Link { to: Route::Blog { id: id - 1 }, "Previous" }
            span { " <---> " }
            Link { to: Route::Blog { id: id + 1 }, "Next" }
        }
    }
}

/// Shared navbar component.
#[component]
fn Navbar() -> Element {
    rsx! {
        div { id: "navbar",
            Link { to: Route::Home {}, "Home" }
            Link { to: Route::Blog { id: 1 }, "Blog" }
        }

        Outlet::<Route> {}
    }
}
