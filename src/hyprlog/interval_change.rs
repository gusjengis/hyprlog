use crate::{interval::Interval, log_reader::LogReader, model_building::build_model, tui::App};

pub fn handle_interval_change(app: &mut App) {
    app.model.reset_mappings();
    // add any new logs to the model
    let mut log_reader = LogReader::new(&app.settings);
    build_model(&mut app.model, &mut log_reader, &app.settings).unwrap();
    if let Some(overlap) = app
        .settings
        .loaded_interval
        .overlap(&app.settings.focused_interval)
    {
        app.model.map_overlap(&overlap);
    }
    app.model.sort();
    // update the interval
    app.settings
        .loaded_interval
        .expand_to_include(&app.settings.focused_interval);
}
