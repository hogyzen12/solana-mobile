use crate::wallet::{Wallet, WalletInfo};
use crate::quantum_vault::StoredVault;
use serde::{Deserialize, Serialize};
use std::path::Path;

// Android-specific imports
#[cfg(target_os = "android")]
use std::path::PathBuf;

// Custom error type that implements Send
#[derive(Debug, Clone)]
pub struct StorageError(String);

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for StorageError {}

impl From<String> for StorageError {
    fn from(s: String) -> Self {
        StorageError(s)
    }
}

impl From<&str> for StorageError {
    fn from(s: &str) -> Self {
        StorageError(s.to_string())
    }
}

#[cfg(target_os = "android")]
impl From<jni::errors::Error> for StorageError {
    fn from(e: jni::errors::Error) -> Self {
        StorageError(format!("JNI Error: {}", e))
    }
}

// Android-specific function to get the proper files directory
#[cfg(target_os = "android")]
fn get_android_files_dir() -> Result<String, StorageError> {
    use dioxus::mobile::wry::prelude::dispatch;
    use jni::objects::{JObject, JString};
    use jni::JNIEnv;
    
    let (tx, rx) = std::sync::mpsc::channel();

    fn run(env: &mut JNIEnv<'_>, activity: &JObject<'_>) -> Result<String, StorageError> {
        // Get the files directory (internal storage)
        let files_dir = env
            .call_method(activity, "getFilesDir", "()Ljava/io/File;", &[])?
            .l()?;
        
        // Get the absolute path
        let files_dir_path: JString<'_> = env
            .call_method(files_dir, "getAbsolutePath", "()Ljava/lang/String;", &[])?
            .l()?
            .into();
        
        // Convert to Rust string
        let files_dir_str: String = env.get_string(&files_dir_path)?.into();
        
        Ok(files_dir_str)
    }

    dispatch(move |env, activity, _webview| {
        let result = run(env, activity);
        tx.send(result).unwrap();
    });

    match rx.recv() {
        Ok(result) => result,
        Err(e) => Err(StorageError::from(format!("Channel receive error: {}", e))),
    }
}

// Use OnceLock instead of lazy_static for Android
#[cfg(target_os = "android")]
fn get_android_files_dir_cached() -> &'static Option<String> {
    use std::sync::OnceLock;
    static ANDROID_FILES_DIR: OnceLock<Option<String>> = OnceLock::new();
    ANDROID_FILES_DIR.get_or_init(|| {
        match get_android_files_dir() {
            Ok(dir) => {
                log::info!("✅ Android files directory initialized: {}", dir);
                Some(dir)
            }
            Err(e) => {
                log::error!("❌ Failed to initialize Android files directory: {}", e);
                None
            }
        }
    })
}

// Use lazy_static only on non-Android platforms
#[cfg(not(target_os = "android"))]
lazy_static::lazy_static! {
    static ref ANDROID_FILES_DIR: Option<String> = None;
}

// Get the appropriate storage directory for the current platform
fn get_storage_dir() -> String {
    #[cfg(target_os = "android")]
    {
        match get_android_files_dir() {
            Ok(dir) => {
                log::info!("✅ Using Android files directory: {}", dir);
                dir
            }
            Err(e) => {
                log::error!("❌ Failed to get Android files directory: {}", e);
                log::warn!("⚠️ Falling back to current directory");
                ".".to_string()
            }
        }
    }
    #[cfg(not(target_os = "android"))]
    {
        let home_dir = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| ".".to_string());
        format!("{home_dir}/.solana_wallet_app")
    }
}

fn get_storage_dir_simple() -> String {
    #[cfg(target_os = "android")]
    {
        if let Some(ref dir) = *get_android_files_dir_cached() {
            dir.clone()
        } else {
            log::warn!("⚠️ Using fallback storage directory");
            "/data/data/com.mobile/files".to_string() // Hardcoded fallback
        }
    }
    #[cfg(target_os = "ios")]
    {
        // Use iOS Application Support directory (better than Documents for app data)
        if let Some(home) = std::env::var_os("HOME") {
            let app_support = std::path::PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("WalletData");
            
            let app_support_str = app_support.to_string_lossy().to_string();
            log::info!("🍎 Using iOS Application Support: {}", app_support_str);
            app_support_str
        } else {
            log::warn!("⚠️ iOS HOME not found, using fallback");
            "./WalletData".to_string()
        }
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let home_dir = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| ".".to_string());
        format!("{home_dir}/.solana_wallet_app")
    }
}

