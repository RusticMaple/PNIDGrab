use anyhow::{Context, Result};
use gtk4::prelude::*;
use libadwaita as adw;
use gtk4::glib;
use libadwaita::prelude::AdwApplicationWindowExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use crate::fetch_all;

pub fn maybe_run_helper() -> bool {
    if std::env::args().any(|a| a == "--helper") {
        let sock_path = "/tmp/pnidgrab.sock";
        let _ = std::fs::remove_file(sock_path);
        let listener = match UnixListener::bind(sock_path) {
            Ok(l) => l,
            Err(e) => {
                eprintln!("Failed to bind helper socket: {e}");
                return true;
            }
        };
        for stream in listener.incoming() {
            if let Ok(mut s) = stream {
                if let Ok(result) = fetch_all() {
                    let json = serde_json::to_string(&result).unwrap_or_else(|_| "{}".into());
                    let _ = s.write_all(json.as_bytes());
                } else {
                    let _ = s.write_all(b"{}");
                }
            }
        }
        return true;
    }
    false
}

fn get_password() -> Option<String> {
    #[cfg(target_os = "macos")] {
        let output = Command::new("/usr/bin/osascript")
            .args([
                "-e",
                r#"tell application "System Events""#,
                "-e",
                r#"activate"#,
                "-e",
                r#"set dlg to display dialog "PNIDGrab needs your administrator password to read process memory." with title "PNIDGrab" default answer "" with icon caution buttons {"OK"} default button "OK" with hidden answer"#,
                "-e",
                r#"text returned of dlg"#,
                "-e",
                r#"end tell"#,
            ])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let mut pw = String::from_utf8_lossy(&output.stdout).to_string();
        while pw.ends_with('\n') || pw.ends_with('\r') {
            pw.pop();
        }
        return if pw.is_empty() { None } else { Some(pw) };
    }

    #[cfg(target_os = "linux")] {
        if let Ok(output) = Command::new("zenity")
            .args([
                "--password",
                "--title=PNIDGrab",
                "--text=PNIDGrab needs root to read process memory.",
            ])
            .output()
        {
            if output.status.success() {
                let mut pw = String::from_utf8_lossy(&output.stdout).to_string();
                while pw.ends_with('\n') || pw.ends_with('\r') {
                    pw.pop();
                }
                if !pw.is_empty() {
                    return Some(pw);
                }
            }
        }

        if let Ok(output) = Command::new("kdialog")
            .args([
                "--title",
                "PNIDGrab",
                "--password",
                "PNIDGrab needs root to read process memory.",
            ])
            .output()
        {
            if output.status.success() {
                let mut pw = String::from_utf8_lossy(&output.stdout).to_string();
                while pw.ends_with('\n') || pw.ends_with('\r') {
                    pw.pop();
                }
                if !pw.is_empty() {
                    return Some(pw);
                }
            }
        }

        eprint!("Password (will be echoed): ");
        let _ = std::io::stderr().flush();
        let mut pw = String::new();
        if std::io::stdin().read_line(&mut pw).is_ok() {
            while pw.ends_with('\n') || pw.ends_with('\r') {
                pw.pop();
            }
            if !pw.is_empty() {
                return Some(pw);
            }
        }
        return None;
    }

    #[allow(unreachable_code)]
    None
}

fn start_privileged_helper() -> Result<()> {
    let Some(password) = get_password() else {
        anyhow::bail!("Password prompt was cancelled or failed");
    };

    let exe = std::env::current_exe().context("current_exe failed")?;

    let mut child = Command::new("sudo")
        .arg("-S")
        .arg("--")
        .arg(exe)
        .arg("--helper")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("failed to spawn sudo helper")?;

    if let Some(mut stdin) = child.stdin.take() {
        let _ = write!(stdin, "{}\n", password);
        let _ = stdin.flush();
    }

    Ok(())
}

fn request_fetch_via_helper() -> Result<crate::FetchResult> {
    let mut retries = 0;
    loop {
        match UnixStream::connect("/tmp/pnidgrab.sock") {
            Ok(mut s) => {
                let mut buf = String::new();
                s.read_to_string(&mut buf)?;
                return Ok(serde_json::from_str(&buf)?);
            }
            Err(_) if retries < 10 => {
                retries += 1;
                thread::sleep(Duration::from_millis(300));
            }
            Err(e) => return Err(e.into()),
        }
    }
}

pub fn run_app() -> Result<()> {
    if maybe_run_helper() {
        return Ok(());
    }

    if nix::unistd::geteuid().as_raw() != 0 {
        let _ = start_privileged_helper();
    }

    let app = adw::Application::builder()
        .application_id("dev.jerrysm64.pnidgrab")
        .build();

    app.connect_activate(build_ui);

    app.run();
    Ok(())
}

