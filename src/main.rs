pub mod ffi;

use std::str::FromStr;

use anyhow::Result;
use async_channel::{unbounded, Receiver, Sender};
use dioxus::prelude::*;
use once_cell::sync::OnceCell;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    pubkey::Pubkey,
    signature::Signature,
    transaction::{Transaction, VersionedTransaction},
};

const RPC_URL: &str = "https://rpc.ironforge.network/mainnet?apiKey=01J4NJDYJXSGJYE3AN6VXEB5VR";

// --- IPC Channel Setup ---
#[derive(Debug, Clone)]
pub enum MsgFromKotlin {
    Pubkey(String),
    SignedTransaction(String),
    SignedMessage(String),
    // USB variants
    UsbDeviceList(String),
    UsbPermissionResult(String),
    UsbDeviceOpened(String),
    UsbDeviceClosed(String),
    UsbDataWritten(String),
    UsbDataRead(String),
    UsbError(String),
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
    #[route("/usb")]
    UsbPage {},
}

const FAVICON: Asset = asset!("/assets/favicon.ico");
const MAIN_CSS: Asset = asset!("/assets/main.css");
const HEADER_SVG: Asset = asset!("/assets/header.svg");
const TAILWIND_CSS: Asset = asset!("/assets/tailwind.css");

#[derive(Debug, Clone)]
pub enum WalletState {
    None,
    Pubkey(Pubkey),
}

#[derive(Debug, Clone)]
pub enum TransactionState {
    None,
    WaitingForSignature,
    Signed(VersionedTransaction),
    Error(String),
}

#[derive(Debug, Clone)]
pub enum MessageState {
    None,
    WaitingForSignature(String),
    Signed(String, Vec<u8>),
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
    // Existing wallet state
    let mut wallet_state = use_signal(|| WalletState::None);
    use_context_provider(|| wallet_state);
    let mut transaction_state = use_signal(|| TransactionState::None);
    use_context_provider(|| transaction_state);
    let mut message_state = use_signal(|| MessageState::None);
    use_context_provider(|| message_state);

    // Add USB state - fix: remove mut from selected_device_state
    let mut usb_devices_state = use_signal(|| String::new());
    use_context_provider(|| usb_devices_state);
    let mut usb_status_state = use_signal(|| "Ready".to_string());
    use_context_provider(|| usb_status_state);
    let selected_device_state = use_signal(|| String::new());
    use_context_provider(|| selected_device_state);

