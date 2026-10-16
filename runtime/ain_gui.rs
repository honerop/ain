// Generic native GTK4 GUI primitive for compiled Ain applications.
//
// Ain calls gui(title, layout) and receives "action\ntext" when the user
// presses a button. The layout is a small line-oriented widget description:
// header<TAB>text, log<TAB>text, entry<TAB>id<TAB>placeholder,
// button<TAB>id<TAB>caption, hint<TAB>text, vim<TAB>1.
// All application logic, content, and control flow stay in Ain.
use std::cell::Cell;
use std::ffi::{c_char, c_void, CStr, CString};
use std::ptr;

#[link(name = "gtk-4")]
unsafe extern "C" {
    fn gtk_init();
    fn gtk_css_provider_new() -> *mut c_void;
    fn gtk_css_provider_load_from_data(provider: *mut c_void, data: *const c_char, length: isize);
    fn gtk_style_context_add_provider_for_display(display: *mut c_void, provider: *mut c_void, priority: u32);
    fn gdk_display_get_default() -> *mut c_void;
    fn gtk_window_new() -> *mut c_void;
    fn gtk_window_set_title(window: *mut c_void, title: *const c_char);
    fn gtk_window_set_default_size(window: *mut c_void, width: i32, height: i32);
    fn gtk_window_set_child(window: *mut c_void, child: *mut c_void);
    fn gtk_window_present(window: *mut c_void);
    fn gtk_box_new(orientation: i32, spacing: i32) -> *mut c_void;
    fn gtk_box_append(container: *mut c_void, child: *mut c_void);
    fn gtk_flow_box_new() -> *mut c_void;
    fn gtk_flow_box_insert(flow_box: *mut c_void, child: *mut c_void, position: i32);
    fn gtk_flow_box_set_max_children_per_line(flow_box: *mut c_void, n: u32);
    fn gtk_flow_box_set_min_children_per_line(flow_box: *mut c_void, n: u32);
    fn gtk_flow_box_set_row_spacing(flow_box: *mut c_void, spacing: u32);
    fn gtk_flow_box_set_column_spacing(flow_box: *mut c_void, spacing: u32);
    fn gtk_label_set_selectable(label: *mut c_void, selectable: i32);
    fn gtk_window_set_focus(window: *mut c_void, focus: *mut c_void);
    fn gtk_widget_set_focusable(widget: *mut c_void, focusable: i32);
    fn gtk_label_new(text: *const c_char) -> *mut c_void;
    fn gtk_label_set_xalign(label: *mut c_void, alignment: f32);
    fn gtk_label_set_wrap(label: *mut c_void, wrap: i32);
    fn gtk_entry_new() -> *mut c_void;
    fn gtk_entry_set_placeholder_text(entry: *mut c_void, text: *const c_char);
    fn gtk_editable_get_text(entry: *mut c_void) -> *const c_char;
    fn gtk_editable_set_text(entry: *mut c_void, text: *const c_char);
    fn gtk_button_new_with_label(text: *const c_char) -> *mut c_void;
    fn gtk_progress_bar_new() -> *mut c_void;
    fn gtk_progress_bar_set_fraction(bar: *mut c_void, fraction: f64);
    fn gtk_progress_bar_set_text(bar: *mut c_void, text: *const c_char);
    fn gtk_progress_bar_set_show_text(bar: *mut c_void, show: i32);
    fn gtk_picture_new_for_filename(filename: *const c_char) -> *mut c_void;
    fn gtk_separator_new(orientation: i32) -> *mut c_void;
    fn gtk_scrolled_window_new() -> *mut c_void;
    fn gtk_scrolled_window_set_child(scroll: *mut c_void, child: *mut c_void);
    fn gtk_scrolled_window_get_vadjustment(scroll: *mut c_void) -> *mut c_void;
    fn gtk_adjustment_get_value(adjustment: *mut c_void) -> f64;
    fn gtk_adjustment_get_upper(adjustment: *mut c_void) -> f64;
    fn gtk_adjustment_get_page_size(adjustment: *mut c_void) -> f64;
    fn gtk_adjustment_set_value(adjustment: *mut c_void, value: f64);
    fn gtk_text_view_new() -> *mut c_void;
    fn gtk_text_view_get_buffer(view: *mut c_void) -> *mut c_void;
    fn gtk_text_view_set_editable(view: *mut c_void, editable: i32);
    fn gtk_text_view_set_cursor_visible(view: *mut c_void, visible: i32);
    fn gtk_text_view_set_wrap_mode(view: *mut c_void, mode: i32);
    fn gtk_text_buffer_set_text(buffer: *mut c_void, text: *const c_char, size: i32);
    fn gtk_widget_set_hexpand(widget: *mut c_void, expand: i32);
    fn gtk_widget_set_size_request(widget: *mut c_void, width: i32, height: i32);
    fn gtk_widget_set_vexpand(widget: *mut c_void, expand: i32);
    fn gtk_widget_set_margin_start(widget: *mut c_void, margin: i32);
    fn gtk_widget_set_margin_end(widget: *mut c_void, margin: i32);
    fn gtk_widget_set_margin_top(widget: *mut c_void, margin: i32);
    fn gtk_widget_set_margin_bottom(widget: *mut c_void, margin: i32);
    fn gtk_widget_grab_focus(widget: *mut c_void) -> i32;
    fn gtk_widget_has_focus(widget: *mut c_void) -> i32;
    fn gtk_widget_add_css_class(widget: *mut c_void, class: *const c_char);
    fn gtk_event_controller_key_new() -> *mut c_void;
    fn gtk_widget_add_controller(widget: *mut c_void, controller: *mut c_void);
    fn gtk_event_controller_set_propagation_phase(controller: *mut c_void, phase: i32);
}
#[link(name = "gobject-2.0")]
unsafe extern "C" {
    fn g_signal_connect_data(object: *mut c_void, name: *const c_char,
        callback: *const c_void, data: *mut c_void, destroy: *const c_void, flags: i32) -> u64;
}
#[link(name = "glib-2.0")]
unsafe extern "C" {
    fn g_main_context_iteration(context: *mut c_void, block: i32) -> i32;
}


