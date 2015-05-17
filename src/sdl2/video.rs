use libc::{c_int, c_float, uint32_t};
use std::ffi::{CStr, CString, NulError};
use std::marker::PhantomData;
use std::mem;
use std::ops::Deref;
use std::ptr;
use std::vec::Vec;

use event::EventPump;
use rect::Rect;
use render::RendererBuilder;
use surface::Surface;
use pixels;
use Sdl;
use SdlResult;
use num::FromPrimitive;

use get_error;

use sys::video as ll;

#[derive(Copy, Clone, PartialEq)]
pub enum GLAttr {
    GLRedSize = 0,
    GLGreenSize = 1,
    GLBlueSize = 2,
    GLAlphaSize = 3,
    GLBufferSize = 4,
    GLDoubleBuffer = 5,
    GLDepthSize = 6,
    GLStencilSize = 7,
    GLAccumRedSize = 8,
    GLAccumGreenSize = 9,
    GLAccumBlueSize = 10,
    GLAccumAlphaSize = 11,
    GLStereo = 12,
    GLMultiSampleBuffers = 13,
    GLMultiSampleSamples = 14,
    GLAcceleratedVisual = 15,
    GLRetailedBacking = 16,
    GLContextMajorVersion = 17,
    GLContextMinorVersion = 18,
    GLContextEGL = 19,
    GLContextFlags = 20,
    GLContextProfileMask = 21,
    GLShareWithCurrentContext = 22,
    GLFramebufferSRGBCapable = 23,
}

#[derive(Copy, Clone, PartialEq)]
pub enum GLProfile {
  GLCoreProfile = 0x0001,
  GLCompatibilityProfile = 0x0002,
  GLESProfile = 0x0004,
}

/// Flags for the GLContextFlags GL attribute
bitflags! {
    flags GLContexFlag: i32 {
        const GL_CONTEXT_DEBUG              = 0x0001,
        const GL_CONTEXT_FORWARD_COMPATIBLE = 0x0002,
        const GL_CONTEXT_ROBUST_ACCESS      = 0x0004,
        const GL_CONTEXT_RESET_ISOLATION    = 0x0008,
    }
}

#[derive(Clone, PartialEq)]
pub struct DisplayMode {
    pub format: u32,
    pub w: i32,
    pub h: i32,
    pub refresh_rate: i32
}

impl DisplayMode {
    pub fn new(format: u32, w: i32, h: i32, refresh_rate: i32) -> DisplayMode {
        DisplayMode {
            format: format,
            w: w,
            h: h,
            refresh_rate: refresh_rate
        }
    }

    pub fn from_ll(raw: &ll::SDL_DisplayMode) -> DisplayMode {
        DisplayMode::new(
            raw.format as u32,
            raw.w as i32,
            raw.h as i32,
            raw.refresh_rate as i32
        )
    }

    pub fn to_ll(&self) -> ll::SDL_DisplayMode {
        ll::SDL_DisplayMode {
            format: self.format as uint32_t,
            w: self.w as c_int,
            h: self.h as c_int,
            refresh_rate: self.refresh_rate as c_int,
            driverdata: ptr::null_mut()
        }
    }
}

#[derive(Copy, Clone, PartialEq)]
pub enum FullscreenType {
    FTOff = 0,
    FTTrue = 0x00000001,
    FTDesktop = 0x00001001,
}

#[derive(PartialEq, Copy, Clone)]
pub enum WindowPos {
    PosUndefined,
    PosCentered,
    Positioned(i32)
}

fn unwrap_windowpos (pos: WindowPos) -> ll::SDL_WindowPos {
    match pos {
        WindowPos::PosUndefined => ll::SDL_WINDOWPOS_UNDEFINED,
        WindowPos::PosCentered => ll::SDL_WINDOWPOS_CENTERED,
        WindowPos::Positioned(x) => x as ll::SDL_WindowPos
    }
}

#[derive(PartialEq)]
pub struct GLContext {
    raw: ll::SDL_GLContext,
    owned: bool
}

impl Drop for GLContext {
    fn drop(&mut self) {
        if self.owned {
            unsafe {
                ll::SDL_GL_DeleteContext(self.raw)
            }
        }
    }
}

