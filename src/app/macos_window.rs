/// macOS-specific window helpers that require Objective-C runtime calls
/// not exposed through winit's safe API.
///
/// All functions are only compiled on macOS.

/// Set the inset (position) of the native traffic-light buttons so they are
/// centred vertically in our custom (taller-than-standard) titlebar.
///
/// winit 0.29 does not expose `setTrafficLightsInset:` through its public
/// API, so we call it directly through the Objective-C runtime.
///
/// `titlebar_height_logical` — logical pixels height of our custom titlebar.
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

    // Traffic-light circles are 12 logical px in diameter (radius = 6).
    // We want their vertical centre at titlebar_height / 2.
    // inset_y is the distance from the TOP of the window to the TOP of each circle.
    // inset_y = (titlebar_height / 2) - 6
    let inset_x: f64 = 8.0;
    let inset_y: f64 = ((titlebar_height_logical / 2.0) - 6.0).max(2.0) as f64;

    unsafe {
        use objc::runtime::Object;
        use objc::{msg_send, sel, sel_impl};

        let ns_view = ns_view as *mut Object;

        // NSView → NSWindow
        let ns_window: *mut Object = msg_send![ns_view, window];
        if ns_window.is_null() {
            return;
        }

        // setTrafficLightsInset: takes a CGPoint (two f64s on 64-bit).
        let _: () = msg_send![ns_window, setTrafficLightsInset: CGPoint { x: inset_x, y: inset_y }];
    }
}

/// CGPoint matches the C struct layout: { CGFloat x; CGFloat y; }
/// On 64-bit platforms CGFloat = f64.
#[repr(C)]
#[derive(Copy, Clone)]
struct CGPoint {
    x: f64,
    y: f64,
}

// Safety: CGPoint is a plain C struct with no pointers.
unsafe impl objc::Encode for CGPoint {
    fn encode() -> objc::Encoding {
        // struct layout: two f64 fields.
        let encoding = format!(
            "{{CGPoint={}{}}}",
            f64::encode().as_str(),
            f64::encode().as_str()
        );
        unsafe { objc::Encoding::from_str(&encoding) }
    }
}
