#[cfg(target_os = "android")]
use std::sync::{Arc, Mutex};
#[cfg(target_os = "android")]
use std::time::{Duration, Instant};
#[cfg(target_os = "android")]
use jni::objects::{JClass, JObject, JString, JValue, JByteArray, JObjectArray, GlobalRef};
#[cfg(target_os = "android")]
use jni::JNIEnv;
#[cfg(target_os = "android")]
use dioxus::mobile::wry::prelude::dispatch;
#[cfg(target_os = "android")]
use crate::hardware::protocol::{Command, Response, format_esp32_command, parse_esp32_response};

#[derive(Debug, Clone)]
pub struct StorageError(String);

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for StorageError {}

#[cfg(target_os = "android")]
impl From<jni::errors::Error> for StorageError {
    fn from(e: jni::errors::Error) -> Self {
        StorageError(format!("JNI Error: {}", e))
    }
}

#[cfg(target_os = "android")]
pub struct AndroidUsbSerial {
    pub port: Option<GlobalRef>,
    device_info: Option<AndroidUsbDevice>,
}

#[cfg(target_os = "android")]
#[derive(Debug, Clone)]
pub struct AndroidUsbDevice {
    pub vendor_id: i32,
    pub product_id: i32,
    pub device_name: String,
    pub manufacturer: Option<String>,
    pub product_name: Option<String>,
}

#[cfg(target_os = "android")]
impl AndroidUsbSerial {
    pub fn new() -> Self {
        Self {
            port: None,
            device_info: None,
        }
    }

    /// Check if hardware wallet devices are present
    pub async fn check_device_presence() -> bool {
        match Self::scan_for_devices().await {
            Ok(devices) => !devices.is_empty(),
            Err(_) => false,
        }
    }

    /// Scan for compatible USB serial devices
    pub async fn scan_for_devices() -> Result<Vec<AndroidUsbDevice>, StorageError> {
        let (tx, rx) = std::sync::mpsc::channel();

        dispatch(move |env, activity, _webview| {
            let activity_local = match env.new_local_ref(activity) {
                Ok(obj) => obj,
                Err(err) => {
                    let _ = tx.send(Err(StorageError::from(err)));
                    return;
                }
            };
            let result = Self::java_scan_usb_serial_devices(env, &activity_local);
            tx.send(result).unwrap();
        });

        match rx.recv() {
            Ok(result) => result,
            Err(e) => Err(StorageError(format!("Channel receive error: {}", e))),
        }
    }

    /// Connect to the first available hardware wallet device
    pub async fn find_and_connect(&mut self) -> Result<(), StorageError> {
        let devices = Self::scan_for_devices().await?;
        
        if devices.is_empty() {
            return Err(StorageError("No hardware wallet devices found".to_string()));
        }

        // Try to connect to the first compatible device
        for device in devices {
            if Self::is_hardware_wallet_device(device.vendor_id, device.product_id) {
                match self.connect_to_device(&device).await {
                    Ok(_) => {
                        log::info!("✅ Connected to hardware wallet: {}", device.device_name);
                        return Ok(());
                    }
                    Err(e) => {
                        log::warn!("❌ Failed to connect to {}: {}", device.device_name, e);
                        continue;
                    }
                }
            }
        }

        Err(StorageError("Failed to connect to any hardware wallet device".to_string()))
    }

    /// Connect to a specific USB device
    pub async fn connect_to_device(&mut self, device: &AndroidUsbDevice) -> Result<(), StorageError> {
        let device_clone = device.clone();
        let (tx, rx) = std::sync::mpsc::channel();

        dispatch(move |env, activity, _webview| {
            let activity_local = match env.new_local_ref(activity) {
                Ok(obj) => obj,
                Err(err) => {
                    let _ = tx.send(Err(StorageError::from(err)));
                    return;
                }
            };
            let result = Self::java_connect_usb_serial_device(env, &activity_local, &device_clone);
            tx.send(result).unwrap();
        });

        match rx.recv() {
            Ok(result) => match result {
                Ok(port_global) => {
                    self.port = Some(port_global);
                    self.device_info = Some(device.clone());
                    log::info!("✅ Connected to USB serial device: {}", device.device_name);
                    Ok(())
                }
                Err(e) => Err(e),
            },
            Err(e) => Err(StorageError(format!("Failed to connect to USB device: {}", e))),
        }
    }

