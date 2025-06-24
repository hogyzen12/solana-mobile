use jni::sys::jobject; // Added for jobject type
use jni::{
    objects::{JClass, JObject, JString, JValue},
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

/// Call Kotlin’s `DioxusUtils#establishMwaSession(ComponentActivity): String`
///
/// This function requires a `jobject` that is a valid reference to an
/// `androidx.activity.ComponentActivity` instance from the Android environment.
pub fn call_kotlin_establish_mwa_session(activity_jobject: jobject) -> String {
    // Uses the with_env helper to get a JNIEnv for the current thread and attach/detach.
    with_env(|env| {
        // env is &mut JNIEnv
        match do_establish_mwa_session(env, activity_jobject) {
            Ok(s) => s,
            Err(e) => {
                // Log the error using the log crate.
                log::error!("JNI error calling DioxusUtils.establishMwaSession: {:?}", e);
                // Return a formatted error message. Consider if more structured error
                // handling is needed by the calling Rust code.
                format!("JNI call to establishMwaSession failed: {:?}", e)
            }
        }
    })
}
