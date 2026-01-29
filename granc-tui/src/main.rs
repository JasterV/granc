mod config;
mod effects;
mod globals;
mod model;
mod msg;
mod update;
mod view;

use model::Model;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    // Initialize global Tokio handle for effects thread
    globals::init_handle();

    let initial_model = Model::default();

    teatui::start(
        initial_model,
        update::update,
        view::view,
        effects::handle_effect,
    )?;

    Ok(())
}