// Add iOS-specific initialization function (add this new function)
#[cfg(target_os = "ios")]
pub fn init_ios_storage() -> Result<(), String> {
    log::info!("🍎 Initializing iOS storage...");
    
    // Log environment info for debugging
    if let Some(home) = std::env::var_os("HOME") {
        log::info!("📱 iOS HOME: {}", home.to_string_lossy());
    } else {
        log::warn!("⚠️ iOS HOME environment variable not found");
    }
    
    // Get and create storage directory
    let storage_dir = get_storage_dir_simple();
    log::info!("📁 iOS storage directory: {}", storage_dir);
    
    // Ensure directory exists
    match ensure_storage_dir() {
        Ok(_) => {
            log::info!("✅ iOS storage directory ready");
            
            // Test read/write capabilities
            let test_file = format!("{}/ios_test.txt", storage_dir);
            match std::fs::write(&test_file, "iOS storage test") {
                Ok(_) => {
                    log::info!("✅ iOS write test successful");
                    
                    // Verify we can read it back
                    match std::fs::read_to_string(&test_file) {
                        Ok(content) => {
                            if content == "iOS storage test" {
                                log::info!("✅ iOS read-write verification successful");
                                let _ = std::fs::remove_file(&test_file); // cleanup
                                Ok(())
                            } else {
                                Err("iOS read-write verification failed".to_string())
                            }
                        }
                        Err(e) => Err(format!("iOS read test failed: {}", e))
                    }
                }
                Err(e) => {
                    Err(format!("iOS write test failed: {}", e))
                }
            }
        }
        Err(e) => {
            Err(format!("iOS storage directory creation failed: {}", e))
        }
    }
}

// Get file paths
fn get_wallets_file_path() -> String {
    let storage_dir = get_storage_dir_simple();
    format!("{storage_dir}/wallets.json")
}

fn get_rpc_file_path() -> String {
    let storage_dir = get_storage_dir_simple();
    format!("{storage_dir}/rpc.txt")
}

fn get_jito_settings_file_path() -> String {
    let storage_dir = get_storage_dir_simple();
    format!("{storage_dir}/jito_settings.json")
}

fn get_bridge_settings_file_path() -> String {
    let storage_dir = get_storage_dir_simple();
    format!("{storage_dir}/bridge_settings.json")
}

fn get_quantum_vaults_file_path() -> String {
    let storage_dir = get_storage_dir_simple();
    format!("{storage_dir}/quantum_vaults.json")
}

// Ensure storage directory exists with logging
fn ensure_storage_dir() -> Result<(), std::io::Error> {
    let storage_dir = get_storage_dir_simple();
    log::info!("Ensuring storage directory exists: {}", storage_dir);
    
    match std::fs::create_dir_all(&storage_dir) {
        Ok(_) => {
            log::info!("✅ Storage directory created/verified: {}", storage_dir);
            
            // Verify permissions by writing a test file
            let test_file = format!("{}/permission_test.txt", storage_dir);
            match std::fs::write(&test_file, "permission_test") {
                Ok(_) => {
                    log::info!("✅ Storage directory is writable");
                    let _ = std::fs::remove_file(&test_file);
                    Ok(())
                }
                Err(e) => {
                    log::error!("❌ Storage directory exists but is not writable: {}", e);
                    Err(e)
                }
            }
        }
        Err(e) => {
            log::error!("❌ Failed to create storage directory {}: {}", storage_dir, e);
            Err(e)
        }
    }
}

// Add this function for testing Android storage
#[cfg(target_os = "android")]
pub fn ensure_android_storage_works() -> Result<(), String> {
    log::info!("🔧 Testing Android storage...");
    
    // Try to write a simple test file
    let test_dir = "/data/data/com.mobile/files";
    
    match std::fs::create_dir_all(test_dir) {
        Ok(_) => log::info!("✅ Created storage directory: {}", test_dir),
        Err(e) => {
            log::error!("❌ Failed to create storage directory: {}", e);
            return Err(format!("Storage directory creation failed: {}", e));
        }
    }
    
    let test_file = format!("{}/test.txt", test_dir);
    match std::fs::write(&test_file, "test") {
        Ok(_) => {
            log::info!("✅ Storage write test successful");
            let _ = std::fs::remove_file(&test_file);
            Ok(())
        }
        Err(e) => {
            log::error!("❌ Storage write test failed: {}", e);
            Err(format!("Storage write failed: {}", e))
        }
    }
}

