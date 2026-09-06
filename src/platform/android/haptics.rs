//! Android Tactile Haptics Engine via JNI Vibrator and VibrationEffect API.
//!
//! Provides sub-frame tactile click feedback on virtual button transitions,
//! supporting Android 8.0+ (API >= 26) `VibrationEffect` with legacy `Vibrator.vibrate` fallbacks.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use log::{debug, info};

use jni::objects::{GlobalRef, JObject, JValue};
use jni::{JNIEnv, JavaVM};

use crate::platform::PlatformHaptics;

/// Native Android Haptic Feedback effect constants (`HapticFeedbackConstants`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum HapticEffect {
    VirtualKey = 1,   // HapticFeedbackConstants.VIRTUAL_KEY
    KeyboardTap = 3,  // HapticFeedbackConstants.KEYBOARD_TAP
    LongPress = 0,    // HapticFeedbackConstants.LONG_PRESS
}

/// Triggers native Android window haptic feedback via decor view with zero permissions.
pub fn trigger_haptic(env: &mut JNIEnv, activity_obj: &JObject, effect: HapticEffect) {
    let window = env
        .call_method(activity_obj, "getWindow", "()Landroid/view/Window;", &[])
        .ok()
        .and_then(|v| v.l().ok());

    if let Some(w) = window {
        let decor_view = env
            .call_method(&w, "getDecorView", "()Landroid/view/View;", &[])
            .ok()
            .and_then(|v| v.l().ok());

        if let Some(view) = decor_view {
            let _ = env.call_method(
                &view,
                "performHapticFeedback",
                "(I)Z",
                &[JValue::Int(effect as i32)],
            );
        }
    }
}

/// Default tactile click duration in milliseconds (15ms - 25ms light impulse).
pub const DEFAULT_HAPTIC_CLICK_MS: u64 = 20;

/// Default amplitude for light tactile click (0-255 or DEFAULT_AMPLITUDE = -1).
pub const DEFAULT_HAPTIC_AMPLITUDE: i32 = 180;

/// Native Android Haptics Engine managing JNI window haptic feedback and Vibrator fallbacks.
pub struct AndroidHaptics {
    vm: Option<Arc<JavaVM>>,
    activity_ref: Option<GlobalRef>,
    enabled: AtomicBool,
}

impl AndroidHaptics {
    /// Creates an uninitialized dummy haptics engine (used when JNI is not available).
    pub fn dummy() -> Self {
        Self {
            vm: None,
            activity_ref: None,
            enabled: AtomicBool::new(false),
        }
    }

    /// Initializes native Android Haptics using the `JavaVM` and `NativeActivity` jobject.
    pub fn new(vm: JavaVM, activity_raw: *mut std::ffi::c_void) -> Self {
        let arc_vm = Arc::new(vm);
        let activity_ref = if !activity_raw.is_null() {
            if let Ok(env) = arc_vm.attach_current_thread() {
                let local_obj = unsafe { JObject::from_raw(activity_raw as _) };
                env.new_global_ref(local_obj).ok()
            } else {
                None
            }
        } else {
            None
        };

        info!("Android Haptics Engine initialized (JNI ready: {})", activity_ref.is_some());

        Self {
            vm: Some(arc_vm),
            activity_ref,
            enabled: AtomicBool::new(true),
        }
    }

    /// Triggers a low-latency native haptic effect on the window DecorView.
    pub fn trigger_effect(&self, effect: HapticEffect) {
        if !self.is_enabled() {
            return;
        }

        let (vm, activity_ref) = match (&self.vm, &self.activity_ref) {
            (Some(vm), Some(act)) => (vm, act),
            _ => return,
        };

        if let Ok(mut env) = vm.attach_current_thread() {
            trigger_haptic(&mut env, activity_ref.as_obj(), effect);
        }
    }

    /// Triggers standard virtual key tactile click.
    pub fn trigger_virtual_key(&self) {
        self.trigger_effect(HapticEffect::VirtualKey);
    }

    /// Triggers subtle keyboard tap tick (used for D-Pad quadrant transitions).
    pub fn trigger_keyboard_tap(&self) {
        self.trigger_effect(HapticEffect::KeyboardTap);
    }

    /// Triggers a short tactile click impulse for virtual button activations.
    pub fn vibrate_click(&self) {
        if !self.is_enabled() {
            return;
        }
        self.trigger_virtual_key();
    }

