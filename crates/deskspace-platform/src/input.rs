#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowEvent {
    Created {
        width: i32,
        height: i32,
        dpi: u32,
    },
    Paint,
    Resized {
        width: i32,
        height: i32,
    },
    MouseMove {
        x: i32,
        y: i32,
    },
    MouseDown {
        x: i32,
        y: i32,
        button: MouseButton,
    },
    MouseUp {
        x: i32,
        y: i32,
        button: MouseButton,
    },
    MouseLeave,
    /// Another window took the mouse capture mid-gesture (for example on
    /// Alt+Tab), so no matching `MouseUp` will arrive. Cancel any drag.
    CaptureLost,
    DoubleClick {
        x: i32,
        y: i32,
    },
    DpiChanged {
        dpi: u32,
    },
    Hotkey {
        id: i32,
    },
    Timer {
        id: usize,
    },
    VisibilityChanged {
        visible: bool,
    },
    CloseRequested,
}
