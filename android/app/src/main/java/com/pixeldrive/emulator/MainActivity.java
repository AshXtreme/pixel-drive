package com.pixeldrive.emulator;

import android.app.NativeActivity;
import android.os.Bundle;

/**
 * PixelDrive Native Activity entry point.
 * Explicitly loads the native library and ensures proper Android runtime lifecycle initialization.
 */
public class MainActivity extends NativeActivity {
    static {
        System.loadLibrary("pixeldrive");
    }

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
    }
}
