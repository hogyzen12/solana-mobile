# Solana Mobile Dioxus Example

This is a sample Dioxus application that demonstrates how to integrate with the Solana Mobile Wallet Adapter (MWA) on Android. The app currently has implementations for `signTransaction`, `signMessage`, and `authorize` (connect).

We have forked the `dioxus-cli` and `wry` (the underlying web-view library from Tauri) to allow for the embedding of Solana dependencies directly into the Android Gradle build/bundle. And we've added a thin Kotlin layer in the android bundle that manages the Foreign Function Interface (FFI) between Rust and Kotlin, enabling communication with the Solana wallet methods.

## Setup

Before you can build this project, you must install the forked `dioxus-cli`. You'll need to clone our fork of the dioxus repo.

- Dioxus: https://github.com/regolith-labs/dioxus

Clone the forked `dioxus` repository, navigate to the `packages/cli` directory, and run the following command:

```bash
cargo install --path . --locked
```

## Android Build Scripts

This project uses a set of scripts to build, bundle, and update the Android application. These scripts are located in the `scripts/` directory and should be run from the root of the project.

### `android.env`

This file is crucial for the Android build process. It sets up the necessary environment variables, including `JAVA_HOME`, `ANDROID_HOME`, and paths to the NDK and other build tools. Before running any of the build scripts, you must ensure that the paths in `android.env` are correct for your local development environment.

### Build (`scripts/android.build.sh`)

This script compiles the Rust code into a native Android library using the Dioxus CLI. It targets the Android platform and creates a release build.

```bash
sh scripts/android.build.sh
```

### Bundle (`scripts/android.bundle.sh`)

After a successful build, this script bundles the application into an Android App Bundle (AAB). The AAB is a publishing format that includes all your app’s compiled code and resources.

```bash
sh scripts/android.bundle.sh
```

### Update (`scripts/android.update.sh`)

This script takes the generated AAB, builds a universal APK set, and installs it on a connected Android device. This is useful for quickly testing changes on a physical device.

```bash
sh scripts/android.update.sh
```


## Continuous Integration

This project uses GitHub Actions for Continuous Integration (CI). The workflow is defined in the `.github/workflows/android-build.yml` file.

The CI runner automates the following key steps:
- **Sets up the environment**: It installs the correct versions of Java, the Android SDK, and the Android NDK.
- **Installs Rust**: It sets up the specific nightly toolchain required for the project.
- **Builds Dependencies**: It compiles required dependencies (OpenSSL, etc.) for the Android target.
- **Builds the App**: It uses our fork of the Dioxus CLI to compile the Rust code and build the Android application.
- **Bundles the App**: It generates the Android App Bundle (AAB) for release.
- **Uploads Artifacts**: The final AAB is uploaded as a build artifact, making it available for download and testing.

This CI build is intended to provide an environment from scratch than can be used to build and bundle an app that is ready to be published to the Solana app store. For actually publishing your bundled app, follow the [official guide](https://docs.solanamobile.com/dapp-publishing/overview).

## Linker Version Script (`empty.version`)

The `empty.version` file is a linker version script used during the Android build process. It controls the visibility of symbols (functions and data) in the compiled native library.

The script `VERS_1 { global: *; };` ensures that all global symbols from the Rust code are exported. This is crucial for the Java Native Interface (JNI) to find and call the necessary Rust functions from the Kotlin/Java side of the Android application.

Without this file, the linker might default to hiding all symbols, which would lead to an `UnsatisfiedLinkError` at runtime and cause the app to crash.
