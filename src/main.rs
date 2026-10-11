#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod probe;
mod tv;
use ferrite_design::prelude::*;
use gpui::{
    App, AppContext as _, ClickEvent, Context, Entity, IntoElement, KeyBinding, Render,
    Subscription, Task, Window, div, px, size,
};

struct Gambit {
    palette: Entity<CommandPalette>,
    _appearance: Subscription,
    position: tv::Position,
    status: String,
    stream: tv::Stream,
    _poll: Task<()>,
}
impl Gambit {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let stream = tv::start();
        let poll = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(200))
                    .await;
                if this
                    .update(cx, |this, cx| {
                        let mut changed = false;
                        while let Ok(event) = this.stream.events.try_recv() {
                            changed = true;
                            match event {
                                Ok(position) => {
                                    this.position = position;
                                    this.status = "LIVE".into();
                                }
                                Err(error) => this.status = error,
                            }
                        }
                        if changed {
                            cx.notify();
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        let app = Self {
            palette: cx.new(|cx| CommandPalette::new(window, cx)),
            _appearance: theme::follow_system(window),
            position: tv::Position::default(),
            status: "Connecting to Lichess TV…".into(),
            stream,
            _poll: poll,
        };
        let weak = cx.weak_entity();
        app.palette.update(cx, |palette, cx| {
            let mut commands = Vec::new();
            for scheme in SCHEMES {
                commands.push(
                    command(format!("Scheme: {}", scheme.name))
                        .group("Theme")
                        .on_run(move |_, cx| theme::set_scheme(scheme, cx)),
                );
            }
            commands.push(
                command("Toggle light / dark")
                    .group("Theme")
                    .on_run(|window, cx| {
                        let appearance = if theme::appearance(cx) == Appearance::Light {
                            Appearance::Dark
                        } else {
                            Appearance::Light
                        };
                        theme::set_appearance(appearance, window, cx);
                    }),
            );
            commands.push(
                command("Open Lichess TV")
                    .group("Gambit")
                    .on_run(move |_, cx| {
                        let _ = weak.update(cx, |this, cx| this.open_source(cx));
                    }),
            );
            palette.set_commands(commands, cx);
        });
        app
    }
    fn open_source(&mut self, cx: &mut Context<Self>) {
        let url = if self.position.game.is_empty() {
            "https://lichess.org/tv".into()
        } else {
            format!("https://lichess.org/{}", self.position.game)
        };
        cx.open_url(&url);
    }
}
impl Render for Gambit {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        if !self.position.fen.is_empty() {
            probe::record(format!("Live board: {}", self.position.players));
        }

        let squares = tv::squares(&self.position.fen).ok();
        let board = panel("Live board")
            .meta("LICHESS TV")
            .flex_1()
            .min_h_0()
            .child(
                responsive("board-room", move |room, window, cx| {
                    let p = palette(cx);
                    let mut scale = Scale::X1;
                    for candidate in [Scale::X2, Scale::X3] {
                        let tile = f32::from(display_size(candidate, window)) * 2.;
                        if tile * 9. <= f32::from(room.width)
                            && tile * 8.5 <= f32::from(room.height)
                        {
                            scale = candidate;
                        }
                    }
                    let tile = display_size(scale, window) * 2.;
                    let mut grid = div().flex().flex_col().flex_shrink_0();
                    if let Some(squares) = &squares {
                        for (rank, cells) in squares.iter().enumerate() {
                            let mut row = div().flex().flex_shrink_0().child(
                                div()
                                    .w(tile)
                                    .h(tile)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .display(Scale::X1, window)
                                    .text_color(hsla(p.fg_dim))
                                    .child((8 - rank).to_string()),
                            );
                            for (file, piece) in cells.iter().enumerate() {
                                row = row.child(
                                    div()
                                        .w(tile)
                                        .h(tile)
                                        .flex_shrink_0()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .bg(hsla(if (rank + file) % 2 == 0 {
                                            p.surface
                                        } else {
                                            p.sunken
                                        }))
                                        .display(scale, window)
                                        .text_color(hsla(if *piece == '.' {
                                            p.fg_faint
                                        } else if piece.is_uppercase() {
                                            p.fg
                                        } else {
                                            p.accent
                                        }))
                                        .child(piece.to_string()),
                                );
                            }
                            grid = grid.child(row);
                        }
                        grid =
                            grid.child(div().flex().h(tile * 0.5).child(div().w(tile)).children(
                                ('A'..='H').map(|file| {
                                    div()
                                        .w(tile)
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .display(Scale::X1, window)
                                        .text_color(hsla(p.fg_dim))
                                        .child(file.to_string())
                                }),
                            ));
                    } else {
                        grid = grid.child(
                            div()
                                .body(text::BASE)
                                .text_color(hsla(p.fg_dim))
                                .child("Connecting to the live board…"),
                        );
                    }
                    div()
                        .size_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(grid)
                        .into_any_element()
                })
                .w_full()
                .flex_1()
                .min_h_0(),
            );
        let clocks = format!(
            "White {} / Black {}",
            clock(self.position.white),
            clock(self.position.black)
        );
        let content: Vec<gpui::AnyElement> = vec![
            board.into_any_element(),
            div()
                .body(text::BASE)
                .child(self.position.players.clone())
                .into_any_element(),
            div().child(clocks).into_any_element(),
            div()
                .text_color(hsla(p.fg_dim))
                .child("White pieces: uppercase · Black pieces: accent lowercase")
                .into_any_element(),
            Button::new("open-tv")
                .label("Open Lichess TV")
                .secondary()
                .on_click(cx.listener(|this, _, _, cx| this.open_source(cx)))
                .into_any_element(),
        ];

        let menu = Button::new("commands")
            .icon(Icon::Menu)
            .ghost()
            .small()
            .tooltip("Commands · Ctrl+Shift+P")
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                this.palette
                    .update(cx, |palette, cx| palette.toggle(window, cx))
            }));
        window_frame().child(power_on_in(
            "power",
            div()
                .flex()
                .flex_col()
                .size_full()
                .bg(hsla(p.bg))
                .text_color(hsla(p.fg))
                .body(text::BASE)
                .on_action(cx.listener(|this, _: &TogglePalette, window, cx| {
                    this.palette
                        .update(cx, |palette, cx| palette.toggle(window, cx))
                }))
                .child(self.palette.clone())
                .child(title_bar("Gambit").child(menu))
                .child(
                    div()
                        .id("content")
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_h_0()
                        .gap_3()
                        .p_4()
                        .children(content),
                )
                .child(
                    status_bar()
                        .left_live(self.status.clone())
                        .right(theme::scheme(cx).name.to_uppercase()),
                ),
        ))
    }
}
fn clock(seconds: Option<u64>) -> String {
    seconds
        .map(|s| format!("{}:{:02}", s / 60, s % 60))
        .unwrap_or_else(|| "--:--".into())
}
fn smoke() -> Result<(), String> {
    let stream = tv::start();
    let position = stream
        .events
        .recv_timeout(std::time::Duration::from_secs(25))
        .map_err(|_| "TV connection timed out")??;
    tv::board(&position.fen)?;
    println!("Lichess TV: {}", position.players);
    Ok(())
}

fn main() {
    if std::env::args().any(|arg| arg == "--version") {
        println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        return;
    }
    if std::env::args().any(|arg| arg == "--smoke") {
        if let Err(error) = smoke() {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    gpui_platform::application().run(|cx: &mut App| {
        ferrite_design::init(Appearance::Dark, cx);
        theme::apply_env(cx);
        cx.bind_keys([KeyBinding::new("ctrl-shift-p", TogglePalette, None)]);
        let options =
            chrome::remembered_window_options("gambit", "Gambit", size(px(1100.), px(800.)), cx);
        cx.open_window(options, |window, cx| {
            chrome::square_corners(window);
            chrome::remember_window("gambit", window, cx);
            chrome::power_off_on_close(window, cx);
            cx.new(|cx| Gambit::new(window, cx))
        })
        .expect("failed to open window");
        cx.activate(true);
    });
}
