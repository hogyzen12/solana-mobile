package dev.dioxus.main

import android.app.Activity
import android.util.Log
import androidx.appcompat.app.AppCompatActivity
import androidx.lifecycle.lifecycleScope
import com.solana.mobilewalletadapter.clientlib.ActivityResultSender
import com.solana.mobilewalletadapter.clientlib.MobileWalletAdapter
import com.solana.mobilewalletadapter.clientlib.TransactionResult
import com.solana.mobilewalletadapter.common.util.toBase58 // For logging PublicKey
import kotlinx.coroutines.launch

class SolanaWalletManager {
    private val TAG = "SolanaWalletManager"

    // This method will be called from Rust via JNI
    fun initiateConnect(activity: Activity) {
        Log.d(TAG, "initiateConnect called with activity: ${activity.javaClass.simpleName}")

        if (activity !is AppCompatActivity) {
            Log.e(TAG, "Activity is not an AppCompatActivity. Cannot launch coroutine with lifecycleScope. Connection not initiated.")
            // In a production app, you might want to throw an exception or return an error code
            // if JNI expects a synchronous error indication here.
            return
        }

        val sender = ActivityResultSender(activity)
        // Using default MobileWalletAdapter constructor (default timeout, default config)
        // You can customize this with MobileWalletAdapter(timeoutMillis = ..., adapterConfig = ...)
        val walletAdapter = MobileWalletAdapter()

        // Launch a coroutine on the lifecycleScope of the activity
        // This is crucial because transact involves Activity results and its callback is suspending.
        activity.lifecycleScope.launch {
            try {
                Log.d(TAG, "Calling walletAdapter.transact within coroutine")
                walletAdapter.transact(sender) { authResult ->
                    // This block is a suspend lambda, executed when the wallet returns a result.
                    Log.d(TAG, "authResult received: $authResult")
                    when (authResult) {
                        is TransactionResult.Success -> {
                            val authParams = authResult.payload
                            Log.i(TAG, "Wallet connection successful! AuthToken: ${authParams.authToken}, PublicKey: ${authParams.publicKey.toBase58()}, WalletURI: ${authParams.walletUriBase}")
                            // TODO: Implement JNI callback to Rust with success details (authToken, publicKey, walletUriBase)
                        }
                        is TransactionResult.Failure -> {
                            Log.e(TAG, "Wallet connection failed: ${authResult.message}")
                            // TODO: Implement JNI callback to Rust with failure details (message)
                        }
                        is TransactionResult.Cancelled -> {
                            Log.w(TAG, "Wallet connection cancelled by user.")
                            // TODO: Implement JNI callback to Rust with cancellation info
                        }
                    }
                }
                Log.d(TAG, "walletAdapter.transact call dispatched. Waiting for async result in callback.")
            } catch (e: Exception) {
                Log.e(TAG, "Exception during walletAdapter.transact or its coroutine execution: ${e.message}", e)
                // TODO: Implement JNI callback to Rust with error details (exception message)
            }
        }
    }
}
