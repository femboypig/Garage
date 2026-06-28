/// macOS-specific window helpers that require Objective-C runtime calls.
/// All functions are only compiled on macOS.

/// Set the inset (position) of the native traffic-light buttons so they are
/// centred vertically in our custom (taller-than-standard) titlebar, and
/// configure the window background color so AppKit renders correct non-transparent
/// unfocused button styles.
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

        // 1. Set window background to dark so AppKit uses dark-theme traffic lights
        // (which are solid grey when unfocused, instead of fading to fully transparent).
        // #1e1e1e (rgb: 30, 30, 30) matches standard dark mode themes.
        let color: *mut Object = msg_send![
            class!(NSColor),
            colorWithCalibratedRed: 30.0 / 255.0
            green: 30.0 / 255.0
            blue: 30.0 / 255.0
            alpha: 1.0
        ];
        let _: () = msg_send![ns_window, setBackgroundColor: color];

        // 2. Center the traffic lights container view vertically.
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
                    let superview_height = superview_frame.size.height;

                    // Calculate the shift needed to center the container in our custom titlebar.
                    // Instead of assuming container height is 22.0, we use its actual height dynamically.
                    let container_h = frame.size.height;
                    let shift_y = ((titlebar_height_logical - container_h as f32) / 2.0) as f64;

                    // In Cocoa coordinates, Y is 0 at the bottom.
                    // The baseline for the container to touch the top is superview_height - container_h.
                    // We subtract shift_y from this baseline to center it.
                    frame.origin.y = superview_height - container_h - shift_y;
                    
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