pub struct Window {
    raw: *mut ll::SDL_Window,
    owned: bool
}

impl_raw_accessors!(
    (GLContext, ll::SDL_GLContext),
    (Window, *mut ll::SDL_Window)
);

impl_owned_accessors!(
    (GLContext, owned),
    (Window, owned)
);

impl_raw_constructor!(
    (Window, Window (raw: *mut ll::SDL_Window, owned: bool))
);

impl Drop for Window {
    fn drop(&mut self) {
        if self.owned {
            unsafe {
                ll::SDL_DestroyWindow(self.raw);
            }
        }
    }
}

/// The type that allows you to build windows.
pub struct WindowBuilder {
    title: CString,
    width: u32,
    height: u32,
    x: WindowPos,
    y: WindowPos,
    window_flags: u32,
    /// The window builder cannot be built on a non-main thread, so prevent cross-threaded moves and references.
    /// `!Send` and `!Sync`
    _nosendsync: PhantomData<*mut ()>
}

impl WindowBuilder {
    /// Initializes a new `WindowBuilder`.
    pub fn new(_sdl: &Sdl, title: &str, width: u32, height: u32) -> WindowBuilder {
        WindowBuilder {
            title: CString::new(title).unwrap(),
            width: width,
            height: height,
            x: WindowPos::PosUndefined,
            y: WindowPos::PosUndefined,
            window_flags: 0,
            _nosendsync: PhantomData
        }
    }

    /// Builds the window.
    pub fn build(&self) -> SdlResult<Window> {
        unsafe {
            if self.width >= (1<<31) || self.height >= (1<<31) {
                // SDL2 only supports int (signed 32-bit) arguments.
                Err(format!("Window is too large."))
            } else {
                let raw_width = self.width as c_int;
                let raw_height = self.height as c_int;

                let raw = ll::SDL_CreateWindow(
                        self.title.as_ptr(),
                        unwrap_windowpos(self.x),
                        unwrap_windowpos(self.y),
                        raw_width,
                        raw_height,
                        self.window_flags
                );

                if raw == ptr::null_mut() {
                    Err(get_error())
                } else {
                    Ok(Window { raw: raw, owned: true })
                }
            }
        }
    }

    /// Gets the underlying window flags.
    pub fn get_window_flags(&self) -> u32 { self.window_flags }

    /// Sets the underlying window flags.
    /// This will effectively undo any previous build operations, excluding window size and position.
    pub fn set_window_flags(&mut self, flags: u32) -> &mut WindowBuilder {
        self.window_flags = flags;
        self
    }

    /// Sets the window position.
    pub fn position(&mut self, x: i32, y: i32) -> &mut WindowBuilder {
        self.x = WindowPos::Positioned(x);
        self.y = WindowPos::Positioned(y);
        self
    }

    /// Centers the window.
    pub fn position_centered(&mut self) -> &mut WindowBuilder {
        self.x = WindowPos::PosCentered;
        self.y = WindowPos::PosCentered;
        self
    }

    /// Sets the window to fullscreen.
    pub fn fullscreen(&mut self) -> &mut WindowBuilder { self.window_flags |= ll::SDL_WindowFlags::SDL_WINDOW_FULLSCREEN as u32; self }

    /// Sets the window to fullscreen at the current desktop resolution.
    pub fn fullscreen_desktop(&mut self) -> &mut WindowBuilder { self.window_flags |= ll::SDL_WindowFlags::SDL_WINDOW_FULLSCREEN_DESKTOP as u32; self }

    /// Sets the window to be usable with an OpenGL context
    pub fn opengl(&mut self) -> & mut WindowBuilder { self.window_flags |= ll::SDL_WindowFlags::SDL_WINDOW_OPENGL as u32; self }

    /// Hides the window.
    pub fn hidden(&mut self) -> &mut WindowBuilder { self.window_flags |= ll::SDL_WindowFlags::SDL_WINDOW_HIDDEN as u32; self }

    /// Removes the window decoration.
    pub fn borderless(&mut self) -> &mut WindowBuilder { self.window_flags |= ll::SDL_WindowFlags::SDL_WINDOW_BORDERLESS as u32; self }

