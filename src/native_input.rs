//! Native input injection for macOS using Core Graphics
//! This allows clicking anywhere on screen, including browser chrome, dialogs, etc.

use crate::error::{ChromeMcpError, Result};
use tracing::debug;

#[cfg(target_os = "macos")]
use core_graphics::{
    display::CGPoint,
    event::{CGEvent, CGEventTapLocation, CGEventType, CGMouseButton},
    event_source::{CGEventSource, CGEventSourceStateID},
};

/// Native input manager for macOS
pub struct NativeInputManager {
    #[cfg(target_os = "macos")]
    event_source: CGEventSource,
}

impl NativeInputManager {
    /// Create a new native input manager
    pub fn new() -> Result<Self> {
        #[cfg(target_os = "macos")]
        {
            let event_source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
                .map_err(|e| ChromeMcpError::native_input_error(format!("Failed to create event source: {:?}", e)))?;
            
            Ok(Self { event_source })
        }
        
        #[cfg(not(target_os = "macos"))]
        {
            warn!("Native input is only supported on macOS");
            Ok(Self {})
        }
    }

    /// Click at screen coordinates
    pub fn click_at(&self, x: f64, y: f64) -> Result<()> {
        debug!("Native click at ({}, {})", x, y);
        
        #[cfg(target_os = "macos")]
        {
            let point = CGPoint::new(x, y);
            
            // Create mouse down event
            let mouse_down = CGEvent::new_mouse_event(
                self.event_source.clone(),
                CGEventType::LeftMouseDown,
                point,
                CGMouseButton::Left,
            ).map_err(|e| ChromeMcpError::native_input_error(format!("Failed to create mouse down event: {:?}", e)))?;
            
            // Create mouse up event
            let mouse_up = CGEvent::new_mouse_event(
                self.event_source.clone(),
                CGEventType::LeftMouseUp,
                point,
                CGMouseButton::Left,
            ).map_err(|e| ChromeMcpError::native_input_error(format!("Failed to create mouse up event: {:?}", e)))?;
            
            // Post events
            mouse_down.post(CGEventTapLocation::HID);
            std::thread::sleep(std::time::Duration::from_millis(50));
            mouse_up.post(CGEventTapLocation::HID);
            
            Ok(())
        }
        
        #[cfg(not(target_os = "macos"))]
        {
            Err(ChromeMcpError::native_input_error("Native input only supported on macOS"))
        }
    }

    /// Right-click at screen coordinates
    pub fn right_click_at(&self, x: f64, y: f64) -> Result<()> {
        debug!("Native right-click at ({}, {})", x, y);
        
        #[cfg(target_os = "macos")]
        {
            let point = CGPoint::new(x, y);
            
            let mouse_down = CGEvent::new_mouse_event(
                self.event_source.clone(),
                CGEventType::RightMouseDown,
                point,
                CGMouseButton::Right,
            ).map_err(|e| ChromeMcpError::native_input_error(format!("Failed to create right mouse down event: {:?}", e)))?;
            
            let mouse_up = CGEvent::new_mouse_event(
                self.event_source.clone(),
                CGEventType::RightMouseUp,
                point,
                CGMouseButton::Right,
            ).map_err(|e| ChromeMcpError::native_input_error(format!("Failed to create right mouse up event: {:?}", e)))?;
            
            mouse_down.post(CGEventTapLocation::HID);
            std::thread::sleep(std::time::Duration::from_millis(50));
            mouse_up.post(CGEventTapLocation::HID);
            
            Ok(())
        }
        
        #[cfg(not(target_os = "macos"))]
        {
            Err(ChromeMcpError::native_input_error("Native input only supported on macOS"))
        }
    }

