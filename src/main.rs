pub mod ffi;

use dioxus::prelude::*;
use once_cell::sync::OnceCell;
use std::sync::mpsc::{self, Sender};

// --- IPC Channel Setup ---

// 1. A static OnceCell to hold the sender part of our channel.
static IPC_SENDER: OnceCell<Sender<String>> = OnceCell::new();

// 2. A public function for the FFI layer to send messages.
// This function will be called from `ffi.rs`.
pub fn send_public_key_from_ffi(pk: String) {
    if let Some(sender) = IPC_SENDER.get() {
        if let Err(e) = sender.send(pk) {
            log::error!("Failed to send public key through channel: {}", e);
        }
    } else {
        log::error!("IPC_SENDER not initialized");
    }
}

// 3. The setup function to be called once at startup.
fn setup_ipc_channel() {
    let (tx, rx) = mpsc::channel::<String>();

    // Store the sender in our static OnceCell.
    // If this fails, it means the channel is already set up, which is fine.
    if IPC_SENDER.set(tx).is_err() {
        log::warn!("IPC_SENDER was already initialized.");
        return;
    }

    // Spawn a dedicated thread to listen for messages.
    std::thread::spawn(move || {
        // Loop forever, waiting for messages on the receiver.
        for received_pk in rx {
            log::info!("Receiver thread got public key: {}", received_pk);
            // Update the global signal. This will trigger UI updates.
            *PUBLIC_KEY.write() = Some(received_pk);
        }
        log::info!("IPC receiver thread shutting down.");
    });
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

// The GlobalSignal remains the source of truth for the UI.
static PUBLIC_KEY: GlobalSignal<Option<String>> =
    GlobalSignal::new(|| SyncSignal::new_maybe_sync(None)());

fn main() {
    // Set up our logger and the IPC channel before launching the app.
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Debug),
    );
    setup_ipc_channel();
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        document::Link { rel: "stylesheet", href: TAILWIND_CSS }
        Router::<Route> {}
    }
}

#[component]
pub fn Hero() -> Element {
    // The Hero component now directly reads from the GlobalSignal.
    // When the receiver thread updates the signal, this component will automatically re-render.
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
        // Display the public key from the global signal.
        if let Some(key) = &*PUBLIC_KEY.read() {
            div { "Public Key: {key}" }
        } else {
            div { "No public key yet." }
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