    /// Sets the window to be resizable.
    pub fn resizable(&mut self) -> &mut WindowBuilder { self.window_flags |= ll::SDL_WindowFlags::SDL_WINDOW_RESIZABLE as u32; self }

    /// Minimizes the window.
    pub fn minimized(&mut self) -> &mut WindowBuilder { self.window_flags |= ll::SDL_WindowFlags::SDL_WINDOW_MINIMIZED as u32; self }

    /// Maximizes the window.
    pub fn maximized(&mut self) -> &mut WindowBuilder { self.window_flags |= ll::SDL_WindowFlags::SDL_WINDOW_MAXIMIZED as u32; self }

    /// Sets the window to have grabbed input focus.
    pub fn input_grabbed(&mut self) -> &mut WindowBuilder { self.window_flags |= ll::SDL_WindowFlags::SDL_WINDOW_INPUT_GRABBED as u32; self }

    /// Creates the window in high-DPI mode if supported (>= SDL 2.0.1)
    pub fn allow_highdpi(&mut self) -> &mut WindowBuilder { self.window_flags |= ll::SDL_WindowFlags::SDL_WINDOW_ALLOW_HIGHDPI as u32; self }
}

/// Contains accessors to a `Window`'s properties.
pub struct WindowProperties<'a> {
    raw: *mut ll::SDL_Window,
    _marker: PhantomData<&'a ()>
}

/// Contains getters to a `Window`'s properties.
///
/// This type acts as an immutable guard to `WindowProperties` using `Deref`.
pub struct WindowPropertiesGetters<'a> {
    window_properties: WindowProperties<'a>
}

impl<'a> Deref for WindowPropertiesGetters<'a> {
    type Target = WindowProperties<'a>;

    fn deref(&self) -> &WindowProperties<'a> {
        &self.window_properties
    }
}

impl Window {
    /// Initializes a new `RendererBuilder`; a convenience method that calls `RendererBuilder::new()`.
    pub fn renderer(self) -> RendererBuilder {
        RendererBuilder::new(self)
    }

    /// Accesses the Window properties, such as the position, size and title of a Window.
    ///
    /// In order to access a Window's properties, it must be guaranteed that the
    /// event loop is not running.
    /// This is why a reference to the application's `EventPump` is required
    /// (a shared `EventPump` reference is only obtainable if it's not being mutated).
    /// Event pumping could otherwise mutate a Window's properties without your consent!
    ///
    /// # Example
    /// ```no_run
    /// let mut sdl_context = sdl2::init(sdl2::INIT_EVERYTHING).unwrap();
    /// let mut window = sdl_context.window("My SDL window", 800, 600).build().unwrap();
    /// let mut event_pump = sdl_context.event_pump();
    ///
    /// loop {
    ///     let mut pos = None;
    ///
    ///     for event in event_pump.poll_iter() {
    ///         use sdl2::event::Event;
    ///         match event {
    ///             Event::MouseMotion { x, y, .. } => { pos = Some((x, y)); },
    ///             _ => ()
    ///         }
    ///     }
    ///
    ///     if let Some((x, y)) = pos {
    ///         // Set the window title
    ///         window.properties(&event_pump).set_title(&format!("{}, {}", x, y));
    ///     }
    /// }
    /// ```
    pub fn properties<'a>(&'a mut self, _event: &'a EventPump) -> WindowProperties<'a> {
        WindowProperties {
            raw: self.raw,
            _marker: PhantomData
        }
    }

    pub fn properties_getters<'a>(&'a self, _event: &'a EventPump) -> WindowPropertiesGetters<'a> {
        WindowPropertiesGetters {
            window_properties: WindowProperties {
                raw: self.raw,
                _marker: PhantomData
            }
        }
    }

    /// Get a Window from a stored ID.
    ///
    /// Warning: This function is unsafe!
    /// It may introduce aliased Window values if a Window of the same ID is
    /// already being used as a variable in the application.
    pub unsafe fn from_id(id: u32) -> SdlResult<Window> {
        let raw = ll::SDL_GetWindowFromID(id);
        if raw == ptr::null_mut() {
            Err(get_error())
        } else {
            Ok(Window{ raw: raw, owned: false})
        }
    }

