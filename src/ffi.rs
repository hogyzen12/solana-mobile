use jni::objects::{JObject, JString};
use jni::sys::jstring;
use jni::JNIEnv;
use jni::JavaVM;
use once_cell::sync::Lazy;

// Global static variable to store the JavaVM pointer
// This is necessary because the JNIEnv is thread-local and we might need to attach/detach threads
// For now, we'll assume we're on the main thread where ANativeActivity_onCreate is called
static JVM: Lazy<JavaVM> = Lazy::new(|| {
    // This is a placeholder. In a real Android app, the JavaVM is typically obtained
    // during JNI_OnLoad or passed from the Java side when the native library is loaded.
    // For Dioxus on Android, the JavaVM is usually initialized by the Dioxus Mobile runtime.
    // We need a way to get it here.
    // For now, let's assume it's magically available or panic.
    // In a proper implementation, this would be initialized by calling a function like:
    // #[no_mangle]
    // pub extern "system" fn JNI_OnLoad(vm: JavaVM, _: *mut std::ffi::c_void) -> jni::sys::jint {
    //     unsafe { JVM_INSTANCE = Some(vm) };
    //     jni::JNIVersion::V6.into()
    // }
    // However, Dioxus mobile might handle this differently.
    // We will need to get the JVM from the current Android context.
    // This part is tricky without knowing exactly how Dioxus integrates with Android's JNI.
    // For now, we will add a function to initialize it.
    panic!("JVM not initialized. Call init_jvm first.");
});

// Function to initialize the JVM. This should be called from Kotlin/Java.
#[no_mangle]
pub extern "system" fn Java_dev_dioxus_main_DioxusJNI_initJVM(_env: JNIEnv, _class: JObject) {
    // It's not straightforward to store the JNIEnv globally as it's thread-local.
    // However, we can get the JavaVM from the JNIEnv and store that.
    // We can't directly assign to JVM here because it's already initialized with a panic.
    // This approach needs refinement. A better way is to have an Option<JavaVM>
    // and initialize it once.
    // For now, this function signature is a starting point for how Java might call Rust.
    // The actual initialization of the static JVM would need to be done carefully.
    // Let's assume for now that Dioxus's Android setup provides a way to get the JavaVM.
    // Perhaps through an existing context or a specific Dioxus FFI function.

    // A more realistic approach for a static JVM:
    // static mut JVM_INSTANCE: Option<JavaVM> = None;
    // pub fn set_java_vm(vm: JavaVM) {
    //     unsafe {
    //         JVM_INSTANCE = Some(vm);
    //     }
    // }
    // Then JVM could be:
    // static JVM: Lazy<&'static JavaVM> = Lazy::new(|| {
    //    unsafe { JVM_INSTANCE.as_ref().expect("JVM not initialized") }
    // });
    // This initJVM function is more of a placeholder for where the JVM would come from.
    // In Dioxus, this might be handled by `android_activity::AndroidApp`.
    // For now, we will focus on the call_kotlin_method function assuming JVM is available.
    // We will need to adjust JVM initialization later.
    log::info!("JVM init function called (placeholder)");
}

#[allow(dead_code)]
pub fn call_get_hardcoded_string_kotlin() -> Result<String, jni::errors::Error> {
    // Get the JNIEnv. This assumes the current thread is attached to the JVM.
    // In an Android app, the main thread usually is.
    // If called from a different Rust thread, it would need to be attached first.
    let _env = JVM.get_env()?;

    // TODO: The rest of the JNI calls will go here.
    Ok("Placeholder from Rust".to_string())
}

// Example of how one might get the JNIEnv if not on an already attached thread.
// This is usually not needed if called from a context where JNIEnv is already available (e.g. JNI native method).
#[allow(dead_code)]
fn get_jni_env() -> Result<JNIEnv<'static>, jni::errors::Error> {
    // This assumes `JVM` is correctly initialized with a valid JavaVM pointer.
    // The `'static` lifetime here is because `JVM` is static.
    // Careful: JNIEnv is thread-local. If you store and reuse it across threads, it's an error.
    // Getting it from `JavaVM::get_env()` or `attach_current_thread()` is the correct way per call or per thread.
    JVM.attach_current_thread_as_daemon() // or attach_current_thread()
}

// To make this callable from Java/Kotlin, e.g. for testing or direct invocation
#[no_mangle]
pub extern "system" fn Java_dev_dioxus_main_DioxusJNI_getHardcodedStringFromRust(
    mut env: JNIEnv, // Add mut here
    // this is the class that owns this static method
    _class: JObject,
) -> jstring {
    unsafe {
        match call_get_hardcoded_string_kotlin_internal(&mut env) {
            // Pass as &mut env
            Ok(rust_string) => {
                // Convert the Rust String to a Java String
                let output = env // This env is the original one, which should still be valid for new_string
                    .new_string(&rust_string)
                    .expect("Couldn't create java string!");
                // Extract the raw JNI jstring pointer
                output.into_raw()
            }
            Err(e) => {
                // In case of an error, we should probably throw a Java exception
                // For now, let's return a generic error message
                let error_msg = format!("Error in Rust: {:?}", e);
                let output = env // Same here
                    .new_string(error_msg)
                    .expect("Couldn't create java string for error!");
                output.into_raw()
            }
        }
    }
}