    /// Send command to the hardware wallet
    pub async fn send_command(&self, command: Command) -> Result<Response, StorageError> {
        // Clone the GlobalRef to avoid lifetime issues
        let port_global = self.port.as_ref()
            .ok_or(StorageError("Not connected to hardware wallet".to_string()))?
            .clone();
        let cmd_data = format_esp32_command(&command);
        let (tx, rx) = std::sync::mpsc::channel();

        dispatch(move |env, activity, _webview| {
            let result = Self::java_usb_serial_transfer(env, activity, &port_global, &cmd_data);
            tx.send(result).unwrap();
        });

        match rx.recv() {
            Ok(result) => result.and_then(|response_data| parse_esp32_response(&response_data).map_err(|e| StorageError(format!("Failed to parse response: {}", e)))),
            Err(e) => Err(StorageError(format!("Failed to send command: {}", e))),
        }
    }

    /// Disconnect from the USB device
    pub async fn disconnect(&mut self) {
        if let Some(port_global) = self.port.take() { // Use take() to move the value out
            let (tx, rx) = std::sync::mpsc::channel();
            dispatch(move |env, activity, _webview| {
                let result = Self::java_disconnect_usb_serial_device(env, activity, &port_global);
                tx.send(result).unwrap();
            });
            let _ = rx.recv(); // Ignore result for simplicity
        }
        self.device_info = None;
        log::info!("🔌 Disconnected from USB serial device");
    }

