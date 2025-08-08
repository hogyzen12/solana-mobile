// src/signing/mod.rs - Updated to include MWA signer
use crate::wallet::Wallet;
use std::error::Error;
use async_trait::async_trait;

pub mod software;
pub mod hardware;

use software::SoftwareSigner;
use hardware::HardwareSigner;

// Import MWA signer for Android only
#[cfg(target_os = "android")]
use crate::mwa::MwaSigner;

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
    
    /// Create an MWA signer (Android only)
    #[cfg(target_os = "android")]
    pub fn mwa(mwa_signer: MwaSigner) -> Self {
        SignerType::Mwa(mwa_signer)
    }
    
    /// Get a user-friendly description of the signer type
    pub fn get_type_name(&self) -> &'static str {
        match self {
            SignerType::Software(_) => "Software Wallet",
            SignerType::Hardware(_) => "Hardware Wallet",
            #[cfg(target_os = "android")]
            SignerType::Mwa(_) => "Mobile Wallet Adapter",
        }
    }
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
}