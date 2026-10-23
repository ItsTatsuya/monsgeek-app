#![cfg_attr(all(not(debug_assertions), not(test)), windows_subsystem = "windows")]

mod app;
mod assets;
mod backup;
mod config;
mod devices;
mod keycodes;
mod layer_lighting;
mod light_preview;
mod polling;
mod probe;
mod protocol;
mod rt_model;
mod theme;
mod ui;
mod win;

/// The app's display name.
pub const APP_NAME: &str = "Monsgeek App";

use gpui::{
    point, px, size, App, AppContext, Application, Bounds, TitlebarOptions, WindowBackgroundAppearance,
    WindowBounds, WindowOptions,
};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |name: &str| args.iter().position(|a| a == name);
    if arg("--probe").is_some() {
        if let Err(e) = probe::probe() {
            eprintln!("probe failed: {e:#}");
            std::process::exit(1);
        }
        return;
    }
    if let Some(i) = arg("--probe-stream") {
        let secs = args.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or(20);
        if let Err(e) = probe::probe_stream(secs) {
            eprintln!("stream probe failed: {e:#}");
            std::process::exit(1);
        }
        return;
    }
    let demo = arg("--demo").is_some();
    // --shot opens the window off-screen without focus, for screenshots that don't disturb the desktop.
    let shot = arg("--shot").is_some();
    // --page <name> opens the app on a given page (handy for screenshots).
    let start_page = arg("--page")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| app::Page::parse(s));

    Application::new()
        .with_assets(assets::Assets)
        .run(move |cx: &mut App| {
            assets::load_fonts(cx);
            cx.on_window_closed(|cx| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            let config = config::Config::load();
            let mut bounds = Bounds::centered(None, size(px(1240.), px(820.)), cx);
            if shot {
                bounds.origin = point(px(-4000.), px(0.));
            }
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some(APP_NAME.into()),
                    ..Default::default()
                }),
                window_background: WindowBackgroundAppearance::Opaque,
                window_min_size: Some(size(px(900.), px(600.))),
                focus: !shot,
                ..Default::default()
            };
            cx.open_window(options, move |window, cx| {
                let view = cx.new(|cx| {
                    cx.on_app_quit(|view: &mut app::AppView, cx| view.shutdown(cx))
                        .detach();
                    let mut view = app::AppView::new(config, cx);
                    view.tools.hwnd = win::hwnd_of(window);
                    view.sync_auto(cx);
                    view.start_page = start_page;
                    if demo {
                        view.connect(true, cx);
                    }
                    view
                });
                let handle = view.downgrade();
                window.on_window_should_close(cx, move |window, cx| {
                    let shutdown = handle.update(cx, |view, cx| {
                        if view.closing {
                            return None;
                        }
                        // Automatic profiles keep working from the tray.
                        if view.hides_to_tray() {
                            view.hide_to_tray(cx);
                            return None;
                        }
                        let shutdown = view.shutdown(cx);
                        view.closing = true;
                        view.transitioning = true;
                        cx.notify();
                        Some(shutdown)
                    });
                    match shutdown {
                        Ok(Some(done)) => {
                            window
                                .spawn(cx, async move |cx| {
                                    done.await;
                                    cx.update(|window, _| window.remove_window()).ok();
                                })
                                .detach();
                            false
                        }
                        Ok(None) => false,
                        Err(_) => true,
                    }
                });
                view
            })
            .expect("open window");
            if !shot {
                cx.activate(true);
            }
        });
}
