mod app;
mod render;
mod ui;

use macroquad::prelude::*;

fn conf() -> Conf {
    Conf {
        window_title: "Vector Field".to_string(),
        window_width: 1280,
        window_height: 800,
        high_dpi: true,
        ..Default::default()
    }
}

#[macroquad::main(conf)]
async fn main() {
    let mut app = app::App::new();
    loop {
        app.frame();
        next_frame().await;
    }
}
