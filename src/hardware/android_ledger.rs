#[cfg(target_os = "android")]
use base64::Engine;
#[cfg(target_os = "android")]
use dioxus::mobile::wry::prelude::dispatch;
#[cfg(target_os = "android")]
use jni::objects::{JClass, JObject, JString};
#[cfg(target_os = "android")]
use jni::JNIEnv;

#[derive(Debug, Clone)]
pub struct LedgerError(pub String);

impl std::fmt::Display for LedgerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for LedgerError {}

#[cfg(target_os = "android")]
impl From<jni::errors::Error> for LedgerError {
    fn from(e: jni::errors::Error) -> Self {
        LedgerError(format!("JNI Error: {}", e))
    }
}

#[cfg(target_os = "android")]
#[derive(Debug, Clone)]
pub struct AndroidLedgerDevice {
    pub vendor_id: i32,
    pub product_id: i32,
    pub device_name: String,
}

#[cfg(target_os = "android")]
pub struct AndroidLedgerConnection;

#[cfg(target_os = "android")]
impl AndroidLedgerConnection {
    pub async fn scan_for_devices() -> Result<Vec<AndroidLedgerDevice>, LedgerError> {
        let (tx, rx) = std::sync::mpsc::channel();

        dispatch(move |env, activity, _webview| {
            let activity_local = match env.new_local_ref(activity) {
                Ok(obj) => obj,
                Err(err) => {
                    let _ = tx.send(Err(LedgerError::from(err)));
                    return;
                }
            };
            let result = Self::java_list_ledger_devices(env, &activity_local);
            tx.send(result).unwrap();
        });

        match rx.recv() {
            Ok(result) => result,
            Err(e) => Err(LedgerError(format!("Channel receive error: {}", e))),
        }
    }

    pub async fn get_public_key() -> Result<String, LedgerError> {
        let (tx, rx) = std::sync::mpsc::channel();

        dispatch(move |env, activity, _webview| {
            let activity_local = match env.new_local_ref(activity) {
                Ok(obj) => obj,
                Err(err) => {
                    let _ = tx.send(Err(LedgerError::from(err)));
                    return;
                }
            };
            let result = Self::java_get_pubkey(env, &activity_local);
            tx.send(result).unwrap();
        });

        match rx.recv() {
            Ok(result) => result,
            Err(e) => Err(LedgerError(format!("Channel receive error: {}", e))),
        }
    }

    pub async fn sign_message(message: &[u8]) -> Result<Vec<u8>, LedgerError> {
        let message_b64 = base64::engine::general_purpose::STANDARD.encode(message);
        let (tx, rx) = std::sync::mpsc::channel();

        dispatch(move |env, activity, _webview| {
            let activity_local = match env.new_local_ref(activity) {
                Ok(obj) => obj,
                Err(err) => {
                    let _ = tx.send(Err(LedgerError::from(err)));
                    return;
                }
            };
            let result = Self::java_sign_message(env, &activity_local, &message_b64);
            tx.send(result).unwrap();
        });

        match rx.recv() {
            Ok(result) => result,
            Err(e) => Err(LedgerError(format!("Channel receive error: {}", e))),
        }
    }

    fn java_list_ledger_devices<'a>(
        env: &mut JNIEnv<'a>,
        activity: &JObject<'a>,
    ) -> Result<Vec<AndroidLedgerDevice>, LedgerError> {
        let ledger_class = Self::load_class(env, activity, "dev.dioxus.main.LedgerUsb")?;
        let devices_obj = env.call_static_method(
            ledger_class,
            "listLedgerDevices",
            "(Landroidx/activity/ComponentActivity;)[Ljava/lang/String;",
            &[activity.into()],
        )?.l()?;

        let array = jni::objects::JObjectArray::from(devices_obj);
        let len = env.get_array_length(&array)?;
        let mut devices = Vec::new();

        for i in 0..len {
            let item = env.get_object_array_element(&array, i)?;
            let entry: String = env.get_string(&JString::from(item))?.into();
            let mut parts = entry.splitn(3, '|');
            let vendor_id = parts.next().and_then(|v| v.parse::<i32>().ok()).unwrap_or(0);
            let product_id = parts.next().and_then(|v| v.parse::<i32>().ok()).unwrap_or(0);
            let device_name = parts.next().unwrap_or("Ledger").to_string();
            devices.push(AndroidLedgerDevice {
                vendor_id,
                product_id,
                device_name,
            });
        }

        Ok(devices)
    }

    fn java_get_pubkey<'a>(
        env: &mut JNIEnv<'a>,
        activity: &JObject<'a>,
    ) -> Result<String, LedgerError> {
        let ledger_class = Self::load_class(env, activity, "dev.dioxus.main.LedgerUsb")?;
        let result_obj = env.call_static_method(
            ledger_class,
            "getLedgerPubkey",
            "(Landroidx/activity/ComponentActivity;)Ljava/lang/String;",
            &[activity.into()],
        )?.l()?;

        let result: String = env.get_string(&JString::from(result_obj))?.into();
        if result.starts_with("ERROR:") {
            let hint = if result.starts_with("ERROR:SW_6988") {
                "Ledger Solana app blocked signing. Enable blind signing in the Solana app and retry."
            } else if result.starts_with("ERROR:SW_6a83") {
                "Ledger Solana app rejected the payload. Update the Solana app and enable blind signing, then retry."
            } else {
                ""
            };
            let message = if hint.is_empty() {
                result
            } else {
                format!("{} ({})", result, hint)
            };
            return Err(LedgerError(message));
        }

        let decoded = base64::engine::general_purpose::STANDARD
            .decode(result.trim())
            .map_err(|e| LedgerError(format!("Ledger pubkey base64 decode failed: {}", e)))?;

        if decoded.len() < 32 {
            return Err(LedgerError(format!(
                "Ledger pubkey too short: {} bytes",
                decoded.len()
            )));
        }

        let pubkey = bs58::encode(&decoded[..32]).into_string();
        Ok(pubkey)
    }

    fn java_sign_message<'a>(
        env: &mut JNIEnv<'a>,
        activity: &JObject<'a>,
        message_b64: &str,
    ) -> Result<Vec<u8>, LedgerError> {
        let ledger_class = Self::load_class(env, activity, "dev.dioxus.main.LedgerUsb")?;
        let message_j = env.new_string(message_b64)?;
        let result_obj = env.call_static_method(
            ledger_class,
            "signMessage",
            "(Landroidx/activity/ComponentActivity;Ljava/lang/String;)Ljava/lang/String;",
            &[activity.into(), (&message_j).into()],
        )?.l()?;

        let result: String = env.get_string(&JString::from(result_obj))?.into();
        if result.starts_with("ERROR:") {
            return Err(LedgerError(result));
        }

        let decoded = base64::engine::general_purpose::STANDARD
            .decode(result.trim())
            .map_err(|e| LedgerError(format!("Ledger signature base64 decode failed: {}", e)))?;

        Ok(decoded)
    }

    fn load_class<'a, 'b>(
        env: &'a mut JNIEnv<'b>,
        activity: &JObject<'b>,
        class_name: &str,
    ) -> Result<JClass<'b>, LedgerError> {
        let class_loader = env.call_method(
            activity,
            "getClassLoader",
            "()Ljava/lang/ClassLoader;",
            &[],
        )?.l()?;
        let class_name = env.new_string(class_name)?;
        let class_obj = env.call_method(
            class_loader,
            "loadClass",
            "(Ljava/lang/String;)Ljava/lang/Class;",
            &[(&class_name).into()],
        )?.l()?;
        Ok(JClass::from(class_obj))
    }
}
