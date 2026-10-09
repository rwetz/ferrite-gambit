#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod tv;
use ferrite_design::prelude::*;
use gpui::{App, AppContext as _, ClickEvent, Context, Entity, IntoElement, KeyBinding, Render, Subscription, Task, Window, div, px, size};

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
                cx.background_executor().timer(std::time::Duration::from_millis(200)).await;
                if this
                    .update(cx, |this, cx| {
                        while let Ok(event) = this.stream.events.try_recv() {
                            match event {
                                Ok(position) => {
                                    this.position = position;
                                    this.status = "LIVE".into();
                                }
                                Err(error) => this.status = error,
                            }
                        }
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        let mut app = Self {
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
                commands
                    .push(command(format!("Scheme: {}", scheme.name)).group("Theme").on_run(move |_, cx| theme::set_scheme(scheme, cx)));
            }
            commands.push(command("Toggle light / dark").group("Theme").on_run(|window, cx| {
                let appearance = if theme::appearance(cx) == Appearance::Light { Appearance::Dark } else { Appearance::Light };
                theme::set_appearance(appearance, window, cx);
            }));
            commands.push(command("Open Lichess TV").group("Gambit").on_run(move |_, cx| {
                let _ = weak.update(cx, |this, cx| this.refresh(cx));
            }));
            palette.set_commands(commands, cx);
        });
        app.refresh(cx);
        app
    }
    fn refresh(&mut self, cx: &mut Context<Self>) {
        let url = if self.position.game.is_empty() {
            "https://lichess.org/tv".into()
        } else {
            format!("https://lichess.org/{}", self.position.game)
        };
        cx.open_url(&url);
    }
}
impl Render for Gambit {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);

        let rows = tv::board(&self.position.fen).unwrap_or_else(|_| vec!["Waiting for the first position…".into()]);
        let board = panel("Live board")
            .meta("UPPERCASE = WHITE · LOWERCASE = BLACK")
            .child(div().flex().flex_col().gap_2().p_4().children(rows.into_iter().map(|row| div().display(Scale::X3, window).child(row))));
        let clocks = format!(
            "White {}s / Black {}s",
            self.position.white.map(|s| s.to_string()).unwrap_or_else(|| "--".into()),
            self.position.black.map(|s| s.to_string()).unwrap_or_else(|| "--".into())
        );
        let content: Vec<gpui::AnyElement> = vec![
            board.into_any_element(),
            div().body(text::BASE).child(self.position.players.clone()).into_any_element(),
            div().child(clocks).into_any_element(),
            Button::new("open-tv")
                .label("Open Lichess TV")
                .secondary()
                .on_click(cx.listener(|this, _, _, cx| this.refresh(cx)))
                .into_any_element(),
        ];

        let menu =
            Button::new("commands").icon(Icon::Menu).ghost().small().tooltip("Commands · Ctrl+Shift+P").on_click(
                cx.listener(|this, _: &ClickEvent, window, cx| this.palette.update(cx, |palette, cx| palette.toggle(window, cx))),
            );
        window_frame().child(power_on_in(
            "power",
            div()
                .flex()
                .flex_col()
                .size_full()
                .bg(hsla(p.bg))
                .text_color(hsla(p.fg))
                .body(text::BASE)
                .on_action(
                    cx.listener(|this, _: &TogglePalette, window, cx| this.palette.update(cx, |palette, cx| palette.toggle(window, cx))),
                )
                .child(self.palette.clone())
                .child(title_bar("Gambit").child(menu))
                .child(div().id("content").flex().flex_col().flex_1().min_h_0().overflow_y_scroll().gap_3().p_4().children(content))
                .child(status_bar().left_live(self.status.clone()).right(theme::scheme(cx).name.to_uppercase())),
        ))
    }
}
fn smoke() -> Result<(), String> {
    let stream = tv::start();
    let position = stream.events.recv_timeout(std::time::Duration::from_secs(25)).map_err(|_| "TV connection timed out")??;
    tv::board(&position.fen)?;
    println!("Lichess TV: {}", position.players);
    Ok(())
}

fn main() {
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
        let options = chrome::window_options("Gambit", size(px(1100.), px(800.)), cx);
        cx.open_window(options, |window, cx| {
            chrome::square_corners(window);
            chrome::power_off_on_close(window, cx);
            cx.new(|cx| Gambit::new(window, cx))
        })
        .expect("failed to open window");
        cx.activate(true);
    });
}
