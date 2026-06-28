/// macOS-specific window helpers that require Objective-C runtime calls.
/// All functions are only compiled on macOS.

/// Helper to create an NSString from a Rust &str.
unsafe fn ns_string(s: &str) -> *mut objc::runtime::Object {
    use objc::{msg_send, sel, sel_impl, class};
    let c_str = std::ffi::CString::new(s).unwrap();
    msg_send![class!(NSString), stringWithUTF8String: c_str.as_ptr()]
}


/// Set the inset (position) of the native traffic-light buttons so they are
/// centred vertically in our custom (taller-than-standard) titlebar, and
/// configure the window background color + dark appearance so AppKit renders
/// correct visible grey unfocused button styles.
#[allow(dead_code)]
pub fn center_traffic_lights(
    window: &winit::window::Window,
    titlebar_height_logical: f32,
) {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

    // Get the raw NSView pointer from winit.
    let ns_view = match window.window_handle() {
        Ok(h) => match h.as_raw() {
            RawWindowHandle::AppKit(appkit) => appkit.ns_view.as_ptr(),
            _ => return,
        },
        Err(_) => return,
    };

    unsafe {
        use objc::runtime::Object;
        use objc::{msg_send, sel, sel_impl, class};

        let ns_view = ns_view as *mut Object;

        // NSView → NSWindow
        let ns_window: *mut Object = msg_send![ns_view, window];
        if ns_window.is_null() {
            return;
        }

        // 1. Force dark appearance for native titlebar and controls.
        // This ensures AppKit uses dark-themed traffic lights (which render as
        // visible solid grey when unfocused) instead of light-themed ones
        // (which render as near-invisible semi-transparent white on dark backgrounds).
        let dark_aqua = ns_string("NSAppearanceNameDarkAqua");
        let appearance: *mut Object = msg_send![class!(NSAppearance), appearanceNamed: dark_aqua];
        if !appearance.is_null() {
            let _: () = msg_send![ns_window, setAppearance: appearance];
        }

        // 2. Set window background to dark so AppKit has the correct color context
        // to render buttons with appropriate contrast and anti-aliasing.
        let color: *mut Object = msg_send![
            class!(NSColor),
            colorWithCalibratedRed: 30.0 / 255.0
            green: 30.0 / 255.0
            blue: 30.0 / 255.0
            alpha: 1.0
        ];
        let _: () = msg_send![ns_window, setBackgroundColor: color];

        // 3. Center the traffic lights container view vertically.
        let close_button: *mut Object = msg_send![ns_window, standardWindowButton: 0];
        if !close_button.is_null() {
            let container: *mut Object = msg_send![close_button, superview];
            if !container.is_null() {
                let mut frame: NSRect = msg_send![container, frame];
                
                // Get the frame of the container's superview (NSThemeFrame).
                let superview: *mut Object = msg_send![container, superview];
                if !superview.is_null() {
                    let superview_frame: NSRect = msg_send![superview, frame];
                    let superview_height = superview_frame.size.height;

                    // Calculate the shift needed to center the container in our custom titlebar.
                    let container_h = frame.size.height;
                    let shift_y = ((titlebar_height_logical - container_h as f32) / 2.0) as f64;

                    // Cocoa coordinates Y=0 is at the bottom.
                    frame.origin.y = superview_height - container_h - shift_y;
                    
                    let _: () = msg_send![container, setFrame: frame];
                }
            }
        }
    }
}

/// Check if the window is currently in macOS native fullscreen mode (styleMask contains NSWindowStyleMaskFullScreen).
#[allow(dead_code)]
pub fn is_fullscreen(window: &winit::window::Window) -> bool {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let ns_view = match window.window_handle() {
        Ok(h) => match h.as_raw() {
            RawWindowHandle::AppKit(appkit) => appkit.ns_view.as_ptr(),
            _ => return false,
        },
        Err(_) => return false,
    };

    unsafe {
        use objc::runtime::Object;
        use objc::{msg_send, sel, sel_impl};

        let ns_view = ns_view as *mut Object;
        let ns_window: *mut Object = msg_send![ns_view, window];
        if ns_window.is_null() {
            return false;
        }

        let style_mask: usize = msg_send![ns_window, styleMask];
        // NSWindowStyleMaskFullScreen = 1 << 14
        (style_mask & (1 << 14)) != 0
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct NSPoint {
    pub x: f64,
    pub y: f64,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct NSSize {
    pub width: f64,
    pub height: f64,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct NSRect {
    pub origin: NSPoint,
    pub size: NSSize,
}

unsafe impl objc::Encode for NSPoint {
    fn encode() -> objc::Encoding {
        let encoding = format!(
            "{{CGPoint={}{}}}",
            f64::encode().as_str(),
            f64::encode().as_str()
        );
        unsafe { objc::Encoding::from_str(&encoding) }
    }
}

unsafe impl objc::Encode for NSSize {
    fn encode() -> objc::Encoding {
        let encoding = format!(
            "{{CGSize={}{}}}",
            f64::encode().as_str(),
            f64::encode().as_str()
        );
        unsafe { objc::Encoding::from_str(&encoding) }
    }
}

unsafe impl objc::Encode for NSRect {
    fn encode() -> objc::Encoding {
        let encoding = format!(
            "{{CGRect={}{}}}",
            NSPoint::encode().as_str(),
            NSSize::encode().as_str()
        );
        unsafe { objc::Encoding::from_str(&encoding) }
    }
}
