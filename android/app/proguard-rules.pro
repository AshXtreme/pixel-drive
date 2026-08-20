# Keep NativeActivity and JNI symbols
-keep class android.app.NativeActivity { *; }
-keepclasseswithmembernames class * {
    native <methods>;
}