    // Listen for messages from kotlin
    use_future(move || async move {
        if let Some(rx) = RX.get().cloned() {
            while let Ok(msg) = rx.recv().await {
                match msg {
                    MsgFromKotlin::Pubkey(base58) => {
                        if let Ok(pubkey) = Pubkey::from_str(base58.as_str()) {
                            wallet_state.set(WalletState::Pubkey(pubkey));
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
                    MsgFromKotlin::SignedMessage(signature) => {
                        if let (
                            MessageState::WaitingForSignature(message),
                            WalletState::Pubkey(pubkey),
                        ) = (message_state.cloned(), wallet_state.cloned())
                        {
                            spawn(async move {
                                let res: Result<()> = (async || -> Result<_> {
                                    let sig_bytes = bs58::decode(signature.as_str()).into_vec()?;
                                    let sig_bytes: [u8; 64] =
                                        sig_bytes.try_into().map_err(|_| {
                                            anyhow::anyhow!("could not parse vec as byte array")
                                        })?;
                                    let sig = Signature::from(sig_bytes);
                                    let message_bytes = bincode::serialize(message.as_str())?;
                                    let verified = sig.verify(
                                        pubkey.to_bytes().as_slice(),
                                        message_bytes.as_slice(),
                                    );
                                    if verified {
                                        tokio::time::sleep(tokio::time::Duration::from_secs(3))
                                            .await;
                                        message_state
                                            .set(MessageState::Signed(message, sig_bytes.to_vec()));
                                    }
                                    Ok(())
                                })()
                                .await;
                                if let Err(err) = res {
                                    message_state.set(MessageState::Error(err.to_string()));
                                }
                            });
                        } else {
                            message_state
                                .set(MessageState::Error("no message available".to_string()));
                        }
                    }
                    // USB message handling
                    MsgFromKotlin::UsbDeviceList(devices_json) => {
                        log::info!("Received USB device list: {}", devices_json);
                        usb_devices_state.set(devices_json);
                        usb_status_state.set("Device list updated".to_string());
                    }
                    MsgFromKotlin::UsbPermissionResult(result) => {
                        log::info!("USB permission result: {}", result);
                        usb_status_state.set(format!("Permission: {}", result));
                    }
                    MsgFromKotlin::UsbDeviceOpened(result) => {
                        log::info!("USB device opened: {}", result);
                        usb_status_state.set(format!("Device opened: {}", result));
                    }
                    MsgFromKotlin::UsbDeviceClosed(result) => {
                        log::info!("USB device closed: {}", result);
                        usb_status_state.set(format!("Device closed: {}", result));
                    }
                    MsgFromKotlin::UsbDataWritten(result) => {
                        log::info!("USB data written: {}", result);
                        usb_status_state.set(format!("Data written: {}", result));
                    }
                    MsgFromKotlin::UsbDataRead(result) => {
                        log::info!("USB data read: {}", result);
                        usb_status_state.set(format!("Data read: {}", result));
                    }
                    MsgFromKotlin::UsbError(error) => {
                        log::error!("USB error: {}", error);
                        usb_status_state.set(format!("Error: {}", error));
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
    // get wallet state context
    let wallet_state = use_context::<Signal<WalletState>>();
    // get transaction state context
    let mut transaction_state = use_context::<Signal<TransactionState>>();
    let transaction: Resource<Result<VersionedTransaction>> = use_resource(move || async move {
        let pubkey = wallet_state.cloned();
        if let WalletState::Pubkey(pubkey) = pubkey {
            let ix = solana_sdk::system_instruction::transfer(&pubkey, &pubkey, 1_000);
            let mut tx = Transaction::new_with_payer(&[ix], Some(&pubkey));
            let client = RpcClient::new(RPC_URL.to_string());
            let hash = client.get_latest_blockhash().await?;
            tx.message.recent_blockhash = hash;
            let tx: VersionedTransaction = tx.into();
            Ok(tx)
        } else {
            Err(anyhow::anyhow!("wallet disconnected"))
        }
    });
    // get message signature state context
    let mut message_state = use_context::<Signal<MessageState>>();
    use_effect(move || {
        let pubkey = wallet_state.cloned();
        if let WalletState::Pubkey(_pubkey) = pubkey {
            let message = "hello world".to_string();
            message_state.set(MessageState::WaitingForSignature(message));
        } else {
            message_state.set(MessageState::Error("wallet disconnected".to_string()));
        }
    });
    rsx! {
        if let WalletState::Pubkey(pubkey) = wallet_state.cloned() {
            div { "{pubkey}" }
        } else {
            div { id: "hero",
                img { src: HEADER_SVG, id: "header" }
                button {
                    onclick: move |_| {
                        let string = crate::ffi::initiate_mwa_session_from_dioxus();
                        log::debug!("session string: {:?}", string);
                    },
                    "connect wallet"
                }
            }
        }
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
        div {
            button {
                onclick: move |_| {
                    if let MessageState::WaitingForSignature(message) = message_state.cloned() {
                        if let Ok(bytes) = bincode::serialize(message.as_str()) {
                            message_state.set(MessageState::WaitingForSignature(message));
                            crate::ffi::initiate_sign_message_from_dioxus(bytes.as_slice());
                        }
                    }
                },
                "sign message"
            }
        }
        div {
            match message_state.cloned() {
                MessageState::None => "no message".to_string(),
                MessageState::WaitingForSignature(_) => "waiting for signature".to_string(),
                MessageState::Signed(_, _) => "signed".to_string(),
                MessageState::Error(err) => err,
            }
        }
    }
}

#[component]
fn UsbManager() -> Element {
    let usb_devices = use_context::<Signal<String>>();
    let usb_status = use_context::<Signal<String>>();
    let mut selected_device = use_context::<Signal<String>>();
    let mut write_data = use_signal(|| String::new());
    let mut read_buffer_size = use_signal(|| "1024".to_string());

    // Parse USB devices from JSON
    let parsed_devices = use_memo(move || {
        let devices_json = usb_devices.read();
        if devices_json.is_empty() {
            return Vec::new();
        }
        
        // Simple JSON parsing - in production you'd want proper error handling
        match serde_json::from_str::<Vec<serde_json::Value>>(&*devices_json) {
            Ok(devices) => devices
                .into_iter()
                .filter_map(|device| {
                    Some((
                        device["name"].as_str()?.to_string(),
                        device["productName"].as_str().unwrap_or("Unknown").to_string(),
                        device["hasPermission"].as_bool().unwrap_or(false),
                    ))
                })
                .collect::<Vec<(String, String, bool)>>(),
            Err(e) => {
                log::error!("Failed to parse USB devices JSON: {:?}", e);
                Vec::new()
            }
        }
    });

    rsx! {
        div {
            class: "usb-manager p-6 max-w-4xl mx-auto",
            
            h1 { 
                class: "text-3xl font-bold mb-6",
                "USB Device Manager" 
            }
            
            div {
                class: "status-section mb-6 p-4 bg-gray-100 rounded",
                h2 { class: "text-xl font-semibold mb-2", "Status" }
                p { class: "text-gray-700", "{usb_status}" }
            }
            
            div {
                class: "controls-section mb-6",
                h2 { class: "text-xl font-semibold mb-4", "Device Controls" }
                
                div { class: "flex gap-4 mb-4",
                    button {
                        class: "bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded",
                        onclick: move |_| {
                            let devices = crate::ffi::get_usb_devices_from_dioxus();
                            log::info!("USB scan result: {}", devices);
                        },
                        "Scan USB Devices"
                    }
                    
                    button {
                        class: "bg-green-500 hover:bg-green-700 text-white font-bold py-2 px-4 rounded",
                        onclick: move |_| {
                            let device_name = selected_device.read().clone();
                            if !device_name.is_empty() {
                                let result = crate::ffi::request_usb_permission_from_dioxus(&device_name);
                                log::info!("Permission request result: {}", result);
                            }
                        },
                        "Request Permission"
                    }
                    
                    button {
                        class: "bg-purple-500 hover:bg-purple-700 text-white font-bold py-2 px-4 rounded",
                        onclick: move |_| {
                            let device_name = selected_device.read().clone();
                            if !device_name.is_empty() {
                                let result = crate::ffi::open_usb_device_from_dioxus(&device_name);
                                log::info!("Open device result: {}", result);
                            }
                        },
                        "Open Device"
                    }
                }
            }
            
            div {
                class: "devices-section mb-6",
                h2 { class: "text-xl font-semibold mb-4", "Connected Devices" }
                
                if parsed_devices.read().is_empty() {
                    p { class: "text-gray-500", "No devices found. Click 'Scan USB Devices' to refresh." }
                } else {
                    div { class: "grid gap-4",
                        // Fix: access the memo value properly and iterate correctly
                        for (device_name, product_name, has_permission) in parsed_devices.read().iter() {
                            div {
                                class: format!("device-card p-4 border rounded cursor-pointer {}",
                                    if selected_device.read().as_str() == device_name { "border-blue-500 bg-blue-50" } else { "border-gray-300" }),
                                onclick: {
                                    let device_name = device_name.clone();
                                    move |_| selected_device.set(device_name.clone())
                                },
                                
                                div { class: "flex justify-between items-center",
                                    div {
                                        h3 { class: "font-semibold", "{product_name}" }
                                        p { class: "text-sm text-gray-600", "Device: {device_name}" }
                                    }
                                    div {
                                        span {
                                            class: format!("px-2 py-1 rounded text-xs {}",
                                                if *has_permission { "bg-green-100 text-green-800" } else { "bg-red-100 text-red-800" }),
                                            if *has_permission { "✓ Permitted" } else { "✗ No Permission" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            
            if !selected_device.read().is_empty() {
                div {
                    class: "data-section mb-6",
                    h2 { class: "text-xl font-semibold mb-4", "Data Operations for: {selected_device}" }
                    
                    div { class: "mb-4",
                        h3 { class: "text-lg font-medium mb-2", "Write Data" }
                        div { class: "flex gap-2",
                            input {
                                class: "flex-1 px-3 py-2 border border-gray-300 rounded",
                                placeholder: "Enter data to write (hex format: 01,02,03...)",
                                value: "{write_data}",
                                oninput: move |evt| write_data.set(evt.value()),
                            }
                            button {
                                class: "bg-orange-500 hover:bg-orange-700 text-white font-bold py-2 px-4 rounded",
                                onclick: move |_| {
                                    let device_name = selected_device.read().clone();
                                    let data_str = write_data.read().clone();
                                    
                                    // Parse hex data (simple implementation)
                                    let data_bytes: Vec<u8> = data_str
                                        .split(',')
                                        .filter_map(|s| u8::from_str_radix(s.trim(), 16).ok())
                                        .collect();
                                    
                                    if !data_bytes.is_empty() {
                                        let result = crate::ffi::write_usb_data_from_dioxus(&device_name, &data_bytes);
                                        log::info!("Write data result: {}", result);
                                    } else {
                                        log::warn!("No valid hex data to write");
                                    }
                                },
                                "Write"
                            }
                        }
                        p { class: "text-sm text-gray-600 mt-1", 
                            "Format: comma-separated hex values (e.g., 01,02,03,FF)" 
                        }
                    }
                    
                    div {
                        h3 { class: "text-lg font-medium mb-2", "Read Data" }
                        div { class: "flex gap-2",
                            input {
                                class: "w-32 px-3 py-2 border border-gray-300 rounded",
                                placeholder: "Buffer size",
                                value: "{read_buffer_size}",
                                oninput: move |evt| read_buffer_size.set(evt.value()),
                            }
                            button {
                                class: "bg-indigo-500 hover:bg-indigo-700 text-white font-bold py-2 px-4 rounded",
                                onclick: move |_| {
                                    let device_name = selected_device.read().clone();
                                    let buffer_size = read_buffer_size.read().parse::<i32>().unwrap_or(1024);
                                    
                                    log::info!("Reading {} bytes from device: {}", buffer_size, device_name);
                                    // TODO: Implement read function in ffi.rs when needed
                                    // let result = crate::ffi::read_usb_data_from_dioxus(&device_name, buffer_size);
                                    // log::info!("Read data result: {}", result);
                                },
                                "Read (Not implemented yet)"
                            }
                        }
                    }
                }
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

/// USB page
#[component]
fn UsbPage() -> Element {
    rsx! {
        UsbManager {}
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
            Link { to: Route::UsbPage {}, "USB Manager" }
        }

        Outlet::<Route> {}
    }
}