    fn java_scan_usb_serial_devices<'a>(
        env: &mut JNIEnv<'a>,
        activity: &JObject<'a>,
    ) -> Result<Vec<AndroidUsbDevice>, StorageError> {
        // Get UsbManager
        let usb_service = env.get_static_field("android/content/Context", "USB_SERVICE", "Ljava/lang/String;")?.l()?;
        let usb_manager = env.call_method(activity, "getSystemService", "(Ljava/lang/String;)Ljava/lang/Object;", &[(&usb_service).into()])?.l()?;

        // Use UsbSerialProber to find all drivers
        let prober_class = Self::load_class(env, activity, "com.hoho.android.usbserial.driver.UsbSerialProber")?;
        let default_prober = env.call_static_method(prober_class, "getDefaultProber", "()Lcom/hoho/android/usbserial/driver/UsbSerialProber;", &[])?.l()?;

        let drivers = env.call_method(&default_prober, "findAllDrivers", "(Landroid/hardware/usb/UsbManager;)Ljava/util/List;", &[(&usb_manager).into()])?.l()?;

        let drivers_size = env.call_method(&drivers, "size", "()I", &[])?.i()?;
        let mut hardware_devices = Vec::new();

        for i in 0..drivers_size {
            let driver = env.call_method(&drivers, "get", "(I)Ljava/lang/Object;", &[i.into()])?.l()?;
            let usb_device = env.call_method(&driver, "getDevice", "()Landroid/hardware/usb/UsbDevice;", &[])?.l()?;

            let vendor_id = env.call_method(&usb_device, "getVendorId", "()I", &[])?.i()?;
            let product_id = env.call_method(&usb_device, "getProductId", "()I", &[])?.i()?;
            let device_name = env.call_method(&usb_device, "getDeviceName", "()Ljava/lang/String;", &[])?.l()?;

            let device_name_str: String = if !device_name.is_null() {
                env.get_string(&JString::from(device_name))?.into()
            } else {
                format!("USB Serial Device {:04X}:{:04X}", vendor_id, product_id)
            };

            if Self::is_hardware_wallet_device(vendor_id, product_id) {
                let hw_device = AndroidUsbDevice {
                    vendor_id,
                    product_id,
                    device_name: device_name_str,
                    manufacturer: None,
                    product_name: None,
                };
                hardware_devices.push(hw_device);
                log::info!("🔍 Found potential hardware wallet: {:04X}:{:04X}", vendor_id, product_id);
            }
        }
        Ok(hardware_devices)
    }

    fn java_connect_usb_serial_device<'a>(
        env: &mut JNIEnv<'a>,
        activity: &JObject<'a>,
        device: &AndroidUsbDevice,
    ) -> Result<GlobalRef, StorageError> {
        log::info!("🔄 Connecting to USB serial device: {:04X}:{:04X}", device.vendor_id, device.product_id);

        let usb_service = env.get_static_field("android/content/Context", "USB_SERVICE", "Ljava/lang/String;")?.l()?;
        let usb_manager = env.call_method(activity, "getSystemService", "(Ljava/lang/String;)Ljava/lang/Object;", &[(&usb_service).into()])?.l()?;

        let prober_class = Self::load_class(env, activity, "com.hoho.android.usbserial.driver.UsbSerialProber")?;
        let default_prober = env.call_static_method(prober_class, "getDefaultProber", "()Lcom/hoho/android/usbserial/driver/UsbSerialProber;", &[])?.l()?;
        let drivers = env.call_method(&default_prober, "findAllDrivers", "(Landroid/hardware/usb/UsbManager;)Ljava/util/List;", &[(&usb_manager).into()])?.l()?;

        let drivers_size = env.call_method(&drivers, "size", "()I", &[])?.i()?;
        let mut target_driver = None;

        for i in 0..drivers_size {
            let driver = env.call_method(&drivers, "get", "(I)Ljava/lang/Object;", &[i.into()])?.l()?;
            let usb_device = env.call_method(&driver, "getDevice", "()Landroid/hardware/usb/UsbDevice;", &[])?.l()?;
            let vendor_id = env.call_method(&usb_device, "getVendorId", "()I", &[])?.i()?;
            let product_id = env.call_method(&usb_device, "getProductId", "()I", &[])?.i()?;

            if vendor_id == device.vendor_id && product_id == device.product_id {
                target_driver = Some(driver);
                break;
            }
        }

        let driver = target_driver.ok_or(StorageError("Could not find USB serial driver for device".to_string()))?;
        let usb_device = env.call_method(&driver, "getDevice", "()Landroid/hardware/usb/UsbDevice;", &[])?.l()?;

        let has_permission = env.call_method(&usb_manager, "hasPermission", "(Landroid/hardware/usb/UsbDevice;)Z", &[(&usb_device).into()])?.z()?;
        if !has_permission {
            let action = env.new_string("dev.dioxus.main.USB_PERMISSION")?;
            let intent = env.new_object("android/content/Intent", "(Ljava/lang/String;)V", &[(&action).into()])?;
            let flags = env.get_static_field("android/app/PendingIntent", "FLAG_IMMUTABLE", "I")
                .map(|v| v.i().unwrap_or(0))
                .unwrap_or(0);
            let pending_intent = env.call_static_method(
                "android/app/PendingIntent",
                "getBroadcast",
                "(Landroid/content/Context;ILandroid/content/Intent;I)Landroid/app/PendingIntent;",
                &[activity.into(), 0.into(), (&intent).into(), flags.into()],
            )?.l()?;
            env.call_method(&usb_manager, "requestPermission", "(Landroid/hardware/usb/UsbDevice;Landroid/app/PendingIntent;)V", &[(&usb_device).into(), (&pending_intent).into()])?;
            return Err(StorageError("USB permission requested - please retry connection after granting access".to_string()));
        }

        let connection = env.call_method(&usb_manager, "openDevice", "(Landroid/hardware/usb/UsbDevice;)Landroid/hardware/usb/UsbDeviceConnection;", &[(&usb_device).into()])?.l()?;

        if connection.is_null() {
            return Err(StorageError("Failed to open USB device connection - permission required".to_string()));
        }

        let ports = env.call_method(&driver, "getPorts", "()Ljava/util/List;", &[])?.l()?;
        let port = env.call_method(&ports, "get", "(I)Ljava/lang/Object;", &[0.into()])?.l()?;

        env.call_method(&port, "open", "(Landroid/hardware/usb/UsbDeviceConnection;)V", &[(&connection).into()])?;
        env.call_method(&port, "setParameters", "(IIII)V", &[115200.into(), 8.into(), 1.into(), 0.into()])?;

        let port_global = env.new_global_ref(&port)?;
        log::info!("✅ USB serial connection established");
        Ok(port_global)
    }

    fn java_usb_serial_transfer(
        env: &mut JNIEnv<'_>,
        _activity: &JObject<'_>,
        port_global: &GlobalRef,
        data: &[u8],
    ) -> Result<Vec<u8>, StorageError> {
        log::info!("📤 USB Serial Transfer: {} bytes", data.len());
        let port = port_global.as_obj();

        Self::drain_serial(env, &port, 200)?;

        let java_data = env.byte_array_from_slice(data)?;
        let bytes_written = Self::call_port_write(env, &port, java_data, data.len())?;

        if bytes_written <= 0 {
            return Err(StorageError("Failed to write data to USB serial port".to_string()));
        }

        let is_sign = data.starts_with(b"SIGN:");
        let start = Instant::now();
        let mut response = Vec::new();
        let max_wait = if is_sign { Duration::from_millis(35_000) } else { Duration::from_millis(5_000) };

        loop {
            let response_buffer = env.new_byte_array(1024)?;
            let bytes_read = env.call_method(
                &port,
                "read",
                "([BI)I",
                &[(&response_buffer).into(), 500.into()],
            )?.i()?;

            if bytes_read > 0 {
                let chunk = env.convert_byte_array(&response_buffer)?;
                response.extend_from_slice(&chunk[..bytes_read as usize]);
                let lines = Self::extract_lines(&response);
                if lines.iter().any(|line| line == "READY") {
                    break;
                }
            }

            if start.elapsed() > max_wait {
                break;
            }

            std::thread::sleep(Duration::from_millis(20));
        }

        if response.is_empty() {
            return Err(StorageError("No response received from hardware wallet".to_string()));
        }

        let lines = Self::extract_lines(&response);
        if !is_sign {
            if let Some(line) = lines.iter().rev().find(|line| line.contains("PUBKEY:") || line.contains("ERROR:")) {
                response = line.as_bytes().to_vec();
            }
        } else {
            if let Some(line) = lines.iter().rev().find(|line| line.contains("SIGNATURE:") || line.contains("ERROR:")) {
                response = line.as_bytes().to_vec();
            }
        }

        let preview_len = response.len().min(120);
        let preview = String::from_utf8_lossy(&response[..preview_len]);
        let hex_preview = hex::encode(&response[..preview_len]);
        log::info!(
            "📥 Received {} bytes from hardware wallet: {}",
            response.len(),
            preview
        );
        log::info!("📥 Response hex ({} bytes): {}", preview_len, hex_preview);
        Ok(response)
    }

    fn extract_lines(data: &[u8]) -> Vec<String> {
        let mut lines = Vec::new();
        let mut current = String::new();
        for &b in data {
            if b == b'\n' || b == b'\r' {
                if !current.is_empty() {
                    lines.push(current.clone());
                    current.clear();
                }
                continue;
            }
            if b == 0x00 {
                continue;
            }
            let ch = b as char;
            if ch.is_control() {
                continue;
            }
            current.push(ch);
        }
        if !current.is_empty() {
            lines.push(current);
        }
        lines
    }

    fn drain_serial(env: &mut JNIEnv<'_>, port: &JObject<'_>, max_ms: u64) -> Result<(), StorageError> {
        let start = Instant::now();
        while start.elapsed() < Duration::from_millis(max_ms) {
            let response_buffer = env.new_byte_array(256)?;
            let bytes_read = env.call_method(
                port,
                "read",
                "([BI)I",
                &[(&response_buffer).into(), 50.into()],
            )?.i()?;
            if bytes_read <= 0 {
                break;
            }
        }
        Ok(())
    }

    fn java_disconnect_usb_serial_device(
        env: &mut JNIEnv<'_>,
        _activity: &JObject<'_>,
        port_global: &GlobalRef,
    ) -> Result<(), StorageError> {
        log::info!("🔌 Disconnecting USB serial device");
        let port = port_global.as_obj();
        env.call_method(&port, "close", "()V", &[])?;
        Ok(())
    }

    /// Check if the USB device could be a hardware wallet based on VID/PID
    fn is_hardware_wallet_device(vendor_id: i32, product_id: i32) -> bool {
        match (vendor_id, product_id) {
            // FTDI chips (commonly used in ESP32 dev boards)
            (0x0403, 0x6001) => true, // FT232R
            (0x0403, 0x6010) => true, // FT2232H
            (0x0403, 0x6011) => true, // FT4232H
            (0x0403, 0x6014) => true, // FT232H
            (0x0403, 0x6015) => true, // FT-X series
            // CP210x series (Silicon Labs)
            (0x10C4, 0xEA60) => true, // CP2102/CP2109
            (0x10C4, 0xEA70) => true, // CP2105
            (0x10C4, 0xEA71) => true, // CP2108
            // CH340/CH341 series (WinChipHead)
            (0x1A86, 0x7523) => true, // CH340
            (0x1A86, 0x5523) => true, // CH341
            // ESP32-S3 native USB
            (0x303A, 0x1001) => true, // ESP32-S3
            (0x303A, 0x0002) => true, // ESP32-S2
            // Add more hardware wallet VID/PIDs as needed
            _ => false,
        }
    }

    fn clear_java_exception(env: &mut JNIEnv<'_>, context: &str) {
        match env.exception_check() {
            Ok(true) => {
                let _ = env.exception_describe();
                let _ = env.exception_clear();
                log::error!("JNI exception while {}", context);
            }
            Ok(false) => {}
            Err(e) => {
                log::error!("Failed to check JNI exception while {}: {}", context, e);
            }
        }
    }

    fn call_port_write(
        env: &mut JNIEnv<'_>,
        port: &JObject<'_>,
        data: JByteArray<'_>,
        data_len: usize,
    ) -> Result<i32, StorageError> {
        let data_obj = JObject::from(data);
        let attempts = [
            ("([BI)V", false),
            ("([B)V", false),
            ("([BI)I", true),
            ("([B)I", true),
        ];

        for (sig, returns_int) in attempts {
            let args: Vec<JValue> = match sig {
                "([BI)I" | "([BI)V" => vec![JValue::from(&data_obj), 1000.into()],
                "([B)I" | "([B)V" => vec![JValue::from(&data_obj)],
                _ => vec![JValue::from(&data_obj), 1000.into()],
            };

            match env.call_method(port, "write", sig, &args) {
                Ok(value) => {
                    if returns_int {
                        return Ok(value.i()?);
                    }
                    return Ok(data_len as i32);
                }
                Err(_) => {
                    Self::clear_java_exception(env, &format!("calling UsbSerialPort.write{}", sig));
                    continue;
                }
            }
        }

        Err(StorageError("USB write failed: no compatible write method found".to_string()))
    }

    fn load_class<'a, 'b>(
        env: &'a mut JNIEnv<'b>,
        activity: &JObject<'b>,
        class_name: &str,
    ) -> Result<JClass<'b>, StorageError> {
        let class_loader = env
            .call_method(activity, "getClassLoader", "()Ljava/lang/ClassLoader;", &[])
            .map_err(|e| {
                Self::clear_java_exception(env, "getting ClassLoader");
                StorageError::from(e)
            })?
            .l()?;
        let class_name = env.new_string(class_name)?;
        let class_obj = env
            .call_method(
                class_loader,
                "loadClass",
                "(Ljava/lang/String;)Ljava/lang/Class;",
                &[(&class_name).into()],
            )
            .map_err(|e| {
                Self::clear_java_exception(env, "loading UsbSerialProber class");
                StorageError::from(e)
            })?
            .l()?;
        Ok(JClass::from(class_obj))
    }
}
