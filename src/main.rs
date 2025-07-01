pub mod ffi;

use std::str::FromStr;

use async_channel::{unbounded, Receiver, Sender};
use dioxus::prelude::*;
use once_cell::sync::OnceCell;
use solana_sdk::pubkey::Pubkey;

// --- IPC Channel Setup ---
#[derive(serde::Serialize, serde::Deserialize)]
pub enum MsgFromKotlin {
    Pubkey(String),
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

fn main() {
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Debug),
    );
    init_ipc_channel();
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let mut wallet_state = use_signal(|| WalletState(Pubkey::default()));
    use_context_provider(|| wallet_state);
    use_future(move || async move {
        if let Some(rx) = RX.get().cloned() {
            while let Ok(msg) = rx.recv().await {
                match msg {
                    MsgFromKotlin::Pubkey(base58) => {
                        if let Ok(pubkey) = Pubkey::from_str(base58.as_str()) {
                            wallet_state.set(WalletState(pubkey));
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