    pub fn get_id(&self) -> u32 {
        unsafe { ll::SDL_GetWindowID(self.raw) }
    }

    pub fn gl_create_context(&self) -> SdlResult<GLContext> {
        let result = unsafe { ll::SDL_GL_CreateContext(self.raw) };
        if result == ptr::null_mut() {
            Err(get_error())
        } else {
            Ok(GLContext{raw: result, owned: true})
        }
    }

    pub fn gl_make_current(&self, context: &GLContext) -> SdlResult<()> {
        unsafe {
            if ll::SDL_GL_MakeCurrent(self.raw, context.raw) == 0 {
                Ok(())
            } else {
                Err(get_error())
            }
        }
    }

    pub fn gl_swap_window(&self) {
        unsafe { ll::SDL_GL_SwapWindow(self.raw) }
    }
}

impl<'a> WindowProperties<'a> {
    pub fn get_display_index(&self) -> SdlResult<i32> {
        let result = unsafe { ll::SDL_GetWindowDisplayIndex(self.raw) };
        if result < 0 {
            return Err(get_error())
        } else {
            Ok(result as i32)
        }
    }

    pub fn set_display_mode(&mut self, display_mode: Option<DisplayMode>) -> SdlResult<()> {
        unsafe {
            let result = ll::SDL_SetWindowDisplayMode(
                self.raw,
                match display_mode {
                    Some(ref mode) => &mode.to_ll(),
                    None => ptr::null()
                }
            );
            if result < 0 {
                Err(get_error())
            } else {
                Ok(())
            }
        }
    }

    pub fn get_display_mode(&self) -> SdlResult<DisplayMode> {
        let mut dm = unsafe { mem::uninitialized() };

        let result = unsafe {
            ll::SDL_GetWindowDisplayMode(
                self.raw,
                &mut dm
            ) == 0
        };

        if result {
            Ok(DisplayMode::from_ll(&dm))
        } else {
            Err(get_error())
        }
    }

    pub fn get_window_pixel_format(&self) -> pixels::PixelFormatEnum {
        unsafe{ FromPrimitive::from_u64(ll::SDL_GetWindowPixelFormat(self.raw) as u64).unwrap() }
    }

    pub fn get_window_flags(&self) -> u32 {
        unsafe {
            ll::SDL_GetWindowFlags(self.raw)
        }
    }

    pub fn set_title(&mut self, title: &str) -> Result<(), NulError> {
        let buff = try!(CString::new(title)).as_ptr();
        unsafe { ll::SDL_SetWindowTitle(self.raw, buff); }
        Ok(())
    }

    pub fn get_title(&self) -> &str {
        use std::ffi::CStr;
        use std::str;

        unsafe {
            let buf = ll::SDL_GetWindowTitle(self.raw);

            // The window title must be encoded in UTF-8.
            str::from_utf8(CStr::from_ptr(buf).to_bytes()).unwrap()
        }
    }

    pub fn set_icon(&mut self, icon: &Surface) {
        unsafe { ll::SDL_SetWindowIcon(self.raw, icon.raw()) }
    }

    //pub fn SDL_SetWindowData(window: *SDL_Window, name: *c_char, userdata: *c_void) -> *c_void; //TODO: Figure out what this does
    //pub fn SDL_GetWindowData(window: *SDL_Window, name: *c_char) -> *c_void;

    pub fn set_position(&mut self, x: WindowPos, y: WindowPos) {
        unsafe { ll::SDL_SetWindowPosition(self.raw, unwrap_windowpos(x), unwrap_windowpos(y)) }
    }

    pub fn get_position(&self) -> (i32, i32) {
        let mut x: c_int = 0;
        let mut y: c_int = 0;
        unsafe { ll::SDL_GetWindowPosition(self.raw, &mut x, &mut y) };
        (x as i32, y as i32)
    }

    pub fn set_size(&mut self, w: i32, h: i32) {
        unsafe { ll::SDL_SetWindowSize(self.raw, w as c_int, h as c_int) }
    }

