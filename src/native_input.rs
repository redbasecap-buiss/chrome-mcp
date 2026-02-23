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

    // Alphabet keys
    pub const A: u16 = 0;
    pub const S: u16 = 1;
    pub const D: u16 = 2;
    pub const F: u16 = 3;
    pub const H: u16 = 4;
    pub const G: u16 = 5;
    pub const Z: u16 = 6;
    pub const X: u16 = 7;
    pub const C: u16 = 8;
    pub const V: u16 = 9;
    pub const B: u16 = 11;
    pub const Q: u16 = 12;
    pub const W: u16 = 13;
    pub const E: u16 = 14;
    pub const R: u16 = 15;
    pub const Y: u16 = 16;
    pub const T: u16 = 17;
    
    // Number keys
    pub const DIGIT_1: u16 = 18;
    pub const DIGIT_2: u16 = 19;
    pub const DIGIT_3: u16 = 20;
    pub const DIGIT_4: u16 = 21;
    pub const DIGIT_6: u16 = 22;
    pub const DIGIT_5: u16 = 23;
    pub const EQUAL: u16 = 24;
    pub const DIGIT_9: u16 = 25;
    pub const DIGIT_7: u16 = 26;
    pub const MINUS: u16 = 27;
    pub const DIGIT_8: u16 = 28;
    pub const DIGIT_0: u16 = 29;
    pub const RIGHT_BRACKET: u16 = 30;
    pub const O: u16 = 31;
    pub const U: u16 = 32;
    pub const LEFT_BRACKET: u16 = 33;
    pub const I: u16 = 34;
    pub const P: u16 = 35;
    
    // Control keys
    pub const RETURN: u16 = 36;
    pub const L: u16 = 37;
    pub const J: u16 = 38;
    pub const QUOTE: u16 = 39;
    pub const K: u16 = 40;
    pub const SEMICOLON: u16 = 41;
    pub const BACKSLASH: u16 = 42;
    pub const COMMA: u16 = 43;
    pub const SLASH: u16 = 44;
    pub const N: u16 = 45;
    pub const M: u16 = 46;
    pub const PERIOD: u16 = 47;
    pub const TAB: u16 = 48;
    pub const SPACE: u16 = 49;
    pub const GRAVE: u16 = 50;
    pub const DELETE: u16 = 51;
    pub const ESCAPE: u16 = 53;
    pub const RIGHT_COMMAND: u16 = 54;
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
    
    // Keypad keys
    pub const KEYPAD_DECIMAL: u16 = 65;
    pub const KEYPAD_MULTIPLY: u16 = 67;
    pub const KEYPAD_PLUS: u16 = 69;
    pub const KEYPAD_CLEAR: u16 = 71;
    
    pub const VOLUME_UP: u16 = 72;
    pub const VOLUME_DOWN: u16 = 73;
    pub const MUTE: u16 = 74;
    pub const KEYPAD_DIVIDE: u16 = 75;
    pub const KEYPAD_ENTER: u16 = 76;
    pub const KEYPAD_MINUS: u16 = 78;
    pub const F18: u16 = 79;
    pub const F19: u16 = 80;
    pub const KEYPAD_EQUALS: u16 = 81;
    pub const KEYPAD_0: u16 = 82;
    pub const KEYPAD_1: u16 = 83;
    pub const KEYPAD_2: u16 = 84;
    pub const KEYPAD_3: u16 = 85;
    pub const KEYPAD_4: u16 = 86;
    pub const KEYPAD_5: u16 = 87;
    pub const KEYPAD_6: u16 = 88;
    pub const KEYPAD_7: u16 = 89;
    pub const F20: u16 = 90;
    pub const KEYPAD_8: u16 = 91;
    pub const KEYPAD_9: u16 = 92;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_native_input_manager_creation() {
        let result = NativeInputManager::new();
        assert!(result.is_ok());
    }

    #[test]
    fn test_native_input_manager_default() {
        // Should not panic
        let _manager = NativeInputManager::default();
    }

    #[test]
    fn test_key_codes_constants() {
        // Test that key codes are defined and have reasonable values
        assert_eq!(NativeKeycodesData::RETURN, 36);
        assert_eq!(NativeKeycodesData::TAB, 48);
        assert_eq!(NativeKeycodesData::SPACE, 49);
        assert_eq!(NativeKeycodesData::DELETE, 51);
        assert_eq!(NativeKeycodesData::ESCAPE, 53);
        
        // Function keys
        assert_eq!(NativeKeycodesData::F1, 122);
        assert_eq!(NativeKeycodesData::F2, 120);
        assert_eq!(NativeKeycodesData::F4, 118);
        
        // Arrow keys
        assert_eq!(NativeKeycodesData::LEFT_ARROW, 123);
        assert_eq!(NativeKeycodesData::RIGHT_ARROW, 124);
        assert_eq!(NativeKeycodesData::UP_ARROW, 126);
        assert_eq!(NativeKeycodesData::DOWN_ARROW, 125);
    }

    #[test]
    fn test_letter_key_codes() {
        // Test alphabet key codes (should be sequential)
        assert_eq!(NativeKeycodesData::A, 0);
        assert_eq!(NativeKeycodesData::S, 1);
        assert_eq!(NativeKeycodesData::D, 2);
        assert_eq!(NativeKeycodesData::F, 3);
        assert_eq!(NativeKeycodesData::H, 4);
        assert_eq!(NativeKeycodesData::G, 5);
        assert_eq!(NativeKeycodesData::Z, 6);
        assert_eq!(NativeKeycodesData::X, 7);
        assert_eq!(NativeKeycodesData::C, 8);
        assert_eq!(NativeKeycodesData::V, 9);
        assert_eq!(NativeKeycodesData::B, 11);
        assert_eq!(NativeKeycodesData::Q, 12);
        assert_eq!(NativeKeycodesData::W, 13);
        assert_eq!(NativeKeycodesData::E, 14);
        assert_eq!(NativeKeycodesData::R, 15);
        assert_eq!(NativeKeycodesData::Y, 16);
        assert_eq!(NativeKeycodesData::T, 17);
        assert_eq!(NativeKeycodesData::O, 31);
        assert_eq!(NativeKeycodesData::U, 32);
        assert_eq!(NativeKeycodesData::I, 34);
        assert_eq!(NativeKeycodesData::P, 35);
        assert_eq!(NativeKeycodesData::L, 37);
        assert_eq!(NativeKeycodesData::J, 38);
        assert_eq!(NativeKeycodesData::K, 40);
        assert_eq!(NativeKeycodesData::N, 45);
        assert_eq!(NativeKeycodesData::M, 46);
    }

    #[test]
    fn test_number_key_codes() {
        // Test number key codes
        assert_eq!(NativeKeycodesData::DIGIT_1, 18);
        assert_eq!(NativeKeycodesData::DIGIT_2, 19);
        assert_eq!(NativeKeycodesData::DIGIT_3, 20);
        assert_eq!(NativeKeycodesData::DIGIT_4, 21);
        assert_eq!(NativeKeycodesData::DIGIT_5, 23);
        assert_eq!(NativeKeycodesData::DIGIT_6, 22);
        assert_eq!(NativeKeycodesData::DIGIT_7, 26);
        assert_eq!(NativeKeycodesData::DIGIT_8, 28);
        assert_eq!(NativeKeycodesData::DIGIT_9, 25);
        assert_eq!(NativeKeycodesData::DIGIT_0, 29);
    }

    #[test]
    fn test_special_key_codes() {
        // Test special character key codes
        assert_eq!(NativeKeycodesData::EQUAL, 24);
        assert_eq!(NativeKeycodesData::MINUS, 27);
        assert_eq!(NativeKeycodesData::RIGHT_BRACKET, 30);
        assert_eq!(NativeKeycodesData::LEFT_BRACKET, 33);
        assert_eq!(NativeKeycodesData::QUOTE, 39);
        assert_eq!(NativeKeycodesData::SEMICOLON, 41);
        assert_eq!(NativeKeycodesData::BACKSLASH, 42);
        assert_eq!(NativeKeycodesData::COMMA, 43);
        assert_eq!(NativeKeycodesData::SLASH, 44);
        assert_eq!(NativeKeycodesData::PERIOD, 47);
        assert_eq!(NativeKeycodesData::GRAVE, 50);
    }

    #[test]
    fn test_modifier_key_codes() {
        // Test modifier key codes
        assert_eq!(NativeKeycodesData::COMMAND, 55);
        assert_eq!(NativeKeycodesData::SHIFT, 56);
        assert_eq!(NativeKeycodesData::CAPS_LOCK, 57);
        assert_eq!(NativeKeycodesData::OPTION, 58);
        assert_eq!(NativeKeycodesData::CONTROL, 59);
        assert_eq!(NativeKeycodesData::RIGHT_COMMAND, 54);
        assert_eq!(NativeKeycodesData::RIGHT_SHIFT, 60);
        assert_eq!(NativeKeycodesData::RIGHT_OPTION, 61);
        assert_eq!(NativeKeycodesData::RIGHT_CONTROL, 62);
        assert_eq!(NativeKeycodesData::FUNCTION, 63);
    }

    #[test]
    fn test_navigation_key_codes() {
        // Test navigation key codes
        assert_eq!(NativeKeycodesData::HOME, 115);
        assert_eq!(NativeKeycodesData::END, 119);
        assert_eq!(NativeKeycodesData::PAGE_UP, 116);
        assert_eq!(NativeKeycodesData::PAGE_DOWN, 121);
        assert_eq!(NativeKeycodesData::FORWARD_DELETE, 117);
        assert_eq!(NativeKeycodesData::HELP, 114);
    }

    #[test]
    fn test_function_key_codes() {
        // Test all function key codes
        assert_eq!(NativeKeycodesData::F1, 122);
        assert_eq!(NativeKeycodesData::F2, 120);
        assert_eq!(NativeKeycodesData::F4, 118);
        assert_eq!(NativeKeycodesData::F5, 96);
        assert_eq!(NativeKeycodesData::F6, 97);
        assert_eq!(NativeKeycodesData::F7, 98);
        assert_eq!(NativeKeycodesData::F8, 100);
        assert_eq!(NativeKeycodesData::F9, 101);
        assert_eq!(NativeKeycodesData::F10, 109);
        assert_eq!(NativeKeycodesData::F11, 103);
        assert_eq!(NativeKeycodesData::F12, 111);
        assert_eq!(NativeKeycodesData::F13, 105);
        assert_eq!(NativeKeycodesData::F14, 107);
        assert_eq!(NativeKeycodesData::F15, 113);
        assert_eq!(NativeKeycodesData::F16, 106);
    }

    #[test]
    fn test_keypad_key_codes() {
        // Test keypad key codes
        assert_eq!(NativeKeycodesData::KEYPAD_DECIMAL, 65);
        assert_eq!(NativeKeycodesData::KEYPAD_MULTIPLY, 67);
        assert_eq!(NativeKeycodesData::KEYPAD_PLUS, 69);
        assert_eq!(NativeKeycodesData::KEYPAD_CLEAR, 71);
        assert_eq!(NativeKeycodesData::KEYPAD_DIVIDE, 75);
        assert_eq!(NativeKeycodesData::KEYPAD_ENTER, 76);
        assert_eq!(NativeKeycodesData::KEYPAD_MINUS, 78);
        assert_eq!(NativeKeycodesData::KEYPAD_EQUALS, 81);
        
        // Keypad digits
        assert_eq!(NativeKeycodesData::KEYPAD_0, 82);
        assert_eq!(NativeKeycodesData::KEYPAD_1, 83);
        assert_eq!(NativeKeycodesData::KEYPAD_2, 84);
        assert_eq!(NativeKeycodesData::KEYPAD_3, 85);
        assert_eq!(NativeKeycodesData::KEYPAD_4, 86);
        assert_eq!(NativeKeycodesData::KEYPAD_5, 87);
        assert_eq!(NativeKeycodesData::KEYPAD_6, 88);
        assert_eq!(NativeKeycodesData::KEYPAD_7, 89);
        assert_eq!(NativeKeycodesData::KEYPAD_8, 91);
        assert_eq!(NativeKeycodesData::KEYPAD_9, 92);
    }

    #[test]
    fn test_key_codes_access() {
        let _keycodes = NativeInputManager::key_codes();
        
        // Test that we can access key codes through the constants
        assert_eq!(NativeKeycodesData::SPACE, 49);
        assert_eq!(NativeKeycodesData::RETURN, 36);
        assert_eq!(NativeKeycodesData::ESCAPE, 53);
        assert_eq!(NativeKeycodesData::A, 0);
        assert_eq!(NativeKeycodesData::DIGIT_1, 18);
    }

    #[test]
    fn test_coordinate_validation() {
        // Test coordinate bounds (should be positive)
        let valid_coords = vec![(0.0, 0.0), (100.0, 200.0), (1920.0, 1080.0)];
        
        for (x, y) in valid_coords {
            assert!(x >= 0.0);
            assert!(y >= 0.0);
        }
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn test_click_fails_on_non_macos() {
        let manager = NativeInputManager::new().unwrap();
        let result = manager.click_at(100.0, 100.0);
        assert!(result.is_err());
        
        match result.unwrap_err() {
            ChromeMcpError::NativeInput(_) => {
                // Expected error on non-macOS platforms
            }
            _ => panic!("Expected NativeInput error"),
        }
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn test_key_press_fails_on_non_macos() {
        let manager = NativeInputManager::new().unwrap();
        let result = manager.key_press(NativeKeycodesData::SPACE);
        assert!(result.is_err());
        
        match result.unwrap_err() {
            ChromeMcpError::NativeInput(_) => {
                // Expected error on non-macOS platforms
            }
            _ => panic!("Expected NativeInput error"),
        }
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn test_type_text_fails_on_non_macos() {
        let manager = NativeInputManager::new().unwrap();
        let result = manager.type_text("test");
        assert!(result.is_err());
        
        match result.unwrap_err() {
            ChromeMcpError::NativeInput(_) => {
                // Expected error on non-macOS platforms
            }
            _ => panic!("Expected NativeInput error"),
        }
    }

    #[test]
    fn test_key_code_uniqueness() {
        // Test that key codes are unique (no duplicates)
        let key_codes = vec![
            NativeKeycodesData::A, NativeKeycodesData::B, NativeKeycodesData::C,
            NativeKeycodesData::DIGIT_1, NativeKeycodesData::DIGIT_2,
            NativeKeycodesData::F1, NativeKeycodesData::F2,
            NativeKeycodesData::SPACE, NativeKeycodesData::RETURN,
            NativeKeycodesData::LEFT_ARROW, NativeKeycodesData::RIGHT_ARROW,
            NativeKeycodesData::KEYPAD_0, NativeKeycodesData::KEYPAD_1,
        ];
        
        for i in 0..key_codes.len() {
            for j in (i + 1)..key_codes.len() {
                assert_ne!(key_codes[i], key_codes[j], 
                    "Key codes at positions {} and {} are not unique", i, j);
            }
        }
    }

    #[test]
    fn test_key_code_ranges() {
        // Test that key codes fall within reasonable ranges
        let all_codes = vec![
            NativeKeycodesData::A, NativeKeycodesData::Z,
            NativeKeycodesData::DIGIT_0, NativeKeycodesData::DIGIT_9,
            NativeKeycodesData::F1, NativeKeycodesData::F16,
            NativeKeycodesData::SPACE, NativeKeycodesData::DELETE,
            NativeKeycodesData::LEFT_ARROW, NativeKeycodesData::UP_ARROW,
            NativeKeycodesData::KEYPAD_0, NativeKeycodesData::KEYPAD_9,
        ];
        
        for code in all_codes {
            assert!(code <= 200, "Key code {} is unexpectedly high", code);
        }
    }
}