    /// Double-click at screen coordinates
    pub fn double_click_at(&self, x: f64, y: f64) -> Result<()> {
        debug!("Native double-click at ({}, {})", x, y);
        
        #[cfg(target_os = "macos")]
        {
            let point = CGPoint::new(x, y);
            
            // First click
            let mouse_down1 = CGEvent::new_mouse_event(
                self.event_source.clone(),
                CGEventType::LeftMouseDown,
                point,
                CGMouseButton::Left,
            ).map_err(|e| ChromeMcpError::native_input_error(format!("Failed to create first mouse down event: {:?}", e)))?;
            
            let mouse_up1 = CGEvent::new_mouse_event(
                self.event_source.clone(),
                CGEventType::LeftMouseUp,
                point,
                CGMouseButton::Left,
            ).map_err(|e| ChromeMcpError::native_input_error(format!("Failed to create first mouse up event: {:?}", e)))?;
            
            // Set click count to 1 for first click
            mouse_down1.set_integer_value_field(55, 1); // kCGMouseEventClickState
            mouse_up1.set_integer_value_field(55, 1);
            
            // Second click
            let mouse_down2 = CGEvent::new_mouse_event(
                self.event_source.clone(),
                CGEventType::LeftMouseDown,
                point,
                CGMouseButton::Left,
            ).map_err(|e| ChromeMcpError::native_input_error(format!("Failed to create second mouse down event: {:?}", e)))?;
            
            let mouse_up2 = CGEvent::new_mouse_event(
                self.event_source.clone(),
                CGEventType::LeftMouseUp,
                point,
                CGMouseButton::Left,
            ).map_err(|e| ChromeMcpError::native_input_error(format!("Failed to create second mouse up event: {:?}", e)))?;
            
            // Set click count to 2 for second click
            mouse_down2.set_integer_value_field(55, 2);
            mouse_up2.set_integer_value_field(55, 2);
            
            // Post all events with proper timing
            mouse_down1.post(CGEventTapLocation::HID);
            std::thread::sleep(std::time::Duration::from_millis(50));
            mouse_up1.post(CGEventTapLocation::HID);
            std::thread::sleep(std::time::Duration::from_millis(100));
            mouse_down2.post(CGEventTapLocation::HID);
            std::thread::sleep(std::time::Duration::from_millis(50));
            mouse_up2.post(CGEventTapLocation::HID);
            
            Ok(())
        }
        
        #[cfg(not(target_os = "macos"))]
        {
            Err(ChromeMcpError::native_input_error("Native input only supported on macOS"))
        }
    }

    /// Move mouse to coordinates
    pub fn move_to(&self, x: f64, y: f64) -> Result<()> {
        debug!("Native mouse move to ({}, {})", x, y);
        
        #[cfg(target_os = "macos")]
        {
            let point = CGPoint::new(x, y);
            
            let mouse_move = CGEvent::new_mouse_event(
                self.event_source.clone(),
                CGEventType::MouseMoved,
                point,
                CGMouseButton::Left, // Doesn't matter for move events
            ).map_err(|e| ChromeMcpError::native_input_error(format!("Failed to create mouse move event: {:?}", e)))?;
            
            mouse_move.post(CGEventTapLocation::HID);
            
            Ok(())
        }
        
        #[cfg(not(target_os = "macos"))]
        {
            Err(ChromeMcpError::native_input_error("Native input only supported on macOS"))
        }
    }

    /// Scroll at coordinates
    pub fn scroll_at(&self, x: f64, y: f64, delta_x: i32, delta_y: i32) -> Result<()> {
        debug!("Native scroll at ({}, {}) delta=({}, {})", x, y, delta_x, delta_y);
        
        #[cfg(target_os = "macos")]
        {
            let point = CGPoint::new(x, y);
            
            // For now, we'll use a simple mouse wheel approach
            // In a full implementation, we'd need to use the correct scroll event APIs
            let scroll_event = CGEvent::new_mouse_event(
                self.event_source.clone(),
                CGEventType::ScrollWheel,
                point,
                CGMouseButton::Left, // Not used for scroll events
            ).map_err(|e| ChromeMcpError::native_input_error(format!("Failed to create scroll event: {:?}", e)))?;
            
            // Set scroll delta values (this is a simplified approach)
            // TODO: Use proper scroll wheel event creation
            
            // TODO: Set location for scroll event (not available in this API version)
            scroll_event.post(CGEventTapLocation::HID);
            
            Ok(())
        }
        
        #[cfg(not(target_os = "macos"))]
        {
            Err(ChromeMcpError::native_input_error("Native input only supported on macOS"))
        }
    }