    pub fn get_size(&self) -> (i32, i32) {
        let mut w: c_int = 0;
        let mut h: c_int = 0;
        unsafe { ll::SDL_GetWindowSize(self.raw, &mut w, &mut h) };
        (w as i32, h as i32)
    }

    pub fn get_drawable_size(&self) -> (i32, i32) {
        let mut w: c_int = 0;
        let mut h: c_int = 0;
        unsafe { ll::SDL_GL_GetDrawableSize(self.raw, &mut w, &mut h) };
        (w as i32, h as i32)
    }

    pub fn set_minimum_size(&mut self, w: i32, h: i32) {
        unsafe { ll::SDL_SetWindowMinimumSize(self.raw, w as c_int, h as c_int) }
    }

    pub fn get_minimum_size(&self) -> (i32, i32) {
        let mut w: c_int = 0;
        let mut h: c_int = 0;
        unsafe { ll::SDL_GetWindowMinimumSize(self.raw, &mut w, &mut h) };
        (w as i32, h as i32)
    }

    pub fn set_maximum_size(&mut self, w: i32, h: i32) {
        unsafe { ll::SDL_SetWindowMaximumSize(self.raw, w as c_int, h as c_int) }
    }

    pub fn get_maximum_size(&self) -> (i32, i32) {
        let mut w: c_int = 0;
        let mut h: c_int = 0;
        unsafe { ll::SDL_GetWindowMaximumSize(self.raw, &mut w, &mut h) };
        (w as i32, h as i32)
    }

    pub fn set_bordered(&mut self, bordered: bool) {
        unsafe { ll::SDL_SetWindowBordered(self.raw, if bordered { 1 } else { 0 }) }
    }

    pub fn show(&mut self) {
        unsafe { ll::SDL_ShowWindow(self.raw) }
    }

    pub fn hide(&mut self) {
        unsafe { ll::SDL_HideWindow(self.raw) }
    }

    pub fn raise(&mut self) {
        unsafe { ll::SDL_RaiseWindow(self.raw) }
    }

    pub fn maximize(&mut self) {
        unsafe { ll::SDL_MaximizeWindow(self.raw) }
    }

    pub fn minimize(&mut self) {
        unsafe { ll::SDL_MinimizeWindow(self.raw) }
    }

    pub fn restore(&mut self) {
        unsafe { ll::SDL_RestoreWindow(self.raw) }
    }

    pub fn set_fullscreen(&mut self, fullscreen_type: FullscreenType) -> SdlResult<()> {
        unsafe {
            if ll::SDL_SetWindowFullscreen(self.raw, fullscreen_type as uint32_t) == 0 {
                Ok(())
            } else {
                Err(get_error())
            }
        }
    }

    pub fn get_surface(&mut self) -> SdlResult<Surface> {
        let raw = unsafe { ll::SDL_GetWindowSurface(self.raw) };

        if raw == ptr::null_mut() {
            Err(get_error())
        } else {
            unsafe { Ok(Surface::from_ll(raw, false)) } //Docs say that it releases with the window
        }
    }

    pub fn update_surface(&self) -> SdlResult<()> {
        unsafe {
            if ll::SDL_UpdateWindowSurface(self.raw) == 0 {
                Ok(())
            } else {
                Err(get_error())
            }
        }
    }

    pub fn update_surface_rects(&self, rects: &[Rect]) -> SdlResult<()> {
        unsafe {
            if ll::SDL_UpdateWindowSurfaceRects(self.raw, rects.as_ptr(), rects.len() as c_int) == 0 {
                Ok(())
            } else {
                Err(get_error())
            }
        }
    }

    pub fn set_grab(&mut self, grabbed: bool) {
        unsafe { ll::SDL_SetWindowGrab(self.raw, if grabbed { 1 } else { 0 }) }
    }

    pub fn get_grab(&self) -> bool {
        unsafe { ll::SDL_GetWindowGrab(self.raw) == 1 }
    }

    pub fn set_brightness(&mut self, brightness: f64) -> SdlResult<()> {
        unsafe {
            if ll::SDL_SetWindowBrightness(self.raw, brightness as c_float) == 0 {
                Ok(())
            } else {
                Err(get_error())
            }
        }
    }

