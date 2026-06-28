/// macOS-specific window helpers that require Objective-C runtime calls.
/// All functions are only compiled on macOS.

/// Set the inset (position) of the native traffic-light buttons so they are
/// centred vertically in our custom (taller-than-standard) titlebar.
///
/// We retrieve the buttons via public `standardWindowButton:` API and adjust
/// their frames, or adjust their container view's frame.
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

    // The standard macOS titlebar height is 22.0.
    // If our titlebar is taller, we shift the buttons down by half of the difference.
    let diff = titlebar_height_logical - 22.0;
    if diff <= 0.0 {
        return;
    }
    let shift_y = (diff / 2.0) as f64;

    unsafe {
        use objc::runtime::Object;
        use objc::{msg_send, sel, sel_impl};

        let ns_view = ns_view as *mut Object;

        // NSView → NSWindow
        let ns_window: *mut Object = msg_send![ns_view, window];
        if ns_window.is_null() {
            return;
        }

        // We shift the container view of standard buttons so they all move together.
        let close_button: *mut Object = msg_send![ns_window, standardWindowButton: 0];
        if !close_button.is_null() {
            let container: *mut Object = msg_send![close_button, superview];
            if !container.is_null() {
                let mut frame: NSRect = msg_send![container, frame];
                
                // Get the frame of the container's superview (NSThemeFrame).
                // Its height represents the total height of the window including the titlebar area.
                let superview: *mut Object = msg_send![container, superview];
                if !superview.is_null() {
                    let superview_frame: NSRect = msg_send![superview, frame];
                    
                    // Cocoa coordinates Y=0 is at the bottom.
                    // Position the container relative to the top of its superview (NSThemeFrame).
                    frame.origin.y = superview_frame.size.height - frame.size.height - shift_y;
                    
                    let _: () = msg_send![container, setFrame: frame];
                }
            }
        }
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
