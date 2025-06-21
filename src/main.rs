use dioxus::prelude::*;

// JNI and logging specific imports
use jni::objects::{GlobalRef, JObject};
use jni::sys::{jint, JNI_VERSION_1_6};
use jni::{JNIEnv, JavaVM};
use std::sync::Once;
use once_cell::sync::OnceCell; // Added OnceCell

// For logging
use android_logger::Config;
use log::{error, info, LevelFilter};

// Static variables to store JavaVM and Activity references using OnceCell for safe initialization
static JAVAVM: OnceCell<JavaVM> = OnceCell::new();
static ACTIVITY_INSTANCE: OnceCell<GlobalRef> = OnceCell::new();
static INIT_LOGGER: Once = Once::new();

// Function to initialize the Android logger
fn init_android_logger() {
    INIT_LOGGER.call_once(|| {
        android_logger::init_once(
            Config::default()
                .with_max_level(LevelFilter::Trace) // Adjust log level as needed
                .with_tag("RustDioxusApp"), // Custom tag for your app
        );
        info!("Android logger initialized from Rust.");
    });
}

// JNI_OnLoad function to store the JavaVM instance
#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn JNI_OnLoad(vm: JavaVM, _: *mut std::ffi::c_void) -> jint {
    init_android_logger(); // Initialize logger when library is loaded
    info!("JNI_OnLoad called");
    if JAVAVM.set(vm).is_err() {
        error!("JNI_OnLoad: JavaVM global instance could not be set. It might have been already set.");
    }
    JNI_VERSION_1_6
}

// This function will be called from WryActivity.kt to pass the Activity instance
#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_dev_dioxus_main_WryActivity_initRustApp(
    env: JNIEnv,
    _class: JObject, // Represents the class that declared this native method (WryActivity)
    activity: JObject, // The Activity instance passed from Kotlin
) {
    init_android_logger(); // Ensure logger is initialized
    info!("Java_dev_dioxus_main_WryActivity_initRustApp called");
    match env.new_global_ref(activity) {
        Ok(global_ref) => {
            if ACTIVITY_INSTANCE.set(global_ref).is_err() {
                error!("Java_dev_dioxus_main_WryActivity_initRustApp: Activity global instance could not be set. It might have been already set.");
            } else {
                info!("Activity instance stored successfully.");
            }
        }
        Err(e) => {
            error!("Failed to create global ref for Activity: {:?}", e);
        }
    }
}

// Public Rust function that Dioxus will call
pub fn rust_initiate_wallet_connect() {
    init_android_logger(); // Ensure logger is initialized
    info!("rust_initiate_wallet_connect called");

    // Get the JavaVM instance from OnceCell
    let jvm = match JAVAVM.get() {
        Some(vm_instance) => vm_instance,
        None => {
            error!("JavaVM has not been initialized. JNI_OnLoad might not have been called or failed.");
            return;
        }
    };

    // Check if the Activity instance is available
    if ACTIVITY_INSTANCE.get().is_none() {
        error!("Activity instance has not been initialized. Java_dev_dioxus_main_WryActivity_initRustApp might not have been called or failed.");
        return;
    }

    match jvm.get_env() {
        Ok(mut env) => { // 'env' is JNIEnv. Note: 'mut env' might still trigger a warning if not all paths use mutability.
            info!("Successfully got JNIEnv");
            // Attempt to find the SolanaWalletManager class
            match env.find_class("dev/dioxus/main/SolanaWalletManager") {
                Ok(_class_obj) => {
                    info!("Found SolanaWalletManager class!");
                    // TODO (Task ID 27):
                    // 1. Get the Activity JObject from ACTIVITY_INSTANCE.get().unwrap().
                    // 2. Create an instance of SolanaWalletManager (or get a static one).
                    //    If creating an instance, it might need the Activity.
                    // 3. Get the method ID for "initiateConnect".
                    // 4. Call the "initiateConnect" method, passing the Activity JObject.
                }
                Err(e) => {
                    error!("Failed to find SolanaWalletManager class: {:?}", e);
                }
            }
        }
        Err(e) => {
            error!("Failed to get JNIEnv: {:?}", e);
        }
    }
}

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

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: MAIN_CSS } document::Link { rel: "stylesheet", href: TAILWIND_CSS }
        Router::<Route> {}
    }
}

#[component]
pub fn Hero() -> Element {
    rsx! {
        div {
            id: "hero",
            img { src: HEADER_SVG, id: "header" }
            div { id: "links",
                a { href: "https://dioxuslabs.com/learn/0.6/", "📚 Learn Dioxus" }
                a { href: "https://dioxuslabs.com/awesome", "🚀 Awesome Dioxus" }
                a { href: "https://github.com/dioxus-community/", "📡 Community Libraries" }
                a { href: "https://github.com/DioxusLabs/sdk", "⚙️ Dioxus Development Kit" }
                a { href: "https://marketplace.visualstudio.com/items?itemName=DioxusLabs.dioxus", "💫 VSCode Extension" }
                a { href: "https://discord.gg/XgGxMSkvUM", "👋 Community Discord" }
            }
        }
    }
}

/// Home page
#[component]
fn Home() -> Element {
    rsx! {
        Hero {}
        div { // Added a div for the button
            button {
                onclick: |_| {
                    // Log that the button was clicked
                    info!("Connect Wallet button clicked via Dioxus UI.");
                    // Call the Rust function that will eventually trigger the JNI call
                    rust_initiate_wallet_connect();
                },
                "Connect Wallet"
            }
        }
    }
}

/// Blog page
#[component]
pub fn Blog(id: i32) -> Element {
    rsx! {
        div {
            id: "blog",

            // Content
            h1 { "This is blog #{id}!" }
            p { "In blog #{id}, we show how the Dioxus router works and how URL parameters can be passed as props to our route components." }

            // Navigation links
            Link {
                to: Route::Blog { id: id - 1 },
                "Previous"
            }
            span { " <---> " }
            Link {
                to: Route::Blog { id: id + 1 },
                "Next"
            }
        }
    }
}

/// Shared navbar component.
#[component]
fn Navbar() -> Element {
    rsx! {
        div {
            id: "navbar",
            Link {
                to: Route::Home {},
                "Home"
            }
            Link {
                to: Route::Blog { id: 1 },
                "Blog"
            }
        }

        Outlet::<Route> {}
    }
}