    pub fn get_brightness(&self) -> f64 {
        unsafe { ll::SDL_GetWindowBrightness(self.raw) as f64 }
    }

    pub fn set_gamma_ramp(&mut self, red: Option<&[u16; 256]>, green: Option<&[u16; 256]>, blue: Option<&[u16; 256]>) -> SdlResult<()> {
        unsafe {
            let unwrapped_red = match red {
                Some(values) => values.as_ptr(),
                None => ptr::null()
            };
            let unwrapped_green = match green {
                Some(values) => values.as_ptr(),
                None => ptr::null()
            };
            let unwrapped_blue = match blue {
                Some(values) => values.as_ptr(),
                None => ptr::null()
            };

            if ll::SDL_SetWindowGammaRamp(self.raw, unwrapped_red, unwrapped_green, unwrapped_blue) == 0 {
                Ok(())
            } else {
                Err(get_error())
            }
        }
    }

    pub fn get_gamma_ramp(&self) -> SdlResult<(Vec<u16>, Vec<u16>, Vec<u16>)> {
        let mut red: Vec<u16> = Vec::with_capacity(256);
        let mut green: Vec<u16> = Vec::with_capacity(256);
        let mut blue: Vec<u16> = Vec::with_capacity(256);
        let result = unsafe {ll::SDL_GetWindowGammaRamp(self.raw, red.as_mut_ptr(), green.as_mut_ptr(), blue.as_mut_ptr()) == 0};
        if result {
            Ok((red, green, blue))
        } else {
            Err(get_error())
        }
    }
}

pub fn get_num_video_drivers() -> SdlResult<i32> {
    let result = unsafe { ll::SDL_GetNumVideoDrivers() };
    if result < 0 {
        Err(get_error())
    } else {
        Ok(result as i32)
    }
}

pub fn get_video_driver(id: i32) -> String {
    unsafe {
        let buf = ll::SDL_GetVideoDriver(id as c_int);
        String::from_utf8_lossy(CStr::from_ptr(buf).to_bytes()).to_string()
    }
}

pub fn video_init(name: &str) -> Result<bool, NulError> {
    let buf =
    match CString::new(name) {
        Ok(s) => s.as_ptr(),
        Err(e) => return Err(e),
    };
    Ok(unsafe { ll::SDL_VideoInit(buf) == 0 })
}

pub fn video_quit() {
    unsafe { ll::SDL_VideoQuit() }
}

pub fn get_current_video_driver() -> String {
    unsafe {
        let video = ll::SDL_GetCurrentVideoDriver();
        String::from_utf8_lossy(CStr::from_ptr(video).to_bytes()).to_string()
    }
}

pub fn get_num_video_displays() -> SdlResult<i32> {
    let result = unsafe { ll::SDL_GetNumVideoDisplays() };
    if result < 0 {
        Err(get_error())
    } else {
        Ok(result as i32)
    }
}

pub fn get_display_name(display_index: i32) -> String {
    unsafe {
        let display = ll::SDL_GetDisplayName(display_index as c_int);
        String::from_utf8_lossy(CStr::from_ptr(display).to_bytes()).to_string()
    }
}

pub fn get_display_bounds(display_index: i32) -> SdlResult<Rect> {
    let mut out: Rect = Rect::new(0, 0, 0, 0);
    let result = unsafe { ll::SDL_GetDisplayBounds(display_index as c_int, &mut out) == 0 };

    if result {
        Ok(out)
    } else {
        Err(get_error())
    }
}

pub fn get_num_display_modes(display_index: i32) -> SdlResult<i32> {
    let result = unsafe { ll::SDL_GetNumDisplayModes(display_index as c_int) };
    if result < 0 {
        Err(get_error())
    } else {
        Ok(result as i32)
    }
}

pub fn get_display_mode(display_index: i32, mode_index: i32) -> SdlResult<DisplayMode> {
    let mut dm = unsafe { mem::uninitialized() };
    let result = unsafe { ll::SDL_GetDisplayMode(display_index as c_int, mode_index as c_int, &mut dm) == 0};

    if result {
        Ok(DisplayMode::from_ll(&dm))
    } else {
        Err(get_error())
    }
}

