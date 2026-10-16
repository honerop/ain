// Native GTK4 desktop host for AIN's Rust backend.
// No browser, local HTTP server, HTML engine, or third-party Rust crate.
use std::ffi::{c_char, c_void, CStr, CString};
use std::process::{Command, Stdio};
use std::io::Write;

#[link(name = "gtk-4")]
unsafe extern "C" {
    fn gtk_application_new(id: *const c_char, flags: i32) -> *mut c_void;
    fn gtk_application_window_new(app: *mut c_void) -> *mut c_void;
    fn gtk_window_set_title(window: *mut c_void, title: *const c_char);
    fn gtk_window_set_default_size(window: *mut c_void, width: i32, height: i32);
    fn gtk_window_set_child(window: *mut c_void, child: *mut c_void);
    fn gtk_window_present(window: *mut c_void);
    fn gtk_box_new(orientation: i32, spacing: i32) -> *mut c_void;
    fn gtk_box_append(container: *mut c_void, child: *mut c_void);
    fn gtk_label_new(text: *const c_char) -> *mut c_void;
    fn gtk_label_set_text(label: *mut c_void, text: *const c_char);
    fn gtk_label_set_xalign(label: *mut c_void, x: f32);
    fn gtk_entry_new() -> *mut c_void;
    fn gtk_entry_set_placeholder_text(entry: *mut c_void, text: *const c_char);
    fn gtk_editable_get_text(editable: *mut c_void) -> *const c_char;
    fn gtk_editable_set_text(editable: *mut c_void, text: *const c_char);
    fn gtk_button_new_with_label(label: *const c_char) -> *mut c_void;
    fn gtk_combo_box_text_new() -> *mut c_void;
    fn gtk_combo_box_text_append_text(combo: *mut c_void, text: *const c_char);
    fn gtk_combo_box_set_active(combo: *mut c_void, index: i32);
    fn gtk_combo_box_get_active(combo: *mut c_void) -> i32;
    fn gtk_scrolled_window_new() -> *mut c_void;
    fn gtk_scrolled_window_set_child(window: *mut c_void, child: *mut c_void);
    fn gtk_scrolled_window_get_vadjustment(window: *mut c_void) -> *mut c_void;
    fn gtk_adjustment_get_value(adj: *mut c_void) -> f64;
    fn gtk_adjustment_get_upper(adj: *mut c_void) -> f64;
    fn gtk_adjustment_get_page_size(adj: *mut c_void) -> f64;
    fn gtk_adjustment_set_value(adj: *mut c_void, value: f64);
    fn gtk_text_view_new() -> *mut c_void;
    fn gtk_text_view_get_buffer(view: *mut c_void) -> *mut c_void;
    fn gtk_text_view_set_editable(view: *mut c_void, editable: i32);
    fn gtk_text_view_set_cursor_visible(view: *mut c_void, visible: i32);
    fn gtk_text_view_set_wrap_mode(view: *mut c_void, mode: i32);
    fn gtk_text_buffer_set_text(buffer: *mut c_void, text: *const c_char, len: i32);
    fn gtk_widget_set_hexpand(widget: *mut c_void, expand: i32);
    fn gtk_widget_set_vexpand(widget: *mut c_void, expand: i32);
    fn gtk_widget_set_margin_start(widget: *mut c_void, margin: i32);
    fn gtk_widget_set_margin_end(widget: *mut c_void, margin: i32);
    fn gtk_widget_set_margin_top(widget: *mut c_void, margin: i32);
    fn gtk_widget_set_margin_bottom(widget: *mut c_void, margin: i32);
    fn gtk_widget_add_css_class(widget: *mut c_void, name: *const c_char);
    fn gtk_widget_grab_focus(widget: *mut c_void) -> i32;
    fn gtk_widget_has_focus(widget: *mut c_void) -> i32;
    fn gtk_event_controller_key_new() -> *mut c_void;
    fn gtk_widget_add_controller(widget: *mut c_void, controller: *mut c_void);
    fn gtk_event_controller_set_propagation_phase(controller: *mut c_void, phase: i32);
}
#[link(name = "gobject-2.0")]
unsafe extern "C" {
    fn g_signal_connect_data(
        instance: *mut c_void, signal: *const c_char, callback: *const c_void,
        user_data: *mut c_void, destroy: *const c_void, flags: i32
    ) -> u64;
    fn g_object_unref(object: *mut c_void);
}
#[link(name = "gio-2.0")]
unsafe extern "C" {
    fn g_application_run(app: *mut c_void, argc: i32, argv: *mut *mut c_char) -> i32;
}
#[link(name = "glib-2.0")]
unsafe extern "C" {
    fn g_idle_add(callback: Option<unsafe extern "C" fn(*mut c_void) -> i32>, data: *mut c_void) -> u32;
}
fn ainex_c(text: &str) -> CString {
    CString::new(text.replace('\0', " ")).expect("NUL removed")
}
fn ainex_json(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""), '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"), '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c <= '\u{001f}' => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
fn ainex_model(model: &str) -> Result<(), String> {
    if model.len() > 128 || !model.chars().all(|c|
        c.is_ascii_alphanumeric() || matches!(c, '.' | '/' | '-' | '_' | ':')
    ) { return Err("Invalid model name".into()); }
    Ok(())
}
fn ainex_deepinfra(system: &str, prompt: &str, model: &str) -> Result<String, String> {
    let token = std::env::var("DEEPINFRA_API_KEY")
        .map_err(|_| "Set DEEPINFRA_API_KEY before launching Ainex".to_string())?;
    let model = if model.is_empty() { "meta-llama/Meta-Llama-3.1-8B-Instruct" } else { model };
    ainex_model(model)?;
    let payload = format!(
        "{{\"model\":{},\"messages\":[{{\"role\":\"system\",\"content\":{}}},{{\"role\":\"user\",\"content\":{}}}]}}",
        ainex_json(model), ainex_json(system), ainex_json(prompt)
    );
    let mut curl = Command::new("curl")
        .args(["--silent", "--show-error", "--fail-with-body", "--max-time", "120",
            "--request", "POST", "https://api.deepinfra.com/v1/openai/chat/completions",
            "--header", "Content-Type: application/json",
            "--header", &format!("Authorization: Bearer {token}"), "--data-binary", "@-"])
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()
        .map_err(|e| format!("curl could not start: {e}"))?;
    curl.stdin.take().ok_or("No curl stdin")?.write_all(payload.as_bytes())
        .map_err(|e| format!("Sending DeepInfra request failed: {e}"))?;
    let output = curl.wait_with_output().map_err(|e| format!("curl failed: {e}"))?;
    if !output.status.success() {
        return Err(format!("DeepInfra request failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()));
    }
    let mut jq = Command::new("jq").args(["-er",
        ".choices[0].message.content | select(type == \"string\")"])
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()
        .map_err(|e| format!("jq could not start: {e}"))?;
    jq.stdin.take().ok_or("No jq stdin")?.write_all(&output.stdout)
        .map_err(|e| format!("Parsing response failed: {e}"))?;
    let answer = jq.wait_with_output().map_err(|e| format!("jq failed: {e}"))?;
    if !answer.status.success() {
        return Err(format!("Unexpected DeepInfra response: {}",
            String::from_utf8_lossy(&output.stdout).chars().take(400).collect::<String>()));
    }
    Ok(String::from_utf8_lossy(&answer.stdout).trim_end().to_string())
}
fn ainex_chatgpt(prompt: &str, model: &str) -> Result<String, String> {
    ainex_model(model)?;
    // Authentication belongs to the official Codex CLI. Never parse its auth files.
    let path = std::env::temp_dir().join(format!(
        "ainex-{}-{}.txt", std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?.as_nanos()
    ));
    let mut command = Command::new("codex");
    command.args(["exec", "--skip-git-repo-check", "--sandbox", "read-only", "--output-last-message"])
        .arg(&path);
    if !model.is_empty() { command.args(["--model", model]); }
    command.arg("-").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn()
        .map_err(|e| format!("Cannot run Codex ({e}). Install Codex and run 'codex login' first."))?;
    child.stdin.take().ok_or("No Codex stdin")?.write_all(prompt.as_bytes())
        .map_err(|e| format!("Sending prompt failed: {e}"))?;
    let output = child.wait_with_output().map_err(|e| format!("Codex failed: {e}"))?;
    let reply = std::fs::read_to_string(&path);
    let _ = std::fs::remove_file(path);
    if !output.status.success() {
        return Err(format!("Codex failed: {}",
            String::from_utf8_lossy(&output.stderr).chars().take(500).collect::<String>()));
    }
    reply.map_err(|e| format!("Codex returned no answer: {e}"))
}
struct AinexSettings {
    title: String,
    system: String,
    welcome: String,
}
struct AinexUi {
    transcript: *mut c_void,
    scroller: *mut c_void,
    composer: *mut c_void,
    provider: *mut c_void,
    model: *mut c_void,
    status: *mut c_void,
    history: String,
    system: String,
    previous_g: bool,
    busy: bool,
}
struct AinexResult {
    ui: usize,
    message: Result<String, String>,
}
unsafe fn ainex_label(value: &str) -> *mut c_void {
    gtk_label_new(ainex_c(value).as_ptr())
}
unsafe fn ainex_append(ui: &mut AinexUi, label: &str, message: &str) {
    if !ui.history.is_empty() { ui.history.push_str("\n\n"); }
    ui.history.push_str(label);
    ui.history.push_str("\n");
    ui.history.push_str(message);
    gtk_text_buffer_set_text(gtk_text_view_get_buffer(ui.transcript),
        ainex_c(&ui.history).as_ptr(), -1);
    let adjustment = gtk_scrolled_window_get_vadjustment(ui.scroller);
    // GTK lays out the new text on a later frame; the next user action can
    // scroll further. Do not drive GTK widgets from a worker thread.
    gtk_adjustment_set_value(adjustment,
        gtk_adjustment_get_upper(adjustment) - gtk_adjustment_get_page_size(adjustment));
}
unsafe extern "C" fn ainex_result_on_main(data: *mut c_void) -> i32 {
    let returned = Box::from_raw(data as *mut AinexResult);
    let ui = &mut *(returned.ui as *mut AinexUi);
    ui.busy = false;
    match returned.message {
        Ok(response) => {
            ainex_append(ui, "AINEX", response.trim_end());
            gtk_label_set_text(ui.status, ainex_c("NORMAL   •   ready").as_ptr());
        }
        Err(message) => {
            ainex_append(ui, "ERROR", &message);
            gtk_label_set_text(ui.status, ainex_c("NORMAL   •   request failed").as_ptr());
        }
    }
    0
}
unsafe fn ainex_send(ui_ptr: *mut AinexUi) {
    let ui = &mut *ui_ptr;
    if ui.busy { return; }
    let message = CStr::from_ptr(gtk_editable_get_text(ui.composer))
        .to_string_lossy().trim().to_string();
    if message.is_empty() { return; }
    gtk_editable_set_text(ui.composer, ainex_c("").as_ptr());
    if message.starts_with(':') {
        match message.as_str() {
            ":clear" => {
                ui.history.clear();
                gtk_text_buffer_set_text(gtk_text_view_get_buffer(ui.transcript), ainex_c("").as_ptr(), 0);
            }
            ":deepinfra" => gtk_combo_box_set_active(ui.provider, 0),
            ":chatgpt" => gtk_combo_box_set_active(ui.provider, 1),
            ":help" => ainex_append(ui, "KEYS", "i: insert  Esc: normal  j/k: scroll  gg/G: top/bottom  :clear  :chatgpt  :deepinfra  :model NAME"),
            _ if message.starts_with(":model ") =>
                gtk_editable_set_text(ui.model, ainex_c(message[7..].trim()).as_ptr()),
            _ => ainex_append(ui, "ERROR", "Unknown command. Type :help"),
        }
        return;
    }
    ainex_append(ui, "YOU", &message);
    ui.busy = true;
    gtk_label_set_text(ui.status, ainex_c("INSERT   •   thinking…").as_ptr());
    let system = ui.system.clone();
    let provider = gtk_combo_box_get_active(ui.provider);
    let model = CStr::from_ptr(gtk_editable_get_text(ui.model))
        .to_string_lossy().trim().to_string();
    let ui_address = ui_ptr as usize;
    std::thread::spawn(move || {
        let response = if provider == 1 {
            ainex_chatgpt(&format!("{}\n\nUser request:\n{}", system, message), &model)
        } else { ainex_deepinfra(&system, &message, &model) };
        let result = Box::new(AinexResult { ui: ui_address, message: response });
        unsafe {
            g_idle_add(Some(ainex_result_on_main), Box::into_raw(result) as *mut c_void);
        }
    });
}
unsafe extern "C" fn ainex_send_clicked(
    _button: *mut c_void, state: *mut c_void
) {
    ainex_send(state as *mut AinexUi);
}
unsafe extern "C" fn ainex_composer_activate(
    _entry: *mut c_void, state: *mut c_void
) {
    ainex_send(state as *mut AinexUi);
}
unsafe extern "C" fn ainex_key_pressed(
    _controller: *mut c_void, key: u32, _keycode: u32, _modifiers: u32,
    state: *mut c_void,
) -> i32 {
    let ui = &mut *(state as *mut AinexUi);
    if key == 0xff1b { // Escape, including while the composer is focused
        gtk_widget_grab_focus(ui.transcript);
        gtk_label_set_text(ui.status, ainex_c("NORMAL").as_ptr());
        ui.previous_g = false;
        return 1;
    }
    if gtk_widget_has_focus(ui.composer) != 0 || gtk_widget_has_focus(ui.model) != 0 {
        return 0;
    }
    if key == 'i' as u32 || key == ':' as u32 {
        if key == ':' as u32 { gtk_editable_set_text(ui.composer, ainex_c(":").as_ptr()); }
        gtk_widget_grab_focus(ui.composer);
        gtk_label_set_text(ui.status, ainex_c("INSERT").as_ptr());
        return 1;
    }
    let adjustment = gtk_scrolled_window_get_vadjustment(ui.scroller);
    let top = gtk_adjustment_get_upper(adjustment) - gtk_adjustment_get_page_size(adjustment);
    match key {
        0x6a => gtk_adjustment_set_value(adjustment, gtk_adjustment_get_value(adjustment) + 55.0),
        0x6b => gtk_adjustment_set_value(adjustment, gtk_adjustment_get_value(adjustment) - 55.0),
        0x47 => gtk_adjustment_set_value(adjustment, top),
        0x67 if ui.previous_g => gtk_adjustment_set_value(adjustment, 0.0),
        0x67 => { ui.previous_g = true; return 1; },
        _ => { ui.previous_g = false; return 0; }
    }
    ui.previous_g = false;
    1
}
unsafe extern "C" fn ainex_activate(app: *mut c_void, config_ptr: *mut c_void) {
    let config = &*(config_ptr as *mut AinexSettings);
    let window = gtk_application_window_new(app);
    gtk_window_set_title(window, ainex_c(&config.title).as_ptr());
    gtk_window_set_default_size(window, 940, 660);
    let root = gtk_box_new(1, 14);
    for apply in [
        gtk_widget_set_margin_top as unsafe extern "C" fn(*mut c_void, i32),
        gtk_widget_set_margin_bottom, gtk_widget_set_margin_start, gtk_widget_set_margin_end
    ] { apply(root, 18); }
    let header = gtk_box_new(0, 10);
    let title = ainex_label(&config.title);
    gtk_widget_add_css_class(title, ainex_c("title-2").as_ptr());
    gtk_label_set_xalign(title, 0.0);
    gtk_widget_set_hexpand(title, 1);
    gtk_box_append(header, title);
    let provider = gtk_combo_box_text_new();
    gtk_combo_box_text_append_text(provider, ainex_c("DeepInfra").as_ptr());
    gtk_combo_box_text_append_text(provider, ainex_c("ChatGPT / Codex").as_ptr());
    gtk_combo_box_set_active(provider, 0);
    gtk_box_append(header, provider);
    let model = gtk_entry_new();
    gtk_entry_set_placeholder_text(model, ainex_c("model (optional)").as_ptr());
    gtk_box_append(header, model);
    gtk_box_append(root, header);

    let scroller = gtk_scrolled_window_new();
    gtk_widget_set_vexpand(scroller, 1);
    let transcript = gtk_text_view_new();
    gtk_widget_set_hexpand(transcript, 1);
    gtk_text_view_set_editable(transcript, 0);
    gtk_text_view_set_cursor_visible(transcript, 0);
    gtk_text_view_set_wrap_mode(transcript, 3);
    gtk_widget_add_css_class(transcript, ainex_c("monospace").as_ptr());
    gtk_scrolled_window_set_child(scroller, transcript);
    gtk_box_append(root, scroller);

    let footer = gtk_box_new(0, 8);
    let composer = gtk_entry_new();
    gtk_widget_set_hexpand(composer, 1);
    gtk_entry_set_placeholder_text(composer, ainex_c("Press i to type, Enter to send…").as_ptr());
    gtk_box_append(footer, composer);
    let button = gtk_button_new_with_label(ainex_c("Send").as_ptr());
    gtk_box_append(footer, button);
    gtk_box_append(root, footer);
    let status = ainex_label("NORMAL   •   i insert  Esc normal  j/k scroll  :help");
    gtk_label_set_xalign(status, 0.0);
    gtk_widget_add_css_class(status, ainex_c("dim-label").as_ptr());
    gtk_box_append(root, status);

    let ui = Box::new(AinexUi {
        transcript, scroller, composer, provider, model, status,
        history: String::new(), system: config.system.clone(),
        previous_g: false, busy: false,
    });
    // Lives until process exit. Worker results never dereference a dropped UI.
    let ui_ptr = Box::into_raw(ui);
    if !config.welcome.is_empty() {
        ainex_append(&mut *ui_ptr, "AINEX", &config.welcome);
    }
    g_signal_connect_data(composer, ainex_c("activate").as_ptr(),
        ainex_composer_activate as *const c_void, ui_ptr.cast(), std::ptr::null(), 0);
    g_signal_connect_data(button, ainex_c("clicked").as_ptr(),
        ainex_send_clicked as *const c_void, ui_ptr.cast(), std::ptr::null(), 0);
    let keys = gtk_event_controller_key_new();
    gtk_event_controller_set_propagation_phase(keys, 1); // GTK_PHASE_CAPTURE
    g_signal_connect_data(keys, ainex_c("key-pressed").as_ptr(),
        ainex_key_pressed as *const c_void, ui_ptr.cast(), std::ptr::null(), 0);
    gtk_widget_add_controller(window, keys);
    gtk_window_set_child(window, root);
    gtk_window_present(window);
    gtk_widget_grab_focus(transcript);
}
fn ain_desktop(title: String, system: String, welcome: String) {
    unsafe {
        let app = gtk_application_new(ainex_c("site.honerop.ainex").as_ptr(), 0);
        assert!(!app.is_null(), "GTK4 is not available");
        let settings = Box::into_raw(Box::new(AinexSettings { title, system, welcome }));
        g_signal_connect_data(app, ainex_c("activate").as_ptr(),
            ainex_activate as *const c_void, settings.cast(), std::ptr::null(), 0);
        let exit = g_application_run(app, 0, std::ptr::null_mut());
        g_object_unref(app);
        drop(Box::from_raw(settings));
        if exit != 0 { eprintln!("Ainex exited with status {exit}"); }
    }
}
