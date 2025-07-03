### Linker Version Script (`empty.version`)

The `empty.version` file is a linker version script used during the Android build process. It controls the visibility of symbols (functions and data) in the compiled native library.

The script `VERS_1 { global: *; };` ensures that all global symbols from the Rust code are exported. This is crucial for the Java Native Interface (JNI) to find and call the necessary Rust functions from the Kotlin/Java side of the Android application.

Without this file, the linker might default to hiding all symbols, which would lead to an `UnsatisfiedLinkError` at runtime and cause the app to crash.


### Serving Your App

Run the following command in the root of your project to start developing with the default platform:

```bash
dx serve
```

To run for a different platform, use the `--platform platform` flag. E.g.
```bash
dx serve --platform desktop
```
