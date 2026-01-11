// src/signing/mod.rs
use crate::wallet::{Wallet, WalletInfo};
use crate::hardware::HardwareWallet;
use std::error::Error;
use async_trait::async_trait;
use std::sync::Arc;

pub mod software;
pub mod hardware;
#[cfg(target_os = "android")]
pub mod mwa;

use software::SoftwareSigner;
use hardware::HardwareSigner;
#[cfg(target_os = "android")]
use mwa::MwaSigner;

/// Trait for different transaction signing methods
#[async_trait]
pub trait TransactionSigner: Send + Sync {
    /// Get the public key of the signer
    async fn get_public_key(&self) -> Result<String, Box<dyn Error>>;
    
    /// Sign a message/transaction
    async fn sign_message(&self, message: &[u8]) -> Result<Vec<u8>, Box<dyn Error>>;
    
    /// Get a display name for the signing method
    fn get_name(&self) -> String;
    
    /// Check if the signer is available/connected
    async fn is_available(&self) -> bool;

    /// Whether this signer is backed by hardware
    fn is_hardware(&self) -> bool {
        false
    }
}

/// Enum to hold different signer types
#[derive(Clone)]
pub enum SignerType {
    Software(SoftwareSigner),
    Hardware(HardwareSigner),
    #[cfg(target_os = "android")]
    Mwa(MwaSigner),
}

impl SignerType {
    /// Create a software signer from a wallet
    pub fn from_wallet(wallet: Wallet) -> Self {
        SignerType::Software(SoftwareSigner::new(wallet))
    }
    
    /// Create a hardware signer (attempts to connect)
    pub async fn hardware() -> Result<Self, Box<dyn Error>> {
        let signer = HardwareSigner::new().await?;
        Ok(SignerType::Hardware(signer))
    }

    #[cfg(target_os = "android")]
    pub fn mwa(pubkey: String) -> Self {
        SignerType::Mwa(MwaSigner::new(pubkey))
    }
}

pub fn select_signer(
    wallet_info: Option<WalletInfo>,
    hardware_wallet: Option<Arc<HardwareWallet>>,
    #[cfg(target_os = "android")] mwa_pubkey: Option<String>,
    #[cfg(not(target_os = "android"))] _mwa_pubkey: Option<String>,
) -> Result<SignerType, String> {
    #[cfg(target_os = "android")]
    {
        if let Some(pubkey) = mwa_pubkey {
            return Ok(SignerType::mwa(pubkey));
        }
    }

    if let Some(hw) = hardware_wallet {
        return Ok(SignerType::Hardware(HardwareSigner::from_wallet(hw)));
    }

    let wallet_info = wallet_info.ok_or_else(|| "No wallet available".to_string())?;
    let wallet = Wallet::from_wallet_info(&wallet_info)
        .map_err(|_| "Failed to load wallet".to_string())?;
    Ok(SignerType::from_wallet(wallet))
}

#[async_trait]
impl TransactionSigner for SignerType {
    async fn get_public_key(&self) -> Result<String, Box<dyn Error>> {
        match self {
            SignerType::Software(s) => s.get_public_key().await,
            SignerType::Hardware(h) => h.get_public_key().await,
            #[cfg(target_os = "android")]
            SignerType::Mwa(m) => m.get_public_key().await,
        }
    }
    
    async fn sign_message(&self, message: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
        match self {
            SignerType::Software(s) => s.sign_message(message).await,
            SignerType::Hardware(h) => h.sign_message(message).await,
            #[cfg(target_os = "android")]
            SignerType::Mwa(m) => m.sign_message(message).await,
        }
    }
    
    fn get_name(&self) -> String {
        match self {
            SignerType::Software(s) => s.get_name(),
            SignerType::Hardware(h) => h.get_name(),
            #[cfg(target_os = "android")]
            SignerType::Mwa(m) => m.get_name(),
        }
    }
    
    async fn is_available(&self) -> bool {
        match self {
            SignerType::Software(s) => s.is_available().await,
            SignerType::Hardware(h) => h.is_available().await,
            #[cfg(target_os = "android")]
            SignerType::Mwa(m) => m.is_available().await,
        }
    }

    fn is_hardware(&self) -> bool {
        match self {
            SignerType::Software(s) => s.is_hardware(),
            SignerType::Hardware(h) => h.is_hardware(),
            #[cfg(target_os = "android")]
            SignerType::Mwa(m) => m.is_hardware(),
        }
    }
}