    /// Type text using native keyboard events
    pub fn type_text(&self, text: &str) -> Result<()> {
        debug!("Native type text: {}", text);
        
        #[cfg(target_os = "macos")]
        {
            for ch in text.chars() {
                // For simplicity, we'll use Unicode key events
                // In a full implementation, we'd map characters to key codes
                let key_down = CGEvent::new_keyboard_event(
                    self.event_source.clone(),
                    0u16, // We'll use Unicode events instead
                    true,
                ).map_err(|e| ChromeMcpError::native_input_error(format!("Failed to create key down event: {:?}", e)))?;
                
                let key_up = CGEvent::new_keyboard_event(
                    self.event_source.clone(),
                    0u16,
                    false,
                ).map_err(|e| ChromeMcpError::native_input_error(format!("Failed to create key up event: {:?}", e)))?;
                
                // Set the Unicode character
                key_down.set_string(&ch.to_string());
                key_up.set_string(&ch.to_string());
                
                key_down.post(CGEventTapLocation::HID);
                std::thread::sleep(std::time::Duration::from_millis(10));
                key_up.post(CGEventTapLocation::HID);
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            
            Ok(())
        }
        
        #[cfg(not(target_os = "macos"))]
        {
            Err(ChromeMcpError::native_input_error("Native input only supported on macOS"))
        }
    }

    /// Press a key by key code
    pub fn press_key(&self, key_code: u16) -> Result<()> {
        debug!("Native key press: {}", key_code);
        
        #[cfg(target_os = "macos")]
        {
            let key_down = CGEvent::new_keyboard_event(
                self.event_source.clone(),
                key_code,
                true,
            ).map_err(|e| ChromeMcpError::native_input_error(format!("Failed to create key down event: {:?}", e)))?;
            
            let key_up = CGEvent::new_keyboard_event(
                self.event_source.clone(),
                key_code,
                false,
            ).map_err(|e| ChromeMcpError::native_input_error(format!("Failed to create key up event: {:?}", e)))?;
            
            key_down.post(CGEventTapLocation::HID);
            std::thread::sleep(std::time::Duration::from_millis(50));
            key_up.post(CGEventTapLocation::HID);
            
            Ok(())
        }
        
        #[cfg(not(target_os = "macos"))]
        {
            Err(ChromeMcpError::native_input_error("Native input only supported on macOS"))
        }
    }

    /// Key codes for common keys (macOS virtual key codes)
    pub fn key_codes() -> NativeKeycodes {
        NativeKeycodesData::new()
    }
}

/// Common key codes for macOS
pub struct NativeKeycodesData;

impl Default for NativeKeycodesData {
    fn default() -> Self {
        Self::new()
    }
}

impl NativeKeycodesData {
    pub fn new() -> Self {
        Self
    }

    pub const RETURN: u16 = 36;
    pub const TAB: u16 = 48;
    pub const SPACE: u16 = 49;
    pub const DELETE: u16 = 51;
    pub const ESCAPE: u16 = 53;
    pub const COMMAND: u16 = 55;
    pub const SHIFT: u16 = 56;
    pub const CAPS_LOCK: u16 = 57;
    pub const OPTION: u16 = 58;
    pub const CONTROL: u16 = 59;
    pub const RIGHT_SHIFT: u16 = 60;
    pub const RIGHT_OPTION: u16 = 61;
    pub const RIGHT_CONTROL: u16 = 62;
    pub const FUNCTION: u16 = 63;
    pub const F17: u16 = 64;
    pub const VOLUME_UP: u16 = 72;
    pub const VOLUME_DOWN: u16 = 73;
    pub const MUTE: u16 = 74;
    pub const F18: u16 = 79;
    pub const F19: u16 = 80;
    pub const F20: u16 = 90;
    pub const F5: u16 = 96;
    pub const F6: u16 = 97;
    pub const F7: u16 = 98;
    pub const F3: u16 = 99;
    pub const F8: u16 = 100;
    pub const F9: u16 = 101;
    pub const F11: u16 = 103;
    pub const F13: u16 = 105;
    pub const F16: u16 = 106;
    pub const F14: u16 = 107;
    pub const F10: u16 = 109;
    pub const F12: u16 = 111;
    pub const F15: u16 = 113;
    pub const HELP: u16 = 114;
    pub const HOME: u16 = 115;
    pub const PAGE_UP: u16 = 116;
    pub const FORWARD_DELETE: u16 = 117;
    pub const F4: u16 = 118;
    pub const END: u16 = 119;
    pub const F2: u16 = 120;
    pub const PAGE_DOWN: u16 = 121;
    pub const F1: u16 = 122;
    pub const LEFT_ARROW: u16 = 123;
    pub const RIGHT_ARROW: u16 = 124;
    pub const DOWN_ARROW: u16 = 125;
    pub const UP_ARROW: u16 = 126;
}

pub type NativeKeycodes = NativeKeycodesData;

impl Default for NativeInputManager {
    fn default() -> Self {
        Self::new().expect("Failed to create native input manager")
    }
}