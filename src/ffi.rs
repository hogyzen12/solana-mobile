use jni::{
    objects::{JClass, JString},
    JNIEnv, JavaVM,
};
use once_cell::sync::OnceCell;

/// Global, immutable JavaVM pointer – initialised in `JNI_OnLoad`.
static JVM: OnceCell<JavaVM> = OnceCell::new();

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
