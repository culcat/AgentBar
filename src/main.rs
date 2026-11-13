mod app;
mod icon;
mod launcher;
mod menu;
mod model;
mod providers;
mod scanner;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    app::run();
}