pub fn get_desktop_display_mode(display_index: i32) -> SdlResult<DisplayMode> {
    let mut dm = unsafe { mem::uninitialized() };
    let result = unsafe { ll::SDL_GetDesktopDisplayMode(display_index as c_int, &mut dm) == 0};

    if result {
        Ok(DisplayMode::from_ll(&dm))
    } else {
        Err(get_error())
    }
}

pub fn get_current_display_mode(display_index: i32) -> SdlResult<DisplayMode> {
    let mut dm = unsafe { mem::uninitialized() };
    let result = unsafe { ll::SDL_GetCurrentDisplayMode(display_index as c_int, &mut dm) == 0};

    if result {
        Ok(DisplayMode::from_ll(&dm))
    } else {
        Err(get_error())
    }
}

pub fn get_closest_display_mode(display_index: i32, mode: &DisplayMode) -> SdlResult<DisplayMode> {
    let input = mode.to_ll();
    let mut dm = unsafe { mem::uninitialized() };

    let result = unsafe { ll::SDL_GetClosestDisplayMode(display_index as c_int, &input, &mut dm) };

    if result == ptr::null_mut() {
        Err(get_error())
    } else {
        Ok(DisplayMode::from_ll(&dm))
    }
}

pub fn is_screen_saver_enabled() -> bool {
    unsafe { ll::SDL_IsScreenSaverEnabled() == 1 }
}

pub fn enable_screen_saver() {
    unsafe { ll::SDL_EnableScreenSaver() }
}

pub fn disable_screen_saver() {
    unsafe { ll::SDL_DisableScreenSaver() }
}

pub fn gl_load_library(path: &str) -> SdlResult<()> {
    unsafe {
        let path = CString::new(path).unwrap().as_ptr();
        if ll::SDL_GL_LoadLibrary(path) == 0 {
            Ok(())
        } else {
            Err(get_error())
        }
    }
}

pub fn gl_unload_library() {
    unsafe { ll::SDL_GL_UnloadLibrary(); }
}

pub fn gl_get_proc_address(procname: &str) -> Option<extern "system" fn()> {
    unsafe {
        let procname =
        match CString::new(procname) {
            Ok(s) => s.as_ptr(),
            Err(_) => return None,
        };
        ll::SDL_GL_GetProcAddress(procname)
    }
}

pub fn gl_extension_supported(extension: &str) -> Result<bool, NulError> {
    let buff =
    match CString::new(extension) {
        Ok(s) => s.as_ptr(),
        Err(e) => return Err(e),
    };
    Ok(unsafe { ll::SDL_GL_ExtensionSupported(buff) == 1 })
}

pub fn gl_set_attribute(attr: GLAttr, value: i32) -> bool {
    unsafe { ll::SDL_GL_SetAttribute(FromPrimitive::from_u64(attr as u64).unwrap(), value as c_int) == 0 }
}

pub fn gl_get_attribute(attr: GLAttr) -> SdlResult<i32> {
    let mut out = 0;

    let result = unsafe { ll::SDL_GL_GetAttribute(FromPrimitive::from_u64(attr as u64).unwrap(), &mut out) } == 0;
    if result {
        Ok(out as i32)
    } else {
        Err(get_error())
    }
}

pub unsafe fn gl_get_current_window() -> SdlResult<Window> {
    let raw = ll::SDL_GL_GetCurrentWindow();
    if raw == ptr::null_mut() {
        Err(get_error())
    } else {
        Ok(Window{ raw: raw, owned: false })
    }
}

pub unsafe fn gl_get_current_context() -> SdlResult<GLContext> {
    let raw = ll::SDL_GL_GetCurrentContext();
    if raw == ptr::null_mut() {
        Err(get_error())
    } else {
        Ok(GLContext{ raw: raw, owned: false })
    }
}

pub fn gl_set_swap_interval(interval: i32) -> bool {
    unsafe { ll::SDL_GL_SetSwapInterval(interval as c_int) == 0 }
}

pub fn gl_get_swap_interval() -> i32 {
    unsafe { ll::SDL_GL_GetSwapInterval() as i32 }
}