fn build_ui(app: &adw::Application) {
    let win = adw::ApplicationWindow::builder()
        .application(app)
        .title("PNIDGrab 3.0.0")
        .default_width(450)
        .default_height(335)
        .resizable(false)
        .build();

    let provider = gtk4::CssProvider::new();
    provider.load_from_data(
        r#"
        .large-font {
            font-size: 13px;
        }
        treeview {
            font-size: 13px;
        }
        treeview header button {
            font-size: 13px;
            font-weight: bold;
        }
        "#,
    );

    if let Some(display) = gtk4::gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    let toast_overlay = adw::ToastOverlay::new();
    
    let header_bar = adw::HeaderBar::new();

    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    vbox.set_margin_start(12);
    vbox.set_margin_end(12);
    vbox.set_margin_top(12);
    vbox.set_margin_bottom(12);

    let list_store = gtk4::ListStore::new(&[
        glib::Type::U8,
        glib::Type::STRING,
        glib::Type::U32,
        glib::Type::STRING,
        glib::Type::STRING,
    ]);

    let tree_view = gtk4::TreeView::with_model(&list_store);
    tree_view.add_css_class("large-font");

    fn add_column(tree: &gtk4::TreeView, title: &str, col_idx: i32) {
        let renderer = gtk4::CellRendererText::new();
        let column = gtk4::TreeViewColumn::new();
        column.set_title(title);
        column.pack_start(&renderer, true);
        column.add_attribute(&renderer, "text", col_idx);
        tree.append_column(&column);
    }

    add_column(&tree_view, "Player #", 0);
    add_column(&tree_view, "PID (Hex)", 1);
    add_column(&tree_view, "PID (Dec)", 2);
    add_column(&tree_view, "PNID", 3);
    add_column(&tree_view, "Name", 4);

    let scrolled = gtk4::ScrolledWindow::new();
    scrolled.set_vexpand(true);
    scrolled.set_child(Some(&tree_view));

    let session_label = gtk4::Label::new(Some("Session ID: None"));
    session_label.set_halign(gtk4::Align::Start);
    session_label.add_css_class("large-font");

    let timestamp_label = gtk4::Label::new(Some("Fetched at: -"));
    timestamp_label.set_halign(gtk4::Align::Start);
    timestamp_label.add_css_class("large-font");

    let bottom_box = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
    bottom_box.set_hexpand(true);
    bottom_box.append(&session_label);
    bottom_box.append(&timestamp_label);

    let fetch_button = gtk4::Button::with_label("Fetch");
    fetch_button.add_css_class("large-font");

    let copy_button = gtk4::Button::with_label("Copy");
    copy_button.add_css_class("large-font");

    let button_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    button_box.set_halign(gtk4::Align::End);
    button_box.append(&copy_button);
    button_box.append(&fetch_button);

    let info_button_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    info_button_box.set_hexpand(true);
    info_button_box.append(&bottom_box);
    info_button_box.append(&button_box);

    vbox.append(&scrolled);
    vbox.append(&info_button_box);

    let main_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    main_box.append(&header_bar);
    main_box.append(&vbox);

    toast_overlay.set_child(Some(&main_box));

    win.set_content(Some(&toast_overlay));

    let player_data = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let session_id_data = std::rc::Rc::new(std::cell::RefCell::new(None));
    let timestamp_data = std::rc::Rc::new(std::cell::RefCell::new(String::new()));

    let list_store_clone = list_store.clone();
    let session_label_clone = session_label.clone();
    let timestamp_label_clone = timestamp_label.clone();
    let _fetch_button_clone = fetch_button.clone();
    let player_data_clone = player_data.clone();
    let session_id_data_clone = session_id_data.clone();
    let timestamp_data_clone = timestamp_data.clone();

    let fetch_logic = move || {
        match request_fetch_via_helper().or_else(|_| fetch_all()) {
            Ok(result) => {
                list_store_clone.clear();
                let mut pd = player_data_clone.borrow_mut();
                *pd = result.players.clone();
                *session_id_data_clone.borrow_mut() = result.session_id;
                *timestamp_data_clone.borrow_mut() = result.fetched_at.format("%Y-%m-%d %H:%M:%S").to_string();

                for p in result.players.iter() {
                    let iter = list_store_clone.append();
                    list_store_clone.set(&iter, &[
                        (0, &p.index),
                        (1, &p.pid_hex),
                        (2, &p.pid_dec),
                        (3, &p.pnid),
                        (4, &p.name),
                    ]);
                }
                match result.session_id {
                    Some(sid) => session_label_clone.set_label(&format!("Session ID: {:08X} (Dec: {})", sid, sid)),
                    None => session_label_clone.set_label("Session ID: None"),
                }
                timestamp_label_clone.set_label(&format!("Fetched at: {}", result.fetched_at.format("%Y-%m-%d %H:%M:%S")));
            }
            Err(e) => eprintln!("Fetch error: {}", e),
        }
        glib::ControlFlow::Break
    };

    glib::idle_add_local(fetch_logic.clone());
    fetch_button.connect_clicked(move |_| { glib::idle_add_local(fetch_logic.clone()); });

    let player_data_copy = player_data.clone();
    let session_id_data_copy = session_id_data.clone();
    let timestamp_data_copy = timestamp_data.clone();
    let toast_overlay_clone = toast_overlay.clone();
    let win_clone = win.clone();

    copy_button.connect_clicked(move |_| {
        let mut copy_text = String::new();
        for p in player_data_copy.borrow().iter() {
            copy_text.push_str(&format!("Player {}: PID (Hex: {}, Dec: {}), PNID: {}, Name: {}\n",
                p.index, p.pid_hex, p.pid_dec, p.pnid, p.name));
        }
        if let Some(sid) = *session_id_data_copy.borrow() {
            copy_text.push_str(&format!("Session ID: {:08X} (Dec: {})\n", sid, sid));
        } else {
            copy_text.push_str("Session ID: None\n");
        }
        copy_text.push_str(&format!("Fetched at: {}\n", *timestamp_data_copy.borrow()));

        let clipboard = win_clone.clipboard();
        clipboard.set_text(&copy_text);

        let toast = adw::Toast::new("Data copied to clipboard!");
        toast.set_timeout(2);
        toast_overlay_clone.add_toast(toast);
    });

    win.show();
}