pub fn save_wallet_to_storage(wallet_info: &WalletInfo) {
    log::info!("🔄 Attempting to save wallet: {}", wallet_info.name);
    
    let mut wallets = load_wallets_from_storage();
    wallets.push(wallet_info.clone());
    
    #[cfg(feature = "web")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        let serialized = serde_json::to_string(&wallets).unwrap();
        storage.set_item("wallets", &serialized).unwrap();
        log::info!("✅ Wallet saved to web storage");
    }
    
    #[cfg(not(feature = "web"))]
    {
        match ensure_storage_dir() {
            Ok(_) => {
                let wallet_file = get_wallets_file_path();
                log::info!("📁 Saving to file: {}", wallet_file);
                
                match serde_json::to_string_pretty(&wallets) {
                    Ok(serialized) => {
                        match std::fs::write(&wallet_file, &serialized) {
                            Ok(_) => {
                                log::info!("✅ Wallet successfully saved to: {}", wallet_file);
                                log::info!("📊 Saved {} wallets total", wallets.len());
                                
                                // Verify the save by reading it back
                                match std::fs::read_to_string(&wallet_file) {
                                    Ok(read_back) => {
                                        if read_back == serialized {
                                            log::info!("✅ Write verification successful");
                                        } else {
                                            log::error!("❌ Write verification failed - content mismatch");
                                        }
                                    }
                                    Err(e) => {
                                        log::error!("❌ Write verification failed - cannot read back: {}", e);
                                    }
                                }
                            }
                            Err(e) => {
                                log::error!("❌ Failed to write wallets to {}: {}", wallet_file, e);
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("❌ Failed to serialize wallets: {}", e);
                    }
                }
            }
            Err(e) => {
                log::error!("❌ Failed to ensure storage directory: {}", e);
            }
        }
    }
}

pub fn load_wallets_from_storage() -> Vec<WalletInfo> {
    log::info!("🔄 Attempting to load wallets from storage");
    
    // iOS-specific initialization
    #[cfg(target_os = "ios")]
    {
        if let Err(e) = init_ios_storage() {
            log::error!("❌ iOS storage init failed: {}", e);
        }
    }
    
    #[cfg(feature = "web")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        let result = storage.get_item("wallets")
            .unwrap()
            .and_then(|data| serde_json::from_str(&data).ok())
            .unwrap_or_default();
        log::info!("📱 Loaded {} wallets from web storage", result.len());
        result
    }
    
    #[cfg(not(feature = "web"))]
    {
        let wallet_file = get_wallets_file_path();
        log::info!("📁 Looking for wallets at: {}", wallet_file);
        
        // Ensure storage directory exists
        if let Err(e) = ensure_storage_dir() {
            log::error!("❌ Storage directory error: {}", e);
            return Vec::new();
        }
        
        // Check if file exists
        if !Path::new(&wallet_file).exists() {
            log::info!("ℹ️ No existing wallet file found at: {}", wallet_file);
            
            // Debug: List directory contents
            let storage_dir = get_storage_dir_simple();
            if let Ok(entries) = std::fs::read_dir(&storage_dir) {
                log::info!("📂 Directory contents of {}:", storage_dir);
                for entry in entries {
                    if let Ok(entry) = entry {
                        log::info!("  - {}", entry.file_name().to_string_lossy());
                    }
                }
            }
            
            return Vec::new();
        }
        
        match std::fs::read_to_string(&wallet_file) {
            Ok(data) => {
                log::info!("📄 Read {} bytes from wallet file", data.len());
                match serde_json::from_str::<Vec<WalletInfo>>(&data) {
                    Ok(wallets) => {
                        log::info!("✅ Successfully loaded {} wallets", wallets.len());
                        for (i, wallet) in wallets.iter().enumerate() {
                            log::info!("  Wallet {}: {} ({}...)", i + 1, wallet.name, &wallet.address[..8]);
                        }
                        wallets
                    }
                    Err(e) => {
                        log::error!("❌ Failed to parse wallets from {}: {}", wallet_file, e);
                        log::error!("📄 File contents preview: {}", &data.chars().take(200).collect::<String>());
                        Vec::new()
                    }
                }
            }
            Err(e) => {
                log::error!("❌ Failed to read wallets from {}: {}", wallet_file, e);
                Vec::new()
            }
        }
    }
}

