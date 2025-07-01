pub mod ffi;

use async_channel::{unbounded, Receiver, Sender};
use dioxus::prelude::*;
use once_cell::sync::OnceCell;

// --- IPC Channel Setup ---
static TX: OnceCell<Sender<String>> = OnceCell::new();
static RX: OnceCell<Receiver<String>> = OnceCell::new();

/// Initialise once – typically at the very top of `main`
fn init_ipc_channel() {
    let (tx, rx) = unbounded::<String>();
    TX.set(tx).unwrap();
    RX.set(rx).unwrap();
}

// 2. A public function for the FFI layer to send messages.
// This function uses `get_or_init` to safely initialize the channel and receiver thread exactly once.
pub fn send_public_key_from_ffi(pk: String) {
    if let Some(tx) = TX.get() {
        // non‑blocking; if the buffer is full the value is dropped/logged
        let _ = tx.try_send(pk);
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
pub struct WalletState(String);

fn main() {
    // Set up our logger before launching the app.
    // The IPC channel is now initialized on demand.
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Debug),
    );
    init_ipc_channel();
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let wallet_state = use_signal(|| WalletState("no pubkey yet".into()));
    use_context_provider(|| wallet_state);
    let _cor: Coroutine<()> = use_coroutine(move |mut _scope| {
        // clone handles that are !Send: we are still on UI thread here
        let mut wallet_state = wallet_state.clone();
        let rx = RX.get().expect("channel not initialised").clone();

        async move {
            while let Ok(pk) = rx.recv().await {
                wallet_state.set(WalletState(pk)); // safe ‑ still UI thread
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