// Internal function that takes JNIEnv to be easily callable from JNI functions
unsafe fn call_get_hardcoded_string_kotlin_internal(
    env: &mut JNIEnv,
) -> Result<String, jni::errors::Error> {
    let class_name = "dev/dioxus/main/DioxusUtils";

    // Step 1a: Get constructor ID using class_name (string)
    // The constructor is a special method named "<init>" with signature "()V" (takes no args, returns void).
    let constructor_mid = env.get_method_id(class_name, "<init>", "()V")?;

    // Step 1b: Find the JClass object (needed for new_object_unchecked)
    let class_jobject = env.find_class(class_name)?;

    // Step 2: Create an instance of the class using the JClass object
    // class_jobject will be moved here.
    let instance = env.new_object_unchecked(class_jobject, constructor_mid, &[])?;

    // Step 3: Get the method ID for getHardcodedString using class_name (string)
    // The method takes no arguments and returns a String.
    // The signature for a method returning String is "()Ljava/lang/String;"
    let method_name = "getHardcodedString";
    let method_sig = "()Ljava/lang/String;";
    let method_id = env.get_method_id(class_name, method_name, method_sig)?;

    // Step 4: Call the method
    // The call_method_unchecked is used here. For type safety, one could use call_method.
    // Since we expect a JObject (specifically a JString), we use `l` for object type in JValue.
    let java_string_obj = env
        .call_method_unchecked(instance, method_id, jni::signature::ReturnType::Object, &[])?
        .l()?;

    // Step 5: Convert the Java String (JString) to a Rust String
    // We need to cast the generic JObject to a JString
    let java_string: JString = java_string_obj.into();
    let rust_string: String = env.get_string(&java_string)?.into();

    Ok(rust_string)
}

// This is the primary function we want to expose to Rust callers within the Dioxus app
#[allow(dead_code)]
pub fn get_string_from_kotlin() -> String {
    // This is a simplified entry point. It assumes JVM is initialized and the thread is attached.
    // This will be problematic if JVM is not initialized.
    // The `Lazy<JavaVM>` with `panic!` is a placeholder.
    // A robust solution needs a proper way to get `JNIEnv`.

    // In a Dioxus Android app, `dioxus::mobile::get_android_app_service().get_jni_env()` might be the way.
    // Let's assume for a moment we can get an env.
    // This part requires understanding how Dioxus provides JNIEnv to the app code.
    // If using `android_activity` directly or similar, there's usually a global `AndroidApp`
    // from which one can get `native_activity().jvm()`.

    // For now, we'll try to use the static JVM, assuming it gets initialized.
    // This function is intended to be called from other Rust code within the app.
    if let Ok(mut env) = JVM.get_env() {
        // Add mut here
        // This will panic if JVM is not initialized by `init_jvm_globally`
        match unsafe { call_get_hardcoded_string_kotlin_internal(&mut env) } {
            // Pass as &mut env
            Ok(s) => s,
            Err(e) => {
                log::error!("Error calling Kotlin from Rust: {:?}", e);
                format!("Error from Rust: {:?}", e)
            }
        }
    } else {
        log::error!("Could not get JNIEnv, JVM might not be initialized or thread not attached.");
        "Error: Could not get JNIEnv".to_string()
    }
}

// A function to be called by Kotlin to set the JVM globally.
// This is a common pattern but needs to be handled carefully with static mut.
use std::sync::Once;
static mut GLOBAL_JVM: Option<JavaVM> = None;
static INIT: Once = Once::new();

#[no_mangle]
pub extern "system" fn Java_dev_dioxus_main_DioxusJNI_cacheVm(env: JNIEnv, _class: JObject) {
    let vm = env.get_java_vm().unwrap();
    INIT.call_once(|| unsafe {
        GLOBAL_JVM = Some(vm);
    });
    log::info!("Rust: JavaVM cached.");
}

// The primary public function for Rust code to call.
#[allow(dead_code)]
pub fn call_kotlin_method_to_get_string() -> Result<String, String> {
    let jvm = unsafe {
        GLOBAL_JVM
            .as_ref()
            .ok_or_else(|| "JVM not cached".to_string())?
    };
    let mut env = jvm
        .get_env()
        .map_err(|e| format!("Failed to get JNIEnv: {:?}", e))?;

    // Now call the internal logic
    unsafe { call_get_hardcoded_string_kotlin_internal(&mut env) } // Pass as &mut env
        .map_err(|e| format!("JNI call failed: {:?}", e))
}