    /// Triggers a vibration with custom duration and amplitude.
    pub fn vibrate_ms(&self, duration_ms: u64, amplitude: i32) {
        if !self.is_enabled() {
            return;
        }

        let (vm, activity_ref) = match (&self.vm, &self.activity_ref) {
            (Some(vm), Some(act)) => (vm, act),
            _ => return,
        };

        let mut env = match vm.attach_current_thread() {
            Ok(env) => env,
            Err(err) => {
                debug!("Failed to attach JNI thread for haptics: {:?}", err);
                return;
            }
        };

        // 1. Get Vibrator service: activity.getSystemService("vibrator")
        let service_name = match env.new_string("vibrator") {
            Ok(s) => s,
            Err(_) => return,
        };

        let vibrator_obj = match env.call_method(
            activity_ref.as_obj(),
            "getSystemService",
            "(Ljava/lang/String;)Ljava/lang/Object;",
            &[JValue::Object(&service_name)],
        ) {
            Ok(val) => match val.l() {
                Ok(obj) if !obj.is_null() => obj,
                _ => return,
            },
            Err(err) => {
                debug!("getSystemService(vibrator) failed: {:?}", err);
                return;
            }
        };

        // 2. Check Android SDK_INT version: android.os.Build.VERSION.SDK_INT
        let sdk_int = env
            .get_static_field("android/os/Build$VERSION", "SDK_INT", "I")
            .and_then(|val| val.i())
            .unwrap_or(26);

        if sdk_int >= 26 {
            // API >= 26: Use VibrationEffect.createOneShot(duration, amplitude)
            let effect_class = match env.find_class("android/os/VibrationEffect") {
                Ok(cls) => cls,
                Err(_) => {
                    // Fallback to legacy vibrate
                    let _ = env.call_method(
                        &vibrator_obj,
                        "vibrate",
                        "(J)V",
                        &[JValue::Long(duration_ms as i64)],
                    );
                    return;
                }
            };

            let effect_obj = env
                .call_static_method(
                    &effect_class,
                    "createOneShot",
                    "(JI)Landroid/os/VibrationEffect;",
                    &[JValue::Long(duration_ms as i64), JValue::Int(amplitude)],
                )
                .and_then(|val| val.l());

            if let Ok(effect) = effect_obj {
                let _ = env.call_method(
                    &vibrator_obj,
                    "vibrate",
                    "(Landroid/os/VibrationEffect;)V",
                    &[JValue::Object(&effect)],
                );
            } else {
                // Fallback to legacy vibrate
                let _ = env.call_method(
                    &vibrator_obj,
                    "vibrate",
                    "(J)V",
                    &[JValue::Long(duration_ms as i64)],
                );
            }
        } else {
            // Legacy API < 26: vibrator.vibrate(duration)
            let _ = env.call_method(
                &vibrator_obj,
                "vibrate",
                "(J)V",
                &[JValue::Long(duration_ms as i64)],
            );
        }
    }

    /// Sets whether tactile haptics are globally enabled.
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Release);
    }

    /// Returns whether tactile haptics are currently enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Acquire)
    }
}

impl PlatformHaptics for AndroidHaptics {
    fn vibrate_click(&self) {
        self.vibrate_click();
    }

    fn vibrate_custom(&self, duration_ms: u64, amplitude: u8) {
        self.vibrate_ms(duration_ms, amplitude as i32);
    }

    fn set_enabled(&self, enabled: bool) {
        self.set_enabled(enabled);
    }

    fn is_enabled(&self) -> bool {
        self.is_enabled()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dummy_android_haptics() {
        let haptics = AndroidHaptics::dummy();
        assert!(!haptics.is_enabled());

        haptics.set_enabled(true);
        assert!(haptics.is_enabled());

        // Calling vibrate on dummy should safely no-op without panicking
        haptics.vibrate_click();
        haptics.vibrate_ms(25, 200);
        haptics.trigger_virtual_key();
        haptics.trigger_keyboard_tap();
        haptics.trigger_effect(HapticEffect::LongPress);
    }

    #[test]
    fn test_haptic_effect_constants() {
        assert_eq!(HapticEffect::VirtualKey as i32, 1);
        assert_eq!(HapticEffect::KeyboardTap as i32, 3);
        assert_eq!(HapticEffect::LongPress as i32, 0);
    }
}
