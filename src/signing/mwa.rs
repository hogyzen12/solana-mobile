// src/signing/mwa.rs
#[cfg(target_os = "android")]
use crate::{ffi, MsgFromKotlin, RX};
use crate::signing::TransactionSigner;
use async_trait::async_trait;
use solana_sdk::{
    message::VersionedMessage,
    signature::Signature,
    transaction::VersionedTransaction,
};
use std::error::Error;
use std::time::{Duration, Instant};

const MWA_SIGN_TIMEOUT_SECS: u64 = 45;

#[derive(Clone)]
pub struct MwaSigner {
    pubkey: String,
}

impl MwaSigner {
    pub fn new(pubkey: String) -> Self {
        Self { pubkey }
    }

    #[cfg(target_os = "android")]
    pub async fn sign_transaction_bytes(unsigned_tx_bytes: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
        let signer = MwaSigner { pubkey: String::new() };
        signer.drain_pending_messages();
        ffi::initiate_sign_transaction_from_dioxus(unsigned_tx_bytes);
        signer.wait_for_signed_transaction().await
    }

    #[cfg(target_os = "android")]
    async fn wait_for_signed_transaction(&self) -> Result<Vec<u8>, Box<dyn Error>> {
        let deadline = Instant::now() + Duration::from_secs(MWA_SIGN_TIMEOUT_SECS);
        let rx = RX.get().ok_or("MWA channel not initialized")?;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(format!(
                    "MWA signing timeout - no response after {} seconds",
                    MWA_SIGN_TIMEOUT_SECS
                )
                .into());
            }
            match tokio::time::timeout(remaining, rx.recv()).await {
                Ok(Ok(MsgFromKotlin::SignedTransaction(signed_tx_b58))) => {
                    return Ok(bs58::decode(&signed_tx_b58).into_vec()?);
                }
                Ok(Ok(other)) => {
                    log::warn!("Ignoring unexpected MWA message while awaiting transaction: {:?}", other);
                }
                Ok(Err(_)) => return Err("MWA channel closed while waiting for signature".into()),
                Err(_) => {
                    return Err(format!(
                        "MWA signing timeout - no response after {} seconds",
                        MWA_SIGN_TIMEOUT_SECS
                    )
                    .into())
                }
            }
        }
    }

    #[cfg(target_os = "android")]
    async fn wait_for_signed_message(&self) -> Result<Vec<u8>, Box<dyn Error>> {
        let deadline = Instant::now() + Duration::from_secs(MWA_SIGN_TIMEOUT_SECS);
        let rx = RX.get().ok_or("MWA channel not initialized")?;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(format!(
                    "MWA signing timeout - no response after {} seconds",
                    MWA_SIGN_TIMEOUT_SECS
                )
                .into());
            }
            match tokio::time::timeout(remaining, rx.recv()).await {
                Ok(Ok(MsgFromKotlin::SignedMessage(signature_b58))) => {
                    return Ok(bs58::decode(&signature_b58).into_vec()?);
                }
                Ok(Ok(other)) => {
                    log::warn!("Ignoring unexpected MWA message while awaiting message signature: {:?}", other);
                }
                Ok(Err(_)) => return Err("MWA channel closed while waiting for signature".into()),
                Err(_) => {
                    return Err(format!(
                        "MWA signing timeout - no response after {} seconds",
                        MWA_SIGN_TIMEOUT_SECS
                    )
                    .into())
                }
            }
        }
    }

    #[cfg(target_os = "android")]
    fn drain_pending_messages(&self) {
        if let Some(rx) = RX.get() {
            while let Ok(msg) = rx.try_recv() {
                log::warn!("Dropping stale MWA message before signing: {:?}", msg);
            }
        }
    }
}

#[async_trait]
impl TransactionSigner for MwaSigner {
    async fn get_public_key(&self) -> Result<String, Box<dyn Error>> {
        Ok(self.pubkey.clone())
    }

    async fn sign_message(&self, message: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
        #[cfg(target_os = "android")]
        {
            self.drain_pending_messages();
            if let Ok(versioned_msg) = bincode::deserialize::<VersionedMessage>(message) {
                let sig_count = versioned_msg.header().num_required_signatures as usize;
                let tx = VersionedTransaction {
                    signatures: vec![Signature::default(); sig_count],
                    message: versioned_msg,
                };
                let tx_bytes = bincode::serialize(&tx)?;
                ffi::initiate_sign_transaction_from_dioxus(&tx_bytes);
                let signed_tx_bytes = self.wait_for_signed_transaction().await?;
                let signed_tx: VersionedTransaction = bincode::deserialize(&signed_tx_bytes)?;
                if let Some(sig) = signed_tx.signatures.first() {
                    return Ok(sig.as_ref().to_vec());
                }
                return Err("Signed transaction returned no signatures".into());
            }

            // Fallback to signMessage for non-transaction payloads.
            ffi::initiate_sign_message_from_dioxus(message);
            return self.wait_for_signed_message().await;
        }
        #[cfg(not(target_os = "android"))]
        {
            let _ = message;
            Err("MWA signer is only available on Android".into())
        }
    }

    fn get_name(&self) -> String {
        "Seed Vault".to_string()
    }

    async fn is_available(&self) -> bool {
        true
    }

    fn is_hardware(&self) -> bool {
        false
    }
}
