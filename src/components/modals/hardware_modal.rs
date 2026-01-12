// src/components/modals/hardware_modal.rs
use dioxus::prelude::*;
use crate::hardware::{HardwareWallet, HardwareDeviceInfo, HardwareDeviceType};
#[cfg(target_os = "android")]
use crate::hardware::protocol::{Command, Response};
use std::sync::Arc;

// Define the assets for device icons - local assets
//const ICON_UNRUGGABLE: Asset = asset!("/assets/icon.png");
//const ICON_LEDGER: Asset = asset!("/assets/icons/ledgerLogo.webp");

const ICON_UNRUGGABLE: &str = "https://cdn.jsdelivr.net/gh/hogyzen12/unruggable-app@main/assets/icon.png";
const ICON_LEDGER: &str = "https://cdn.jsdelivr.net/gh/hogyzen12/unruggable-app@main/assets/icons/ledgerLogo.webp";


#[component]
pub fn HardwareWalletModal(
    onclose: EventHandler<()>,
    onsuccess: EventHandler<Arc<HardwareWallet>>,
    ondisconnect: EventHandler<()>,
    existing_wallet: Option<Arc<HardwareWallet>>,
) -> Element {
    let mut connecting = use_signal(|| false);
    let mut error_message = use_signal(|| None as Option<String>);
    let mut hardware_wallet = use_signal(|| existing_wallet.clone());
    let mut connected = use_signal(|| existing_wallet.is_some());
    let mut public_key = use_signal(|| None as Option<String>);
    let mut device_type = use_signal(|| None as Option<HardwareDeviceType>);
    let mut available_devices = use_signal(|| Vec::<HardwareDeviceInfo>::new());
    let mut scanning = use_signal(|| false);
    let mut debug_logs = use_signal(|| Vec::<String>::new());
    let mut debug_devices = use_signal(|| Vec::<HardwareDeviceInfo>::new());
    let mut debug_scanning = use_signal(|| false);
    let mut debug_connecting = use_signal(|| false);
    let mut debug_wallet = use_signal(|| None as Option<Arc<HardwareWallet>>);
    let mut debug_pubkey = use_signal(|| None as Option<String>);
    let mut show_usb_debug = use_signal(|| false);
    
    // Store if we have an existing wallet
    let has_existing_wallet = existing_wallet.is_some();
    
    // If we have an existing wallet, get its details
    use_effect(move || {
        if let Some(wallet) = &existing_wallet {
            let wallet = wallet.clone();
            spawn(async move {
                if let Ok(pubkey) = wallet.get_public_key().await {
                    public_key.set(Some(pubkey));
                    connected.set(true);
                }
                if let Some(dev_type) = wallet.get_device_type().await {
                    device_type.set(Some(dev_type));
                }
            });
        }
    });

    // Scan for available devices when modal opens
    use_effect(move || {
        if !has_existing_wallet {
            scanning.set(true);
            spawn(async move {
                let devices = HardwareWallet::scan_available_devices().await;
                available_devices.set(devices);
                scanning.set(false);
            });
        }
    });

    // Function to connect to a specific device type
    let mut connect_device = move |dev_type: HardwareDeviceType| {
        connecting.set(true);
        error_message.set(None);
        
        spawn(async move {
            let wallet = Arc::new(HardwareWallet::new());
            
            let result = match dev_type {
                HardwareDeviceType::ESP32 => wallet.connect_esp32().await,
                HardwareDeviceType::Ledger => wallet.connect_ledger().await,
            };

            match result {
                Ok(_) => {
                    match wallet.get_public_key().await {
                        Ok(pubkey) => {
                            public_key.set(Some(pubkey.clone()));
                            device_type.set(Some(dev_type));
                            hardware_wallet.set(Some(wallet.clone()));
                            connected.set(true);
                            connecting.set(false);
                            
                            // Automatically proceed after successful connection
                            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                            onsuccess.call(wallet);
                        }
                        Err(e) => {
                            error_message.set(Some(format!("Failed to get public key: {}", e)));
                            connecting.set(false);
                        }
                    }
                }
                Err(e) => {
                    error_message.set(Some(format!("Failed to connect: {}", e)));
                    connecting.set(false);
                }
            }
        });
    };

    // Function to disconnect
    let disconnect_device = move |_| {
        if let Some(wallet) = hardware_wallet() {
            spawn(async move {
                let _ = wallet.disconnect().await;
            });
        }
        hardware_wallet.set(None);
        connected.set(false);
        public_key.set(None);
        device_type.set(None);
        ondisconnect.call(());
    };

    rsx! {
        div {
            class: "modal-backdrop",
            onclick: move |_| onclose.call(()),
            
            div {
                class: "modal-content hardware-modal",
                onclick: move |e| e.stop_propagation(),
                
                div {
                    class: "modal-header",
                    h2 { class: "modal-title", "Hardware Wallet" }
                    button {
                        class: "button-standard",
                        style: "margin-right: 12px; padding: 6px 10px; border-radius: 8px; font-size: 12px; background: #2b2b2b; color: #f8fafc; border: 1px solid rgba(255,255,255,0.1);",
                        onclick: move |_| show_usb_debug.set(!show_usb_debug()),
                        if show_usb_debug() { "Hide USB Debug" } else { "USB Debug" }
                    }
                    button {
                        class: "modal-close-button",
                        onclick: move |_| onclose.call(()),
                        "×"
                    }
                }
                
                div {
                    class: "modal-body",
                    
                    // Show error if any
                    if let Some(error) = error_message() {
                        div {
                            class: "error-message",
                            div { class: "error-icon", "⚠️" }
                            div { class: "error-text", "{error}" }
                        }
                    }

                    if cfg!(target_os = "android") && show_usb_debug() {
                        div {
                            style: "margin-bottom: 20px; padding: 16px; border-radius: 16px; background: #1f1f1f; border: 1px solid rgba(255,255,255,0.08);",
                            h4 { style: "margin: 0 0 8px 0; font-size: 16px; font-weight: 700; color: #f8fafc;", "USB Debug" }
                            p { style: "margin: 0 0 12px 0; font-size: 12px; color: #9ca3af;", "Tap Scan → Connect → GET_PUBKEY. The first connect will prompt for USB permission." }

                            div {
                                style: "display: flex; flex-wrap: wrap; gap: 8px; margin-bottom: 12px;",
                                button {
                                    style: "padding: 8px 12px; border-radius: 10px; background: #2b2b2b; color: #f8fafc; border: 1px solid rgba(255,255,255,0.08);",
                                    disabled: debug_scanning(),
                                    onclick: move |_| {
                                        debug_scanning.set(true);
                                        let mut debug_logs = debug_logs.clone();
                                        let mut debug_devices = debug_devices.clone();
                                        spawn(async move {
                                            debug_logs.with_mut(|logs| {
                                                logs.push("USB scan started".to_string());
                                                if logs.len() > 8 { logs.remove(0); }
                                            });
                                            let devices = HardwareWallet::scan_available_devices().await;
                                            let count = devices.len();
                                            debug_devices.set(devices);
                                            debug_logs.with_mut(|logs| {
                                                logs.push(format!("USB scan complete: {} device(s)", count));
                                                if logs.len() > 8 { logs.remove(0); }
                                            });
                                            debug_scanning.set(false);
                                        });
                                    },
                                    if debug_scanning() { "Scanning..." } else { "Scan USB" }
                                }
                                button {
                                    style: "padding: 8px 12px; border-radius: 10px; background: #2b2b2b; color: #f8fafc; border: 1px solid rgba(255,255,255,0.08);",
                                    disabled: debug_connecting(),
                                    onclick: move |_| {
                                        debug_connecting.set(true);
                                        debug_pubkey.set(None);
                                        let mut debug_logs = debug_logs.clone();
                                        let mut debug_wallet = debug_wallet.clone();
                                        let mut debug_pubkey = debug_pubkey.clone();
                                        spawn(async move {
                                            debug_logs.with_mut(|logs| {
                                                logs.push("Connecting to ESP32 (debug)...".to_string());
                                                if logs.len() > 8 { logs.remove(0); }
                                            });
                                            let wallet = Arc::new(HardwareWallet::new());
                                            match wallet.connect_esp32().await {
                                                Ok(_) => {
                                                    match wallet.get_public_key().await {
                                                        Ok(pubkey) => {
                                                            debug_pubkey.set(Some(pubkey.clone()));
                                                            debug_wallet.set(Some(wallet));
                                                            debug_logs.with_mut(|logs| {
                                                                logs.push(format!("Connected: {}", pubkey));
                                                                if logs.len() > 8 { logs.remove(0); }
                                                            });
                                                        }
                                                        Err(e) => {
                                                            debug_logs.with_mut(|logs| {
                                                                logs.push(format!("Connected, but pubkey failed: {}", e));
                                                                if logs.len() > 8 { logs.remove(0); }
                                                            });
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    debug_logs.with_mut(|logs| {
                                                        logs.push(format!("Connect failed: {}", e));
                                                        if logs.len() > 8 { logs.remove(0); }
                                                    });
                                                }
                                            }
                                            debug_connecting.set(false);
                                        });
                                    },
                                    if debug_connecting() { "Connecting..." } else { "Connect ESP32" }
                                }
                                button {
                                    style: "padding: 8px 12px; border-radius: 10px; background: #2b2b2b; color: #f8fafc; border: 1px solid rgba(255,255,255,0.08);",
                                    onclick: move |_| {
                                        let wallet = debug_wallet();
                                        let mut debug_logs = debug_logs.clone();
                                        let mut debug_pubkey = debug_pubkey.clone();
                                        spawn(async move {
                                            match wallet {
                                                Some(w) => {
                                                    match w.send_command(Command::GetPubkey).await {
                                                        Ok(Response::Pubkey(pubkey)) => {
                                                            debug_pubkey.set(Some(pubkey.clone()));
                                                            debug_logs.with_mut(|logs| {
                                                                logs.push(format!("GET_PUBKEY: {}", pubkey));
                                                                if logs.len() > 8 { logs.remove(0); }
                                                            });
                                                        }
                                                        Ok(Response::Error(err)) => {
                                                            debug_logs.with_mut(|logs| {
                                                                logs.push(format!("GET_PUBKEY error: {}", err));
                                                                if logs.len() > 8 { logs.remove(0); }
                                                            });
                                                        }
                                                        Ok(other) => {
                                                            debug_logs.with_mut(|logs| {
                                                                logs.push(format!("Unexpected response: {:?}", other));
                                                                if logs.len() > 8 { logs.remove(0); }
                                                            });
                                                        }
                                                        Err(e) => {
                                                            debug_logs.with_mut(|logs| {
                                                                logs.push(format!("GET_PUBKEY failed: {}", e));
                                                                if logs.len() > 8 { logs.remove(0); }
                                                            });
                                                        }
                                                    }
                                                }
                                                None => {
                                                    debug_logs.with_mut(|logs| {
                                                        logs.push("No debug connection active".to_string());
                                                        if logs.len() > 8 { logs.remove(0); }
                                                    });
                                                }
                                            }
                                        });
                                    },
                                    "GET_PUBKEY"
                                }
                                button {
                                    style: "padding: 8px 12px; border-radius: 10px; background: #2b2b2b; color: #f8fafc; border: 1px solid rgba(255,255,255,0.08);",
                                    onclick: move |_| {
                                        let wallet = debug_wallet();
                                        let mut debug_logs = debug_logs.clone();
                                        spawn(async move {
                                            if let Some(w) = wallet {
                                                let _ = w.disconnect().await;
                                            }
                                            debug_logs.with_mut(|logs| {
                                                logs.push("Disconnected debug session".to_string());
                                                if logs.len() > 8 { logs.remove(0); }
                                            });
                                        });
                                        debug_wallet.set(None);
                                        debug_pubkey.set(None);
                                    },
                                    "Disconnect"
                                }
                            }

                            div { style: "font-size: 12px; color: #9ca3af; margin-bottom: 8px;", "Devices found: {debug_devices().len()}" }
                            if let Some(pubkey) = debug_pubkey() {
                                div { style: "font-size: 12px; color: #f8fafc; margin-bottom: 8px;", "Last pubkey: {pubkey}" }
                            }

                            div {
                                style: "max-height: 140px; overflow: auto; font-size: 12px; background: #141414; border-radius: 10px; padding: 8px; color: #d1d5db;",
                                if debug_logs().is_empty() {
                                    div { "No debug logs yet." }
                                } else {
                                    for line in debug_logs() {
                                        div { "{line}" }
                                    }
                                }
                            }
                        }
                    }

                    if !connected() {
                        div {
                            class: "connection-section",
                            
                            div {
                                class: "info-header",
                                h3 { "Connect Your Hardware Wallet" }
                                p { class: "info-subtitle", "Secure your transactions with hardware-based signing" }
                            }

                            // Device scanning status
                            if scanning() {
                                div {
                                    class: "scanning-container",
                                    div { class: "scanning-spinner" }
                                    div { class: "scanning-text", "Scanning for devices..." }
                                }
                            } else {
                                // Show available devices or empty state
                                if available_devices().is_empty() {
                                    div {
                                        class: "no-devices-container",
                                        div { class: "no-devices-icon", "🔍" }
                                        div { class: "no-devices-title", "No Hardware Wallets Detected" }
                                        div { class: "no-devices-subtitle", "Please connect your device and ensure:" }
                                        ul {
                                            class: "device-requirements",
                                            li { 
                                                strong { "Unruggable: " }
                                                "Device is connected via USB with proper drivers installed"
                                            }
                                            li { 
                                                strong { "Ledger: " }
                                                "Device is unlocked, Solana app is open, and Ledger Live is closed"
                                            }
                                        }
                                    }
                                } else {
                                    div {
                                        class: "devices-section",
                                        h4 { class: "devices-title", "Available Devices" }
                                        
                                        div {
                                            class: "devices-grid",
                                            for device in available_devices() {
                                                div {
                                                    class: "device-card",
                                                    div {
                                                        class: "device-icon-container",
                                                        div {
                                                            class: if device.device_type == HardwareDeviceType::ESP32 {
                                                                "device-icon device-icon-unruggable"
                                                            } else {
                                                                "device-icon device-icon-ledger"
                                                            },
                                                            // Device logo images
                                                            img {
                                                                src: if device.device_type == HardwareDeviceType::ESP32 {
                                                                    ICON_UNRUGGABLE
                                                                } else {
                                                                    ICON_LEDGER
                                                                },
                                                                alt: if device.device_type == HardwareDeviceType::ESP32 {
                                                                    "Unruggable Hardware Wallet"
                                                                } else {
                                                                    "Ledger Hardware Wallet"
                                                                },
                                                                width: "48",
                                                                height: "48"
                                                            }
                                                        }
                                                    }
                                                    
                                                    div {
                                                        class: "device-info",
                                                        div { class: "device-name", "{device.name}" }
                                                        div { 
                                                            class: if device.device_type == HardwareDeviceType::ESP32 {
                                                                "device-type-badge unruggable-badge"
                                                            } else {
                                                                "device-type-badge ledger-badge"
                                                            },
                                                            if device.device_type == HardwareDeviceType::ESP32 {
                                                                "Unruggable Wallet"
                                                            } else {
                                                                "Ledger Wallet"
                                                            }
                                                        }
                                                    }
                                                    
                                                    button {
                                                        class: if connecting() {
                                                            "connect-device-button connecting"
                                                        } else {
                                                            "connect-device-button"
                                                        },
                                                        disabled: connecting(),
                                                        onclick: {
                                                            let dev_type = device.device_type.clone();
                                                            move |_| connect_device(dev_type.clone())
                                                        },
                                                        if connecting() {
                                                            div { class: "button-spinner" }
                                                            span { "Connecting..." }
                                                        } else {
                                                            span { "Connect" }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        
                    } else {
                        // Connected state - show wallet info and options
                        div {
                            class: "connected-section",
                            
                            div {
                                class: "success-header",
                                div { class: "success-icon", "✅" }
                                h3 { "Hardware Wallet Connected" }
                            }
                            
                            if let Some(dev_type) = device_type() {
                                div {
                                    class: "connected-device-card",
                                    div {
                                        class: "connected-device-icon",
                                        div {
                                            class: if dev_type == HardwareDeviceType::ESP32 {
                                                "device-icon-large device-icon-unruggable"
                                            } else {
                                                "device-icon-large device-icon-ledger"
                                            },
                                            // Larger device logo images for connected state
                                            img {
                                                src: if dev_type == HardwareDeviceType::ESP32 {
                                                    ICON_UNRUGGABLE
                                                } else {
                                                    ICON_LEDGER
                                                },
                                                alt: if dev_type == HardwareDeviceType::ESP32 {
                                                    "Unruggable Hardware Wallet"
                                                } else {
                                                    "Ledger Hardware Wallet"
                                                },
                                                width: "64",
                                                height: "64"
                                            }
                                        }
                                    }
                                    
                                    div {
                                        class: "connected-device-info",
                                        h4 { class: "connected-device-name", "{dev_type}" }
                                        if let Some(pubkey) = public_key() {
                                            div {
                                                class: "device-pubkey-section",
                                                div { class: "pubkey-label", "Public Key:" }
                                                div { 
                                                    class: "pubkey-display",
                                                    onclick: move |_| {
                                                        // Copy to clipboard functionality could be added here
                                                        log::info!("Public key copied: {}", pubkey);
                                                    },
                                                    span { class: "pubkey-text", "{pubkey}" }
                                                    div { class: "copy-hint", "Click to copy" }
                                                }
                                            }
                                        }
                                        
                                        div {
                                            class: "connection-status",
                                            div { class: "status-indicator connected" }
                                            span { "Securely Connected" }
                                        }
                                    }
                                }
                            }
                        }
                        
                        div { 
                            class: "connected-modal-actions",
                            button {
                                class: "connect-device-button",
                                onclick: disconnect_device,
                                div { class: "disconnect-icon", "🔌" }
                                span { "Disconnect Device" }
                            }
                        }
                    }

                }
            }
        }
    }
}
