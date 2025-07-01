pub mod ffi;

use dioxus::prelude::*;
use once_cell::sync::OnceCell;
use std::sync::mpsc::{self, Sender};

// --- IPC Channel Setup ---

// 1. A static OnceCell to hold the sender part of our channel.
static IPC_SENDER: OnceCell<Sender<String>> = OnceCell::new();

// 2. A public function for the FFI layer to send messages.
// This function uses `get_or_init` to safely initialize the channel and receiver thread exactly once.
pub fn send_public_key_from_ffi(pk: String) {
    let sender = IPC_SENDER.get_or_init(|| {
        log::info!("Initializing IPC channel and spawning receiver thread.");
        let (tx, rx) = mpsc::channel::<String>();

        // Spawn a dedicated thread to listen for messages.
        spawn(async move {
            let mut wallet_state: Signal<WalletState> = use_context();
            // Loop forever, waiting for messages on the receiver.
            // This loop will only end if the sender is dropped, which should only happen
            // when the application is shutting down.
            for received_pk in rx {
                log::info!("Receiver thread got public key: {}", received_pk);
                // Update the global signal. This will trigger UI updates.
                *wallet_state.write() = WalletState(received_pk);
            }
            log::warn!(
                "IPC receiver thread shutting down. This should not happen in normal operation."
            );
        });

        // The closure returns the sender, which is then stored in the OnceCell.
        tx
    });

    // Now that we have a sender, try to send the public key.
    if let Err(e) = sender.send(pk) {
        log::error!("Failed to send public key through channel: {}", e);
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
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let wallet_state = use_signal(|| WalletState("no pubkey yet".to_string()));
    use_context_provider(|| wallet_state);
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