pub fn import_wallet_from_key(private_key: &str, name: String) -> Result<WalletInfo, String> {
    let private_key = private_key.trim();
    
    // Try to parse the key based on format
    let key_bytes = if private_key.starts_with('[') && private_key.ends_with(']') {
        // JSON array format: [252,183,...159,189]
        parse_json_array_key(private_key)?
    } else if private_key.contains(',') {
        // Comma-separated format: 252,183,...159,189
        parse_comma_separated_key(private_key)?
    } else {
        // Base58 format (original)
        bs58::decode(private_key)
            .into_vec()
            .map_err(|e| format!("Invalid base58 format: {}", e))?
    };
    
    let wallet_name = if name.is_empty() { 
        "Imported Wallet".to_string() 
    } else { 
        name 
    };
    
    let wallet = Wallet::from_private_key(&key_bytes, wallet_name)?;
    
    Ok(wallet.to_wallet_info())
}

// Helper function to parse JSON array format
fn parse_json_array_key(key_str: &str) -> Result<Vec<u8>, String> {
    serde_json::from_str::<Vec<u8>>(key_str)
        .map_err(|e| format!("Invalid JSON array format: {}", e))
}

// Helper function to parse comma-separated format
fn parse_comma_separated_key(key_str: &str) -> Result<Vec<u8>, String> {
    key_str
        .split(',')
        .map(|s| {
            s.trim()
                .parse::<u8>()
                .map_err(|e| format!("Invalid number in key: {}", e))
        })
        .collect::<Result<Vec<u8>, String>>()
}

// Optional: Add a validation function to check key format before import
pub fn validate_key_format(private_key: &str) -> Result<String, String> {
    let private_key = private_key.trim();
    
    if private_key.is_empty() {
        return Err("Private key is empty".to_string());
    }
    
    if private_key.starts_with('[') && private_key.ends_with(']') {
        return Ok("JSON array format".to_string());
    } else if private_key.contains(',') {
        return Ok("Comma-separated format".to_string());
    } else {
        // Check if it's valid base58
        bs58::decode(private_key)
            .into_vec()
            .map_err(|e| format!("Invalid base58 format: {}", e))?;
        return Ok("Base58 format".to_string());
    }
}

pub fn save_rpc_to_storage(rpc_url: &str) {
    log::info!("🔄 Saving RPC URL to storage");
    
    #[cfg(feature = "web")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        storage.set_item("custom_rpc", rpc_url).unwrap();
    }
    
    #[cfg(not(feature = "web"))]
    {
        if let Ok(_) = ensure_storage_dir() {
            let rpc_file = get_rpc_file_path();
            match std::fs::write(&rpc_file, rpc_url) {
                Ok(_) => log::info!("✅ RPC URL saved to: {}", rpc_file),
                Err(e) => log::error!("❌ Failed to write RPC to {}: {}", rpc_file, e),
            }
        }
    }
}

pub fn load_rpc_from_storage() -> Option<String> {
    log::info!("🔄 Loading RPC URL from storage");
    
    #[cfg(feature = "web")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        storage.get_item("custom_rpc").unwrap()
    }
    
    #[cfg(not(feature = "web"))]
    {
        let rpc_file = get_rpc_file_path();
        match std::fs::read_to_string(&rpc_file) {
            Ok(data) => {
                let result = Some(data.trim().to_string());
                log::info!("✅ RPC URL loaded from storage");
                result
            }
            Err(e) => {
                if e.kind() != std::io::ErrorKind::NotFound {
                    log::error!("❌ Failed to read RPC from {}: {}", rpc_file, e);
                }
                None
            }
        }
    }
}