const AIN_CYAN_CSS: &str = r#"
.ain-theme-cyan { background: #101821; color: #e0ebf3; }
.ain-theme-cyan .ain-header { font-size: 27px; font-weight: 800; color: #70e0ed; }
.ain-theme-cyan .ain-subtitle { color: #93aabb; font-size: 12px; }
.ain-theme-cyan .ain-transcript-frame {
    background: #111d28; border: 1px solid #304657; border-radius: 15px;
}
.ain-theme-cyan .ain-message-list { padding: 16px; background: #111d28; }
.ain-theme-cyan .ain-message { border-radius: 12px; padding: 12px 15px; margin-bottom: 7px; }
.ain-theme-cyan .ain-role { font-size: 11px; font-weight: 800; letter-spacing: 0.8px; }
.ain-theme-cyan .ain-message-body { font-size: 14px; color: #e5f0f6; }
.ain-theme-cyan .ain-message-user { background: #143c49; border: 1px solid #276b7a; }
.ain-theme-cyan .ain-message-user .ain-role { color: #73effa; }
.ain-theme-cyan .ain-message-assistant { background: #1c2b38; border: 1px solid #324555; }
.ain-theme-cyan .ain-message-assistant .ain-role { color: #acdcea; }
.ain-theme-cyan .ain-message-error { background: #40252c; border: 1px solid #b45d68; }
.ain-theme-cyan .ain-message-error .ain-role, .ain-theme-cyan .ain-message-error .ain-message-body { color: #ffc5ca; }
.ain-theme-cyan .ain-message-system, .ain-theme-cyan .ain-message-info {
    background: #1c2833; border: 1px solid #31414e;
}
.ain-theme-cyan .ain-message-system .ain-role, .ain-theme-cyan .ain-message-info .ain-role { color: #97aec1; }
.ain-theme-cyan .ain-composer { min-height: 44px; border-radius: 11px; font-size: 14px; }
.ain-theme-cyan .ain-composer:focus { outline-color: #33cadd; }
.ain-theme-cyan .ain-primary { background: #39d5e7; color: #091923; font-weight: 800; border-radius: 10px; }
.ain-theme-cyan .ain-provider { border-radius: 9px; }
"#;

struct AinGui {
    window: *mut c_void,
    event: Option<String>,
    entry: *mut c_void,
    scroll: *mut c_void,
    transcript: *mut c_void,
    last_g: bool,
    vim: bool,
    insert_mode: bool,
}
struct AinGuiButton {
    gui: *mut AinGui,
    id: String,
}
thread_local! {
    static AIN_GUI: Cell<*mut AinGui> = const { Cell::new(ptr::null_mut()) };
}
fn ain_gui_c(text: &str) -> CString {
    CString::new(text.replace('\0', " ")).expect("NUL removed")
}
unsafe fn ain_gui_event(gui: *mut AinGui, action: &str) {
    let input = if (*gui).entry.is_null() { String::new() } else {
        CStr::from_ptr(gtk_editable_get_text((*gui).entry)).to_string_lossy().into_owned()
    };
    (*gui).event = Some(format!("{action}\n{input}"));
}
unsafe extern "C" fn ain_gui_button_destroy(data: *mut c_void, _closure: *mut c_void) {
    drop(Box::from_raw(data as *mut AinGuiButton));
}
unsafe extern "C" fn ain_gui_button_clicked(_button: *mut c_void, data: *mut c_void) {
    let command = &*(data as *mut AinGuiButton);
    ain_gui_event(command.gui, &command.id);
}
unsafe extern "C" fn ain_gui_enter(_entry: *mut c_void, data: *mut c_void) {
    ain_gui_event(data as *mut AinGui, "send");
}
unsafe extern "C" fn ain_gui_close(_window: *mut c_void, data: *mut c_void) -> i32 {
    (*(data as *mut AinGui)).event = Some("quit\n".into());
    1 // Defer GTK destruction until application code handles quit.
}
// A modal key handler must use mode, not a transient focus check, to decide
// whether printable characters belong to the editor. GTK runs this controller
// in the capture phase, before the entry handles the same event.
unsafe extern "C" fn ain_gui_key(_controller: *mut c_void, key: u32,
        _hardware: u32, modifiers: u32, data: *mut c_void) -> i32 {
    let gui = &mut *(data as *mut AinGui);
    if !gui.vim { return 0; }

    // Clicking the entry is another way to enter insert mode.
    if !gui.entry.is_null() && gtk_widget_has_focus(gui.entry) != 0 {
        gui.insert_mode = true;
    }
    if key == 0xff1b { // Escape always returns to normal mode.
        gui.insert_mode = false;
        gui.last_g = false;
        if !gui.transcript.is_null() { gtk_widget_grab_focus(gui.transcript); }
        return 1;
    }
    // In insert mode the GTK editor gets *every* printable character,
    // including i, j, k, g, G and ':'. No Vim shortcut runs here.
    if gui.insert_mode { return 0; }
    // Do not steal Ctrl/Alt/Super shortcuts from the window or desktop.
    if modifiers & (4 | 8 | (1 << 26)) != 0 { return 0; }
    match key {
        0x69 | 0x3a => { // i or : enters the composer.
            if gui.entry.is_null() { return 0; }
            if key == 0x3a {
                gtk_editable_set_text(gui.entry, ain_gui_c(":").as_ptr());
            }
            gui.insert_mode = true;
            gui.last_g = false;
            gtk_widget_grab_focus(gui.entry);
        }
        0x6a | 0x6b | 0x47 | 0x67 => {
            if gui.scroll.is_null() { return 0; }
            if key == 0x67 && !gui.last_g { gui.last_g = true; return 1; }
            let a = gtk_scrolled_window_get_vadjustment(gui.scroll);
            let bottom = (gtk_adjustment_get_upper(a) - gtk_adjustment_get_page_size(a)).max(0.0);
            let target = match key {
                0x6a => gtk_adjustment_get_value(a) + 60.0,
                0x6b => gtk_adjustment_get_value(a) - 60.0,
                0x47 => bottom,
                _ => 0.0
            };
            gtk_adjustment_set_value(a, target.clamp(0.0, bottom));
        }
        _ => { gui.last_g = false; return 0; }
    }
    gui.last_g = false;
    1
}
unsafe fn ain_gui_create() -> *mut AinGui {
    gtk_init();
    // Optional CSS classes only affect layouts that select the cyan theme.
    let display = gdk_display_get_default();
    if !display.is_null() {
        let provider = gtk_css_provider_new();
        gtk_css_provider_load_from_data(provider, ain_gui_c(AIN_CYAN_CSS).as_ptr(), -1);
        gtk_style_context_add_provider_for_display(display, provider, 600);
    }
    let window = gtk_window_new();
    assert!(!window.is_null(), "Could not create GTK window");
    gtk_window_set_default_size(window, 960, 700);
    let gui = Box::into_raw(Box::new(AinGui {
        window, event: None, entry: ptr::null_mut(), scroll: ptr::null_mut(),
        transcript: ptr::null_mut(), last_g: false, vim: false, insert_mode: true,
    }));
    g_signal_connect_data(window, ain_gui_c("close-request").as_ptr(),
        ain_gui_close as *const c_void, gui.cast(), ptr::null(), 0);
    let keys = gtk_event_controller_key_new();
    gtk_event_controller_set_propagation_phase(keys, 1);
    g_signal_connect_data(keys, ain_gui_c("key-pressed").as_ptr(),
        ain_gui_key as *const c_void, gui.cast(), ptr::null(), 0);
    gtk_widget_add_controller(window, keys);
    gui
}
unsafe fn ain_gui_render(gui: *mut AinGui, title: &str, spec: &str) {
    gtk_window_set_title((*gui).window, ain_gui_c(title).as_ptr());
    (*gui).entry = ptr::null_mut();
    (*gui).scroll = ptr::null_mut();
    (*gui).transcript = ptr::null_mut();
    (*gui).vim = false;
    (*gui).insert_mode = true;
    (*gui).last_g = false;
    let root = gtk_box_new(1, 10);
    if spec.lines().any(|line| line == "theme\tcyan") {
        gtk_widget_add_css_class(root, ain_gui_c("ain-theme-cyan").as_ptr());
    }
    for apply in [
        gtk_widget_set_margin_top as unsafe extern "C" fn(*mut c_void, i32),
        gtk_widget_set_margin_bottom, gtk_widget_set_margin_start, gtk_widget_set_margin_end
    ] { apply(root, 18); }
    let buttons = gtk_flow_box_new();
    gtk_flow_box_set_max_children_per_line(buttons, 3);
    gtk_flow_box_set_min_children_per_line(buttons, 1);
    gtk_flow_box_set_row_spacing(buttons, 8);
    gtk_flow_box_set_column_spacing(buttons, 8);
    let mut message_list: *mut c_void = ptr::null_mut();
    for line in spec.lines() {
        let mut tokens = line.splitn(3, '\t');
        let kind = tokens.next().unwrap_or("");
        let a = tokens.next().unwrap_or("");
        let b = tokens.next().unwrap_or("");
        match kind {
            "conversation" => {
                // Messages are real GTK widgets, not a giant flattened text field.
                let scroll = gtk_scrolled_window_new();
                let list = gtk_box_new(1, 7);
                gtk_widget_add_css_class(list, ain_gui_c("ain-message-list").as_ptr());
                gtk_widget_add_css_class(scroll, ain_gui_c("ain-transcript-frame").as_ptr());
                gtk_scrolled_window_set_child(scroll, list);
                gtk_widget_set_vexpand(scroll, 1);
                gtk_widget_set_hexpand(scroll, 1);
                gtk_widget_set_focusable(scroll, 1);
                gtk_box_append(root, scroll);
                message_list = list;
                (*gui).transcript = scroll;
                (*gui).scroll = scroll;
            }
            "message" => {
                if message_list.is_null() { continue; }
                // splitn(3) keeps tabs in user text inside the body.
                let role = match a {
                    "user" => "user",
                    "error" => "error",
                    "system" => "system",
                    "assistant" => "assistant",
                    _ => "info"
                };
                let label = match role {
                    "user" => "YOU",
                    "assistant" => "AINEX",
                    "error" => "ERROR",
                    "system" => "SYSTEM",
                    _ => "INFO"
                };
                let bubble = gtk_box_new(1, 6);
                gtk_widget_add_css_class(bubble, ain_gui_c("ain-message").as_ptr());
                gtk_widget_add_css_class(bubble, ain_gui_c(&format!("ain-message-{role}")).as_ptr());
                let role_label = gtk_label_new(ain_gui_c(label).as_ptr());
                gtk_label_set_xalign(role_label, 0.0);
                gtk_widget_add_css_class(role_label, ain_gui_c("ain-role").as_ptr());
                gtk_box_append(bubble, role_label);
                let body = gtk_label_new(ain_gui_c(&b.replace("\\n", "\n")).as_ptr());
                gtk_label_set_xalign(body, 0.0);
                gtk_label_set_wrap(body, 1);
                gtk_label_set_selectable(body, 1);
                gtk_widget_add_css_class(body, ain_gui_c("ain-message-body").as_ptr());
                gtk_box_append(bubble, body);
                gtk_box_append(message_list, bubble);
            }
            "header" | "hint" => {
                let label = gtk_label_new(ain_gui_c(a).as_ptr());
                gtk_label_set_xalign(label, 0.0);
                gtk_label_set_wrap(label, 1);
                gtk_widget_add_css_class(label, ain_gui_c(
                    if kind == "header" { "ain-header" } else { "ain-subtitle" }).as_ptr());
                if kind == "header" {
                    gtk_widget_add_css_class(label, ain_gui_c("title-2").as_ptr());
                } else {
                    gtk_widget_add_css_class(label, ain_gui_c("dim-label").as_ptr());
                }
                gtk_box_append(root, label);
            }
            "log" => {
                let scroll = gtk_scrolled_window_new();
                let view = gtk_text_view_new();
                gtk_text_view_set_editable(view, 0);
                gtk_text_view_set_cursor_visible(view, 0);
                gtk_text_view_set_wrap_mode(view, 3);
                gtk_widget_add_css_class(view, ain_gui_c("monospace").as_ptr());
                gtk_widget_add_css_class(view, ain_gui_c("ain-transcript").as_ptr());
                gtk_widget_add_css_class(scroll, ain_gui_c("ain-transcript-frame").as_ptr());
                gtk_text_buffer_set_text(gtk_text_view_get_buffer(view), ain_gui_c(a).as_ptr(), -1);
                gtk_scrolled_window_set_child(scroll, view);
                gtk_widget_set_vexpand(scroll, 1);
                gtk_widget_set_hexpand(scroll, 1);
                gtk_box_append(root, scroll);
                (*gui).transcript = view;
                (*gui).scroll = scroll;
            }
            "entry" => {
                let entry = gtk_entry_new();
                gtk_widget_set_hexpand(entry, 1);
                gtk_widget_add_css_class(entry, ain_gui_c("ain-composer").as_ptr());
                gtk_entry_set_placeholder_text(entry, ain_gui_c(b).as_ptr());
                g_signal_connect_data(entry, ain_gui_c("activate").as_ptr(),
                    ain_gui_enter as *const c_void, gui.cast(), ptr::null(), 0);
                gtk_box_append(root, entry);
                (*gui).entry = entry;
            }
            "button" | "action" => {
                if a.len() > 64 || a.is_empty() { continue; }
                let button = gtk_button_new_with_label(ain_gui_c(b).as_ptr());
                if kind == "action" { gtk_widget_set_hexpand(button, 1); }
                gtk_widget_add_css_class(button, ain_gui_c(if a == "send" { "ain-primary" } else { "ain-provider" }).as_ptr());
                let action = Box::into_raw(Box::new(AinGuiButton { gui, id: a.to_string() }));
                g_signal_connect_data(button, ain_gui_c("clicked").as_ptr(),
                    ain_gui_button_clicked as *const c_void, action.cast(), ain_gui_button_destroy as *const c_void, 0);
                if kind == "action" {
                    gtk_box_append(root, button);
                } else {
                    gtk_flow_box_insert(buttons, button, -1);
                }
            }
            "progress" => {
                let percent = a.parse::<i64>().unwrap_or(0).clamp(0, 100) as f64;
                let bar = gtk_progress_bar_new();
                gtk_progress_bar_set_fraction(bar, percent / 100.0);
                let caption = if b.is_empty() { format!("{}%", percent as i64) } else { b.to_string() };
                gtk_progress_bar_set_text(bar, ain_gui_c(&caption).as_ptr());
                gtk_progress_bar_set_show_text(bar, 1);
                gtk_widget_set_hexpand(bar, 1);
                gtk_box_append(root, bar);
            }
            "separator" => {
                gtk_box_append(root, gtk_separator_new(0)); // horizontal separator
            }
            "image" => {
                if a.len() > 4096 || a.is_empty() || a.contains('\0') { continue; }
                ain_check_permission("read");
                let size = b.parse::<i32>().unwrap_or(240).clamp(32, 720);
                let picture = gtk_picture_new_for_filename(ain_gui_c(a).as_ptr());
                gtk_widget_set_size_request(picture, size, size);
                gtk_box_append(root, picture);
            }
            "vim" => (*gui).vim = a == "1",
            _ => {}
        }
    }
    gtk_box_append(root, buttons);
    gtk_window_set_child((*gui).window, root);
    gtk_window_present((*gui).window);
    // Start with a usable composer, even when Vim navigation is enabled.
    // Esc explicitly moves to the read-only transcript for normal mode.
    if !(*gui).entry.is_null() {
        // Assign the GTK window's actual focus widget. Grabbing focus before
        // the toplevel becomes active was unreliable in virtual displays.
        gtk_window_set_focus((*gui).window, (*gui).entry);
        gtk_widget_grab_focus((*gui).entry);
    }
}
fn ain_gui_instance() -> *mut AinGui {
    AIN_GUI.with(|cell| {
        let p = cell.get();
        if p.is_null() {
            let p = unsafe { ain_gui_create() };
            cell.set(p);
            p
        } else { p }
    })
}
// Paint a window but immediately return control to Ain. Call guitick while
// network or model work continues in an Ain task, then guiupdate when ready.
fn ain_guiupdate(title: String, spec: String) {
    let gui = ain_gui_instance();
    unsafe {
        (*gui).event = None;
        ain_gui_render(gui, &title, &spec);
        while g_main_context_iteration(ptr::null_mut(), 0) != 0 {}
    }
}
// Processes pending GTK events without rebuilding widgets or losing typed input.
// "tick\n" means no user event. Delay is capped at 250 ms for responsiveness.
fn ain_guitick(delay_ms: i64) -> String {
    if !(0..=250).contains(&delay_ms) {
        return "error\nGUI tick delay must be 0..250 ms".into();
    }
    let gui = AIN_GUI.with(|cell| cell.get());
    if gui.is_null() { return "error\nGUI has not been initialized".into(); }
    let until = std::time::Instant::now() + std::time::Duration::from_millis(delay_ms as u64);
    loop {
        unsafe {
            if let Some(event) = (*gui).event.take() { return event; }
            // GTK callbacks (including close-request) run on this thread.
            if g_main_context_iteration(ptr::null_mut(), 0) != 0 { continue; }
        }
        if std::time::Instant::now() >= until { return "tick\n".into(); }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}
fn ain_gui(title: String, spec: String) -> String {
    ain_guiupdate(title, spec);
    let gui = ain_gui_instance();
    unsafe {
        loop {
            if let Some(event) = (*gui).event.take() { return event; }
            g_main_context_iteration(ptr::null_mut(), 1);
        }
    }
}
