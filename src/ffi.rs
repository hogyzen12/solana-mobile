use jni::{
    objects::{JClass, JString}, // Removed GlobalRef, JObject
    JNIEnv,
    JavaVM,
};
use once_cell::sync::OnceCell;
use std::ffi::c_void; // Removed sync::Once

/// Global, immutable JavaVM pointer – initialised in `JNI_OnLoad`.
static JVM: OnceCell<JavaVM> = OnceCell::new();

/// Convenience: get a JNIEnv for *this* thread, attaching if necessary.
fn with_env<F, R>(f: F) -> R
where
    F: FnOnce(&mut JNIEnv) -> R, // Changed to &mut JNIEnv
{
    let vm = JVM.get().expect("JavaVM not initialised");
    // attach_current_thread returns a guard that detaches on Drop
    let mut guard = vm.attach_current_thread().expect("attach failed"); // Made mutable
    f(&mut guard) // Passed as mutable
}

/* ---------- JNI entry-points ---------- */

/// Called automatically by Android when the library is loaded.
/// Caches the JavaVM pointer exactly once.
#[no_mangle]
pub unsafe extern "system" fn JNI_OnLoad(
    vm: *mut jni::sys::JavaVM,
    _reserved: *mut c_void,
) -> jni::sys::jint {
    // SAFETY: vm is valid for the entire process lifetime.
    let java_vm = JavaVM::from_raw(vm).expect("JNI_OnLoad: invalid VM");
    JVM.set(java_vm).expect("JavaVM already set");
    // Tell Dalvik/ART which JNI version we support.
    jni::sys::JNI_VERSION_1_6
}

/// Kotlin calls this static method to test round‑trip.
/// Signature in Kotlin:  
/// `external fun getHardcodedStringFromRust(): String`
#[no_mangle]
pub extern "system" fn Java_dev_dioxus_main_DioxusJNI_getHardcodedStringFromRust(
    mut env: JNIEnv, // Made mutable
    _klass: JClass,
) -> jni::sys::jstring {
    match get_hardcoded_string(&mut env) {
        // Pass as mutable
        Ok(rust_string) => env.new_string(rust_string).unwrap().into_raw(),
        Err(e) => {
            let msg = env.new_string(format!("{:?}", e)).unwrap();
            msg.into_raw()
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

    // create object
    let obj = env.new_object(CLASS, "()V", &[])?;
    // call method
    let jstr = env.call_method(obj, METHOD, SIG, &[])?.l()?;
    // convert
    Ok(env.get_string(&JString::from(jstr))?.into())
}

/* ---------- Safe Rust API for the rest of the app ---------- */

pub fn call_kotlin_get_string() -> String {
    with_env(|env| match get_hardcoded_string(env) {
        Ok(s) => s,
        Err(e) => {
            log::error!("JNI error: {:?}", e);
            String::from("JNI error")
        }
    })
}