pub fn clear_rpc_storage() {
    #[cfg(feature = "web")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        storage.remove_item("custom_rpc").unwrap();
    }
    
    #[cfg(not(target_os = "android"))]
    {
        let rpc_file = get_rpc_file_path();
        match std::fs::remove_file(&rpc_file) {
            Ok(_) => log::info!("✅ RPC file removed"),
            Err(e) => {
                if e.kind() != std::io::ErrorKind::NotFound {
                    log::error!("❌ Failed to remove RPC file {}: {}", rpc_file, e);
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct JitoSettings {
    pub jito_tx: bool,
    pub jito_bundles: bool,
}

impl Default for JitoSettings {
    fn default() -> Self {
        Self {
            jito_tx: true,
            jito_bundles: false,
        }
    }
}

pub fn save_jito_settings_to_storage(settings: &JitoSettings) {
    log::info!("🔄 Saving Jito settings to storage");
    
    #[cfg(feature = "web")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        let serialized = serde_json::to_string(settings).unwrap();
        storage.set_item("jito_settings", &serialized).unwrap();
    }
    
    #[cfg(not(feature = "web"))]
    {
        if let Ok(_) = ensure_storage_dir() {
            let jito_file = get_jito_settings_file_path();
            match serde_json::to_string_pretty(settings) {
                Ok(serialized) => {
                    match std::fs::write(&jito_file, serialized) {
                        Ok(_) => log::info!("✅ Jito settings saved to: {}", jito_file),
                        Err(e) => log::error!("❌ Failed to write Jito settings to {}: {}", jito_file, e),
                    }
                }
                Err(e) => log::error!("❌ Failed to serialize Jito settings: {}", e),
            }
        }
    }
}

pub fn load_jito_settings_from_storage() -> JitoSettings {
    log::info!("🔄 Loading Jito settings from storage");
    
    #[cfg(feature = "web")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        storage
            .get_item("jito_settings")
            .unwrap()
            .and_then(|data| serde_json::from_str(&data).ok())
            .unwrap_or_default()
    }
    
    #[cfg(not(feature = "web"))]
    {
        let jito_file = get_jito_settings_file_path();
        match std::fs::read_to_string(&jito_file) {
            Ok(data) => {
                match serde_json::from_str(&data) {
                    Ok(settings) => {
                        log::info!("✅ Jito settings loaded from storage");
                        settings
                    }
                    Err(e) => {
                        log::error!("❌ Failed to parse Jito settings from {}: {}", jito_file, e);
                        JitoSettings::default()
                    }
                }
            }
            Err(e) => {
                if e.kind() != std::io::ErrorKind::NotFound {
                    log::error!("❌ Failed to read Jito settings from {}: {}", jito_file, e);
                }
                JitoSettings::default()
            }
        }
    }
}

pub fn get_current_jito_settings() -> JitoSettings {
    load_jito_settings_from_storage()
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct BridgeSettings {
    pub enabled: bool,
}

impl Default for BridgeSettings {
    fn default() -> Self {
        Self { enabled: false }
    }
}

pub fn save_bridge_settings_to_storage(settings: &BridgeSettings) {
    log::info!("🔄 Saving bridge settings to storage");

    #[cfg(feature = "web")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        let serialized = serde_json::to_string(settings).unwrap();
        storage.set_item("bridge_settings", &serialized).unwrap();
    }

    #[cfg(not(feature = "web"))]
    {
        if let Ok(_) = ensure_storage_dir() {
            let settings_file = get_bridge_settings_file_path();
            match serde_json::to_string_pretty(settings) {
                Ok(serialized) => {
                    match std::fs::write(&settings_file, serialized) {
                        Ok(_) => log::info!("✅ Bridge settings saved to: {}", settings_file),
                        Err(e) => log::error!("❌ Failed to write bridge settings to {}: {}", settings_file, e),
                    }
                }
                Err(e) => log::error!("❌ Failed to serialize bridge settings: {}", e),
            }
        }
    }
}

pub fn load_bridge_settings_from_storage() -> BridgeSettings {
    log::info!("🔄 Loading bridge settings from storage");

    #[cfg(feature = "web")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        storage
            .get_item("bridge_settings")
            .unwrap()
            .and_then(|data| serde_json::from_str(&data).ok())
            .unwrap_or_default()
    }

    #[cfg(not(feature = "web"))]
    {
        let settings_file = get_bridge_settings_file_path();
        match std::fs::read_to_string(&settings_file) {
            Ok(data) => {
                match serde_json::from_str(&data) {
                    Ok(settings) => {
                        log::info!("✅ Bridge settings loaded from storage");
                        settings
                    }
                    Err(e) => {
                        log::error!("❌ Failed to parse bridge settings from {}: {}", settings_file, e);
                        BridgeSettings::default()
                    }
                }
            }
            Err(e) => {
                if e.kind() != std::io::ErrorKind::NotFound {
                    log::error!("❌ Failed to read bridge settings from {}: {}", settings_file, e);
                }
                BridgeSettings::default()
            }
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Quantum Vault Storage Functions
// ══════════════════════════════════════════════════════════════════════════════

/// Save a quantum vault to storage
pub fn save_quantum_vault_to_storage(vault: &StoredVault) {
    log::info!("🔐 Attempting to save quantum vault: {}", vault.name);

    let mut vaults = load_quantum_vaults_from_storage();
    vaults.push(vault.clone());

    #[cfg(feature = "web")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        let serialized = serde_json::to_string(&vaults).unwrap();
        storage.set_item("quantum_vaults", &serialized).unwrap();
        log::info!("✅ Quantum vault saved to web storage");
    }

    #[cfg(not(feature = "web"))]
    {
        match ensure_storage_dir() {
            Ok(_) => {
                let vault_file = get_quantum_vaults_file_path();
                match serde_json::to_string_pretty(&vaults) {
                    Ok(serialized) => {
                        match std::fs::write(&vault_file, &serialized) {
                            Ok(_) => {
                                log::info!("✅ Quantum vault successfully saved to: {}", vault_file);
                                log::info!("📊 Saved {} quantum vaults total", vaults.len());
                            }
                            Err(e) => {
                                log::error!("❌ Failed to write quantum vaults to {}: {}", vault_file, e);
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("❌ Failed to serialize quantum vaults: {}", e);
                    }
                }
            }
            Err(e) => {
                log::error!("❌ Failed to ensure storage directory: {}", e);
            }
        }
    }
}

/// Load all quantum vaults from storage
pub fn load_quantum_vaults_from_storage() -> Vec<StoredVault> {
    log::info!("🔐 Attempting to load quantum vaults from storage");

    #[cfg(feature = "web")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        let result = storage.get_item("quantum_vaults")
            .unwrap()
            .and_then(|data| serde_json::from_str(&data).ok())
            .unwrap_or_default();
        log::info!("📱 Loaded {} quantum vaults from web storage", result.len());
        result
    }

    #[cfg(not(feature = "web"))]
    {
        let vault_file = get_quantum_vaults_file_path();

        if let Err(e) = ensure_storage_dir() {
            log::error!("❌ Storage directory error: {}", e);
            return Vec::new();
        }

        if !Path::new(&vault_file).exists() {
            log::info!("ℹ️ No existing quantum vault file found");
            return Vec::new();
        }

        match std::fs::read_to_string(&vault_file) {
            Ok(data) => {
                match serde_json::from_str::<Vec<StoredVault>>(&data) {
                    Ok(vaults) => vaults,
                    Err(e) => {
                        log::error!("❌ Failed to parse quantum vaults: {}", e);
                        Vec::new()
                    }
                }
            }
            Err(e) => {
                log::error!("❌ Failed to read quantum vaults: {}", e);
                Vec::new()
            }
        }
    }
}

/// Mark a quantum vault as used after splitting
pub fn mark_quantum_vault_as_used(vault_address: &str) {
    log::info!("🔐 Marking quantum vault as used: {}", vault_address);

    let mut vaults = load_quantum_vaults_from_storage();

    if let Some(vault) = vaults.iter_mut().find(|v| v.address == vault_address) {
        vault.used = true;
        save_quantum_vaults_to_storage(&vaults);
        log::info!("✅ Quantum vault marked as used");
    } else {
        log::warn!("⚠️ Quantum vault not found: {}", vault_address);
    }
}

/// Delete a quantum vault from storage
pub fn delete_quantum_vault_from_storage(vault_address: &str) {
    log::info!("🔐 Attempting to delete quantum vault: {}", vault_address);

    let mut vaults = load_quantum_vaults_from_storage();
    let original_count = vaults.len();

    vaults.retain(|vault| vault.address != vault_address);

    if vaults.len() < original_count {
        log::info!("✅ Quantum vault {} removed from memory", vault_address);
        save_quantum_vaults_to_storage(&vaults);
        log::info!("✅ Quantum vault deletion completed. {} vaults remaining.", vaults.len());
    } else {
        log::warn!("⚠️ Quantum vault {} not found in storage", vault_address);
    }
}

/// Save quantum vaults list to storage
pub fn save_quantum_vaults_to_storage(vaults: &Vec<StoredVault>) {
    log::info!("🔐 Saving {} quantum vaults to storage", vaults.len());

    #[cfg(feature = "web")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        let serialized = serde_json::to_string(vaults).unwrap();
        storage.set_item("quantum_vaults", &serialized).unwrap();
        log::info!("✅ Quantum vaults saved to web storage");
    }

    #[cfg(not(feature = "web"))]
    {
        match ensure_storage_dir() {
            Ok(_) => {
                let vault_file = get_quantum_vaults_file_path();
                match serde_json::to_string_pretty(vaults) {
                    Ok(serialized) => {
                        match std::fs::write(&vault_file, &serialized) {
                            Ok(_) => {
                                log::info!("✅ Quantum vaults successfully saved to: {}", vault_file);
                            }
                            Err(e) => {
                                log::error!("❌ Failed to write quantum vaults to {}: {}", vault_file, e);
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("❌ Failed to serialize quantum vaults: {}", e);
                    }
                }
            }
            Err(e) => {
                log::error!("❌ Failed to ensure storage directory: {}", e);
            }
        }
    }
}

/// Delete a wallet by address from storage
pub fn delete_wallet_from_storage(wallet_address: &str) {
    log::info!("🔄 Attempting to delete wallet: {}", wallet_address);
    
    let mut wallets = load_wallets_from_storage();
    let original_count = wallets.len();
    
    // Remove wallet with matching address
    wallets.retain(|wallet| wallet.address != wallet_address);
    
    if wallets.len() < original_count {
        log::info!("✅ Wallet {} removed from memory", wallet_address);
        
        // Save updated wallet list
        save_wallets_to_storage(&wallets);
        log::info!("✅ Wallet deletion completed. {} wallets remaining.", wallets.len());
    } else {
        log::warn!("⚠️ Wallet {} not found in storage", wallet_address);
    }
}

/// Save wallets list to storage (only add this if it doesn't already exist in your storage.rs)
pub fn save_wallets_to_storage(wallets: &Vec<WalletInfo>) {
    log::info!("🔄 Saving {} wallets to storage", wallets.len());
    
    #[cfg(feature = "web")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        let serialized = serde_json::to_string(wallets).unwrap();
        storage.set_item("wallets", &serialized).unwrap();
        log::info!("✅ Wallets saved to web storage");
    }
    
    #[cfg(not(feature = "web"))]
    {
        match ensure_storage_dir() {
            Ok(_) => {
                let wallet_file = get_wallets_file_path();
                match serde_json::to_string_pretty(wallets) {
                    Ok(serialized) => {
                        match std::fs::write(&wallet_file, &serialized) {
                            Ok(_) => {
                                log::info!("✅ Wallets successfully saved to: {}", wallet_file);
                            }
                            Err(e) => {
                                log::error!("❌ Failed to write wallets to {}: {}", wallet_file, e);
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("❌ Failed to serialize wallets: {}", e);
                    }
                }
            }
            Err(e) => {
                log::error!("❌ Failed to ensure storage directory: {}", e);
            }
        }
    }
}

pub fn has_completed_onboarding() -> bool {
    log::info!("🔄 Checking onboarding status");
    
    #[cfg(feature = "web")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        storage.get_item("onboarding_completed")
            .unwrap()
            .map(|val| val == "true")
            .unwrap_or(false)
    }
    
    #[cfg(not(feature = "web"))]
    {
        let storage_dir = get_storage_dir_simple();
        let onboarding_file = format!("{}/onboarding_completed.txt", storage_dir);
        
        match std::fs::read_to_string(&onboarding_file) {
            Ok(data) => {
                let completed = data.trim() == "true";
                log::info!("✅ Onboarding status: {}", completed);
                completed
            }
            Err(_) => {
                log::info!("📝 No onboarding file found - first launch");
                false
            }
        }
    }
}

pub fn mark_onboarding_completed() {
    log::info!("✅ Marking onboarding as completed");
    
    #[cfg(feature = "web")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        storage.set_item("onboarding_completed", "true").unwrap();
    }
    
    #[cfg(not(feature = "web"))]
    {
        if let Ok(_) = ensure_storage_dir() {
            let storage_dir = get_storage_dir_simple();
            let onboarding_file = format!("{}/onboarding_completed.txt", storage_dir);
            
            match std::fs::write(&onboarding_file, "true") {
                Ok(_) => log::info!("✅ Onboarding completion saved"),
                Err(e) => log::error!("❌ Failed to save onboarding status: {}", e),
            }
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// PIN Storage Functions
// ══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinData {
    pub pin_hash: String,
    pub salt: Vec<u8>,
    pub failed_attempts: u32,
}

fn get_pin_file_path() -> String {
    let storage_dir = get_storage_dir_simple();
    format!("{}/pin.json", storage_dir)
}

/// Check if a PIN is set
pub fn has_pin() -> bool {
    #[cfg(feature = "web")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        storage.get_item("pin_data").unwrap().is_some()
    }
    
    #[cfg(not(feature = "web"))]
    {
        let pin_file = get_pin_file_path();
        Path::new(&pin_file).exists()
    }
}

/// Save PIN hash and salt
pub fn save_pin(pin: &str) -> Result<(), String> {
    use crate::pin::{hash_pin, generate_salt};
    
    log::info!("🔐 Saving PIN to storage");
    
    let pin_hash = hash_pin(pin);
    let salt = generate_salt();
    
    let pin_data = PinData {
        pin_hash,
        salt: salt.to_vec(),
        failed_attempts: 0,
    };
    
    #[cfg(feature = "web")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        let serialized = serde_json::to_string(&pin_data)
            .map_err(|e| format!("Failed to serialize PIN data: {}", e))?;
        storage.set_item("pin_data", &serialized)
            .map_err(|_| "Failed to save PIN to web storage".to_string())?;
        log::info!("✅ PIN saved to web storage");
        Ok(())
    }
    
    #[cfg(not(feature = "web"))]
    {
        ensure_storage_dir()
            .map_err(|e| format!("Failed to ensure storage directory: {}", e))?;
        
        let pin_file = get_pin_file_path();
        let serialized = serde_json::to_string_pretty(&pin_data)
            .map_err(|e| format!("Failed to serialize PIN data: {}", e))?;
        
        std::fs::write(&pin_file, serialized)
            .map_err(|e| format!("Failed to write PIN file: {}", e))?;
        
        log::info!("✅ PIN saved to: {}", pin_file);
        Ok(())
    }
}

/// Verify PIN and return salt if correct
pub fn verify_pin(pin: &str) -> Result<Vec<u8>, String> {
    use crate::pin::hash_pin;
    
    if is_pin_locked() {
        return Err("PIN is locked due to too many failed attempts".to_string());
    }
    
    let mut pin_data = load_pin_data()?;
    let pin_hash = hash_pin(pin);
    
    if pin_hash == pin_data.pin_hash {
        // Correct PIN - reset failed attempts
        pin_data.failed_attempts = 0;
        let _ = save_pin_data(&pin_data);
        log::info!("✅ PIN verified successfully");
        Ok(pin_data.salt)
    } else {
        // Wrong PIN - increment failed attempts
        pin_data.failed_attempts += 1;
        log::warn!("❌ PIN verification failed. Attempts: {}/10", pin_data.failed_attempts);
        let _ = save_pin_data(&pin_data);
        
        if pin_data.failed_attempts >= 10 {
            Err("PIN locked due to too many failed attempts".to_string())
        } else {
            Err(format!("Incorrect PIN. {} attempts remaining", 10 - pin_data.failed_attempts))
        }
    }
}

/// Check if PIN is locked
pub fn is_pin_locked() -> bool {
    if let Ok(pin_data) = load_pin_data() {
        pin_data.failed_attempts >= 10
    } else {
        false
    }
}

/// Get salt for encryption (used when PIN is already verified)
pub fn get_pin_salt() -> Result<Vec<u8>, String> {
    let pin_data = load_pin_data()?;
    Ok(pin_data.salt)
}

/// Load PIN data from storage
fn load_pin_data() -> Result<PinData, String> {
    #[cfg(feature = "web")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        let data = storage.get_item("pin_data")
            .map_err(|_| "Failed to access web storage".to_string())?
            .ok_or_else(|| "No PIN data found".to_string())?;
        
        serde_json::from_str(&data)
            .map_err(|e| format!("Failed to parse PIN data: {}", e))
    }
    
    #[cfg(not(feature = "web"))]
    {
        let pin_file = get_pin_file_path();
        let data = std::fs::read_to_string(&pin_file)
            .map_err(|_| "No PIN data found".to_string())?;
        
        serde_json::from_str(&data)
            .map_err(|e| format!("Failed to parse PIN data: {}", e))
    }
}

/// Save PIN data to storage
fn save_pin_data(pin_data: &PinData) -> Result<(), String> {
    #[cfg(feature = "web")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        let serialized = serde_json::to_string(pin_data)
            .map_err(|e| format!("Failed to serialize PIN data: {}", e))?;
        storage.set_item("pin_data", &serialized)
            .map_err(|_| "Failed to save PIN data to web storage".to_string())?;
        Ok(())
    }
    
    #[cfg(not(feature = "web"))]
    {
        let pin_file = get_pin_file_path();
        let serialized = serde_json::to_string_pretty(pin_data)
            .map_err(|e| format!("Failed to serialize PIN data: {}", e))?;
        
        std::fs::write(&pin_file, serialized)
            .map_err(|e| format!("Failed to write PIN file: {}", e))?;
        
        Ok(())
    }
}

/// Remove PIN from storage
pub fn remove_pin() -> Result<(), String> {
    log::info!("🔐 Removing PIN from storage");
    
    #[cfg(feature = "web")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        storage.remove_item("pin_data")
            .map_err(|_| "Failed to remove PIN from web storage".to_string())?;
        log::info!("✅ PIN removed from web storage");
        Ok(())
    }
    
    #[cfg(not(feature = "web"))]
    {
        let pin_file = get_pin_file_path();
        std::fs::remove_file(&pin_file)
            .map_err(|e| format!("Failed to remove PIN file: {}", e))?;
        log::info!("✅ PIN removed from storage");
        Ok(())
    }
}
