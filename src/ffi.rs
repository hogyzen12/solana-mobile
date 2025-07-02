use jni::sys::jobject;
use jni::{
    objects::{GlobalRef, JClass, JObject, JString, JValue},
    JNIEnv, JavaVM,
};
use once_cell::sync::OnceCell;

use crate::MsgFromKotlin;

/// Global, immutable JavaVM pointer – initialised in `JNI_OnLoad`.
static JVM: OnceCell<JavaVM> = OnceCell::new();
/// Global, immutable WryActivity jobject – initialised in `Java_dev_dioxus_main_WryActivity_create`.
static WRY_ACTIVITY: OnceCell<GlobalRef> = OnceCell::new();

/// Convenience: get a JNIEnv for *this* thread, attaching if necessary.
fn with_env<F, R>(f: F) -> R
where
    F: FnOnce(&mut JNIEnv) -> R,
{
    let vm = JVM.get().expect("JavaVM not initialised");
    // attach_current_thread returns a guard that detaches on Drop
    let mut guard = vm.attach_current_thread().expect("attach failed");
    f(&mut guard)
}

/* ---------- JNI entry-points ---------- */

/// Called by Kotlin's Ipc.cacheVm() to cache the JavaVM pointer.
/// Matches Kotlin declaration:
///
/// ```kotlin
/// companion object {
///   @JvmStatic external fun cacheVm()
/// }
/// ```
#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_dev_dioxus_main_Ipc_cacheVm(
    env: JNIEnv,
    _class: JClass, // JClass representing dev.dioxus.main.Ipc
) {
    match env.get_java_vm() {
        Ok(vm) => {
            // Store it once; ignores subsequent calls if already set.
            JVM.set(vm).ok();
        }
        Err(e) => {
            // It's crucial to handle this error, perhaps by logging or panicking,
            // as the application cannot function correctly without the JVM pointer.
            // For now, let's use a log, assuming a logger is set up.
            // If no logger, this will be a silent failure in release builds.
            eprintln!("JNI: failed to get JavaVM pointer in cacheVm: {:?}", e);
            // Consider panicking in debug builds or a more robust error handling strategy.
            // panic!("JNI: failed to get JavaVM pointer in cacheVm: {:?}", e);
        }
    }
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_dev_dioxus_main_Ipc_sendPublicKey(
    mut env: JNIEnv,
    _class: JClass, // JClass representing dev.dioxus.main.Ipc
    publicKey: JString,
) {
    let pub_key_str: String = match env.get_string(&publicKey) {
        Ok(s) => s.into(),
        Err(e) => {
            log::error!("Failed to get public key string from JNI: {:?}", e);
            return;
        }
    };
    log::info!(
        "Received public key from Kotlin, sending to channel: {}",
        pub_key_str
    );
    let msg = MsgFromKotlin::Pubkey(pub_key_str);
    crate::send_msg_from_ffi(msg);
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_dev_dioxus_main_Ipc_sendSignedTransaction(
    mut env: JNIEnv,
    _class: JClass,
    signedTransaction: JString,
) {
    let tx_str: String = match env.get_string(&signedTransaction) {
        Ok(s) => s.into(),
        Err(e) => {
            log::error!("Failed to get public key string from JNI: {:?}", e);
            return;
        }
    };
    log::info!(
        "Received signed transaction from Kotlin, sending to channel: {}",
        tx_str
    );
    let msg = MsgFromKotlin::SignedTransaction(tx_str);
    crate::send_msg_from_ffi(msg);
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_dev_dioxus_main_WryActivity_cacheActivityInstance(
    env: JNIEnv,
    // In Kotlin: `create(this)` is called on a WryActivity instance.
    // `external fun create(activity: WryActivity)`
    // So, `thiz_activity_obj` is the WryActivity instance on which `create` is invoked.
    // And `activity_arg_obj` is also that same WryActivity instance, passed as the argument.
    _thiz_activity_obj: JObject,
    activity_arg_obj: JObject,
) {
    match env.new_global_ref(activity_arg_obj) {
        Ok(global_ref) => {
            if WRY_ACTIVITY.set(global_ref).is_err() {
                // This case means WRY_ACTIVITY was already set. The new global_ref passed to set()
                // is returned in the Err variant and will be dropped, automatically deleting the JNI ref.
                eprintln!("JNI: WRY_ACTIVITY global ref was already set. New ref dropped.");
            }
        }
        Err(e) => {
            eprintln!("JNI: failed to create global ref for WryActivity: {:?}", e);
        }
    }
}

/* ---------- Rust helpers ---------- */

/// Call Kotlin’s `DioxusUtils#getHardcodedString(): String`
fn get_hardcoded_string(env: &mut JNIEnv) -> jni::errors::Result<String> {
    // Takes &mut JNIEnv
    const CLASS: &str = "dev/dioxus/main/DioxusUtils";
    const METHOD: &str = "getHardcodedString";
    const SIG: &str = "()Ljava/lang/String;";

    // Find the class
    let class = env.find_class(CLASS)?;
    // Call the static method
    let jstr = env.call_static_method(class, METHOD, SIG, &[])?.l()?;
    // Convert the JString to a Rust String
    Ok(env.get_string(&JString::from(jstr))?.into())
}

// Helper function to call Kotlin's DioxusUtils.establishMwaSession
// This function performs the actual JNI call.
fn do_establish_mwa_session(
    env: &mut JNIEnv,
    activity_jobject: jobject,
) -> jni::errors::Result<String> {
    const CLASS_NAME: &str = "dev/dioxus/main/DioxusUtils";
    const METHOD_NAME: &str = "establishMwaSession";
    // JNI signature for: static String establishMwaSession(androidx.activity.ComponentActivity activity)
    const METHOD_SIG: &str = "(Landroidx/activity/ComponentActivity;)Ljava/lang/String;";

    // Find the class dev.dioxus.main.DioxusUtils
    let class = env.find_class(CLASS_NAME)?;

    // Convert the raw jobject (which is a pointer/handle to the ComponentActivity instance)
    // into a jni-rs JObject wrapper.
    // Safety: Assumes activity_jobject is a valid, non-null JNI reference to a ComponentActivity.
    let activity_obj = unsafe { JObject::from_raw(activity_jobject) };

    // Prepare arguments for the JNI call.
    // JValue::from takes a reference to JObject.
    let jvalue_args = [JValue::from(&activity_obj)];

    // Call the static Java method.
    let result_jvalue = env.call_static_method(
        class,        // The JClass object for DioxusUtils
        METHOD_NAME,  // Name of the method: "establishMwaSession"
        METHOD_SIG,   // Signature: "(Landroidx/activity/ComponentActivity;)Ljava/lang/String;"
        &jvalue_args, // Arguments: the ComponentActivity JObject
    )?;

    // The result_jvalue is a JValue. We need to convert it to a JObject (which represents the Java String).
    // .l() attempts this conversion, returning a Result<JObject, Error>.
    let jstring_obj = result_jvalue.l()?;

    // Convert the JString JObject into a Rust String.
    // JString::from(jstring_obj) casts the JObject to JString.
    let rust_string: String = env.get_string(&JString::from(jstring_obj))?.into();

    Ok(rust_string)
}

// Helper function to call Kotlin's DioxusUtils.signTransaction
fn do_sign_transaction(
    env: &mut JNIEnv,
    activity_jobject: jobject,
    transaction: &[u8],
) -> jni::errors::Result<String> {
    const CLASS_NAME: &str = "dev/dioxus/main/DioxusUtils";
    const METHOD_NAME: &str = "signTransaction";
    // JNI signature for: static String signTransaction(androidx.activity.ComponentActivity activity, byte[] transaction)
    const METHOD_SIG: &str = "(Landroidx/activity/ComponentActivity;[B)Ljava/lang/String;";

    // Find the class
    let class = env.find_class(CLASS_NAME)?;

    // Convert raw jobject to JObject
    let activity_obj = unsafe { JObject::from_raw(activity_jobject) };

    // Convert rust byte slice to java byte array
    let transaction_jbyte_array = env.byte_array_from_slice(transaction)?;

    // Prepare arguments
    let transaction_jobject: JObject = transaction_jbyte_array.into();
    let jvalue_args = [
        JValue::from(&activity_obj),
        JValue::from(&transaction_jobject),
    ];

    // Call static method
    let result_jvalue = env.call_static_method(class, METHOD_NAME, METHOD_SIG, &jvalue_args)?;

    // Process result
    let jstring_obj = result_jvalue.l()?;
    let rust_string: String = env.get_string(&JString::from(jstring_obj))?.into();

    Ok(rust_string)
}

/* ---------- Safe Rust API for the rest of the app ---------- */

// /// Safely retrieves the public key that was sent from the Kotlin layer.
// pub fn get_public_key() -> Option<String> {
//     // Lock the mutex and clone the value.
//     // The lock is released automatically when the guard from `lock()` is dropped.
//     PUBLIC_KEY.lock().unwrap().clone()
// }

pub fn call_kotlin_get_string() -> String {
    with_env(|env| match get_hardcoded_string(env) {
        Ok(s) => s,
        Err(e) => {
            log::error!("JNI error: {:?}", e);
            String::from("JNI error")
        }
    })
}

// New function callable from Dioxus to initiate MWA session
pub fn initiate_mwa_session_from_dioxus() -> String {
    // Get the globally stored WryActivity GlobalRef
    let activity_global_ref = match WRY_ACTIVITY.get() {
        Some(glob_ref) => glob_ref,
        None => {
            let err_msg = "Error: WryActivity reference not available. MWA session cannot be initiated from Dioxus. Ensure WryActivity.create() has been called by the Android lifecycle.";
            log::error!("{}", err_msg);
            return String::from(err_msg);
        }
    };

    // Use with_env to get a JNIEnv for the current thread
    with_env(|env| {
        // Get a JObject (local reference) from the GlobalRef.
        // This local reference is valid only for the duration of this JNIEnv (inside this closure).
        let activity_jobject_local_ref = activity_global_ref.as_obj();

        // Convert the JObject (local ref) to a raw jobject, which is what
        // our `do_establish_mwa_session` helper expects.
        let raw_activity_jobject: jobject = activity_jobject_local_ref.as_raw();

        // Now call the helper function that actually performs the JNI call to Kotlin
        match do_establish_mwa_session(env, raw_activity_jobject) {
            Ok(s) => s,
            Err(e) => {
                log::error!("JNI error in initiate_mwa_session_from_dioxus when calling do_establish_mwa_session: {:?}", e);
                format!(
                    "JNI call from Dioxus to establishMwaSession failed: {:?}",
                    e
                )
            }
        }
    })
}

// New function callable from Dioxus to initiate MWA signing
// NOTE: This function must be invoked from the main dioxus thread,
// that means we cannot call this function from inside a dioxus::spawn
pub fn initiate_sign_transaction_from_dioxus(transaction: &[u8]) -> String {
    let activity_global_ref = match WRY_ACTIVITY.get() {
        Some(glob_ref) => glob_ref,
        None => {
            let err_msg = "Error: WryActivity reference not available. MWA signing cannot be initiated. Ensure WryActivity.create() has been called.";
            log::error!("{}", err_msg);
            return String::from(err_msg);
        }
    };

    with_env(|env| {
        let activity_jobject_local_ref = activity_global_ref.as_obj();
        let raw_activity_jobject: jobject = activity_jobject_local_ref.as_raw();

        match do_sign_transaction(env, raw_activity_jobject, transaction) {
            Ok(s) => s,
            Err(e) => {
                log::error!(
                    "JNI error in initiate_sign_transaction_from_dioxus: {:?}",
                    e
                );
                format!("JNI call to signTransaction failed: {:?}", e)
            }
        }
    })
}
