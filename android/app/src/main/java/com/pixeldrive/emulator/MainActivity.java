package com.pixeldrive.emulator;

import android.app.NativeActivity;
import android.content.Intent;
import android.net.Uri;
import android.os.Bundle;
import android.util.Log;

/**
 * PixelDrive Native Activity entry point.
 * Explicitly loads libc++_shared and the native library, and provides SAF ROM Document Picker integration.
 */
public class MainActivity extends NativeActivity {
    private static final String TAG = "PixelDrive";
    public static final int REQUEST_CODE_PICK_ROM = 0x524F; // "RO"

    // Native JNI callback to forward the selected SAF Content URI into the Rust event loop
    public static native void nativeOnRomSelected(String uriString);

    static {
        try {
            Log.i(TAG, "Loading C++ standard library runtime 'libc++_shared.so'...");
            System.loadLibrary("c++_shared");
            Log.i(TAG, "Successfully loaded 'libc++_shared.so'");
        } catch (Throwable t) {
            Log.w(TAG, "Note: libc++_shared load fallback (may already be provided by system): " + t.getMessage());
        }

        try {
            Log.i(TAG, "Loading native library 'libpixeldrive.so'...");
            System.loadLibrary("pixeldrive");
            Log.i(TAG, "Successfully loaded native library 'libpixeldrive.so'");
        } catch (Throwable t) {
            Log.e(TAG, "CRITICAL: Exception loading native library 'libpixeldrive.so': " + t.getMessage(), t);
        }
    }

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        Log.i(TAG, "MainActivity.onCreate started");
        try {
            super.onCreate(savedInstanceState);
            Log.i(TAG, "MainActivity.onCreate completed successfully");
        } catch (Throwable t) {
            Log.e(TAG, "CRITICAL: Exception inside NativeActivity.onCreate: " + t.getMessage(), t);
            throw t;
        }
    }

    @Override
    protected void onActivityResult(int requestCode, int resultCode, Intent data) {
        super.onActivityResult(requestCode, resultCode, data);
        if (requestCode == REQUEST_CODE_PICK_ROM && resultCode == RESULT_OK && data != null) {
            Uri uri = data.getData();
            if (uri != null) {
                String uriString = uri.toString();
                Log.i(TAG, "SAF ROM Picker selected Content URI: " + uriString);
                try {
                    // Take persistable URI permissions if granted
                    final int takeFlags = data.getFlags() & (Intent.FLAG_GRANT_READ_URI_PERMISSION | Intent.FLAG_GRANT_WRITE_URI_PERMISSION);
                    if (takeFlags != 0) {
                        try {
                            getContentResolver().takePersistableUriPermission(uri, takeFlags);
                            Log.i(TAG, "Acquired persistable URI permission for " + uriString + " with flags: 0x" + Integer.toHexString(takeFlags));
                        } catch (SecurityException se) {
                            Log.w(TAG, "Could not take persistable URI permission (provider might not support persistence): " + se.getMessage());
                        }
                    } else {
                        Log.d(TAG, "No persistable flags returned in data intent; streaming direct content stream");
                    }
                } catch (Throwable t) {
                    Log.w(TAG, "Unexpected error attempting persistable URI permission: " + t.getMessage());
                }

                try {
                    nativeOnRomSelected(uriString);
                    Log.i(TAG, "Dispatched nativeOnRomSelected to JNI event loop for: " + uriString);
                } catch (UnsatisfiedLinkError err) {
                    Log.e(TAG, "Failed to invoke nativeOnRomSelected: " + err.getMessage());
                }
            }
        }
    }

    /**
     * Launches the Android Storage Access Framework (SAF) document picker.
     */
    public void openRomPicker() {
        runOnUiThread(new Runnable() {
            @Override
            public void run() {
                try {
                    Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT);
                    intent.addCategory(Intent.CATEGORY_OPENABLE);
                    intent.setType("*/*");
                    intent.putExtra(Intent.EXTRA_MIME_TYPES, new String[] {
                        "application/octet-stream",
                        "application/zip",
                        "application/x-zip-compressed",
                        "application/x-gameboy-rom",
                        "application/x-gba-rom"
                    });
                    intent.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION | Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION);
                    startActivityForResult(intent, REQUEST_CODE_PICK_ROM);
                    Log.i(TAG, "Successfully fired ACTION_OPEN_DOCUMENT intent (persistable URI requested)");
                } catch (Exception e) {
                    Log.e(TAG, "Failed to launch ACTION_OPEN_DOCUMENT: " + e.getMessage(), e);
                }
            }
        });
    }

    /**
     * Shows a short Toast message on the UI thread.
     */
    public void showToast(final String message) {
        runOnUiThread(new Runnable() {
            @Override
            public void run() {
                try {
                    android.widget.Toast.makeText(MainActivity.this, message, android.widget.Toast.LENGTH_SHORT).show();
                } catch (Exception e) {
                    Log.e(TAG, "Failed to display toast: " + e.getMessage());
                }
            }
        });
    }
}
