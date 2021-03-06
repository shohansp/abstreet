use crate::app::App;
use crate::game::State;
use crate::helpers::{checkbox_per_mode, cmp_duration_shorter, color_for_mode};
use crate::sandbox::dashboards::generic_trip_table::GenericTripTable;
use crate::sandbox::dashboards::table::{Col, Filter, Table};
use crate::sandbox::dashboards::DashTab;
use abstutil::prettyprint_usize;
use geom::{Duration, Time};
use sim::{TripEndpoint, TripID, TripMode};
use std::collections::{BTreeSet, HashMap};
use widgetry::{Btn, Checkbox, EventCtx, Filler, Line, Panel, Text, Widget};

pub struct FinishedTripTable;

impl FinishedTripTable {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn State> {
        GenericTripTable::new(
            ctx,
            app,
            DashTab::FinishedTripTable,
            make_table_finished_trips(app),
            make_panel_finished_trips,
        )
    }
}

pub struct CancelledTripTable;

impl CancelledTripTable {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn State> {
        GenericTripTable::new(
            ctx,
            app,
            DashTab::CancelledTripTable,
            make_table_cancelled_trips(app),
            make_panel_cancelled_trips,
        )
    }
}

pub struct UnfinishedTripTable;

impl UnfinishedTripTable {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Box<dyn State> {
        GenericTripTable::new(
            ctx,
            app,
            DashTab::UnfinishedTripTable,
            make_table_unfinished_trips(app),
            make_panel_unfinished_trips,
        )
    }
}

struct FinishedTrip {
    id: TripID,
    mode: TripMode,
    modified: bool,
    capped: bool,
    starts_off_map: bool,
    ends_off_map: bool,
    departure: Time,
    duration_after: Duration,
    duration_before: Duration,
    waiting: Duration,
    percent_waiting: usize,
}

struct CancelledTrip {
    id: TripID,
    mode: TripMode,
    departure: Time,
    starts_off_map: bool,
    ends_off_map: bool,
    duration_before: Duration,
    // TODO Reason
}

struct UnfinishedTrip {
    id: TripID,
    mode: TripMode,
    departure: Time,
    duration_before: Duration,
    // TODO Estimated wait time?
}

struct Filters {
    modes: BTreeSet<TripMode>,
    off_map_starts: bool,
    off_map_ends: bool,
    unmodified_trips: bool,
    modified_trips: bool,
    uncapped_trips: bool,
    capped_trips: bool,
}

fn produce_raw_data(app: &App) -> (Vec<FinishedTrip>, Vec<CancelledTrip>) {
    let mut finished = Vec::new();
    let mut cancelled = Vec::new();

    // Only make one pass through prebaked data
    let trip_times_before = if app.has_prebaked().is_some() {
        let mut times = HashMap::new();
        for (_, id, maybe_mode, dt) in &app.prebaked().finished_trips {
            if maybe_mode.is_some() {
                times.insert(*id, *dt);
            }
        }
        Some(times)
    } else {
        None
    };

    let sim = &app.primary.sim;
    for (_, id, maybe_mode, duration_after) in &sim.get_analytics().finished_trips {
        let trip = sim.trip_info(*id);
        let starts_off_map = match trip.start {
            TripEndpoint::Border(_, _) => true,
            _ => false,
        };
        let ends_off_map = match trip.end {
            TripEndpoint::Border(_, _) => true,
            _ => false,
        };
        let duration_before = if let Some(ref times) = trip_times_before {
            times.get(id).cloned()
        } else {
            Some(Duration::ZERO)
        };

        if maybe_mode.is_none() || duration_before.is_none() {
            cancelled.push(CancelledTrip {
                id: *id,
                mode: trip.mode,
                departure: trip.departure,
                starts_off_map,
                ends_off_map,
                duration_before: duration_before.unwrap_or(Duration::ZERO),
            });
            continue;
        };

        let (_, waiting) = sim.finished_trip_time(*id).unwrap();

        finished.push(FinishedTrip {
            id: *id,
            mode: trip.mode,
            departure: trip.departure,
            modified: trip.modified,
            capped: trip.capped,
            starts_off_map,
            ends_off_map,
            duration_after: *duration_after,
            duration_before: duration_before.unwrap(),
            waiting,
            percent_waiting: (100.0 * waiting / *duration_after) as usize,
        });
    }

    (finished, cancelled)
}

fn make_table_finished_trips(app: &App) -> Table<FinishedTrip, Filters> {
    let (finished, _) = produce_raw_data(app);
    let any_congestion_caps = app
        .primary
        .map
        .all_zones()
        .iter()
        .any(|z| z.restrictions.cap_vehicles_per_hour.is_some());
    let filter: Filter<FinishedTrip, Filters> = Filter {
        state: Filters {
            modes: TripMode::all().into_iter().collect(),
            off_map_starts: true,
            off_map_ends: true,
            unmodified_trips: true,
            modified_trips: true,
            uncapped_trips: true,
            capped_trips: true,
        },
        to_controls: Box::new(move |ctx, app, state| {
            Widget::col(vec![
                checkbox_per_mode(ctx, app, &state.modes),
                Widget::row(vec![
                    Checkbox::switch(ctx, "starting off-map", None, state.off_map_starts),
                    Checkbox::switch(ctx, "ending off-map", None, state.off_map_ends),
                    if app.primary.has_modified_trips {
                        Checkbox::switch(
                            ctx,
                            "trips unmodified by experiment",
                            None,
                            state.unmodified_trips,
                        )
                    } else {
                        Widget::nothing()
                    },
                    if app.primary.has_modified_trips {
                        Checkbox::switch(
                            ctx,
                            "trips modified by experiment",
                            None,
                            state.modified_trips,
                        )
                    } else {
                        Widget::nothing()
                    },
                    if any_congestion_caps {
                        Checkbox::switch(
                            ctx,
                            "trips not affected by congestion caps",
                            None,
                            state.uncapped_trips,
                        )
                    } else {
                        Widget::nothing()
                    },
                    if any_congestion_caps {
                        Checkbox::switch(
                            ctx,
                            "trips affected by congestion caps",
                            None,
                            state.capped_trips,
                        )
                    } else {
                        Widget::nothing()
                    },
                ]),
            ])
        }),
        from_controls: Box::new(|panel| {
            let mut modes = BTreeSet::new();
            for m in TripMode::all() {
                if panel.is_checked(m.ongoing_verb()) {
                    modes.insert(m);
                }
            }
            Filters {
                modes,
                off_map_starts: panel.is_checked("starting off-map"),
                off_map_ends: panel.is_checked("ending off-map"),
                unmodified_trips: panel
                    .maybe_is_checked("trips unmodified by experiment")
                    .unwrap_or(true),
                modified_trips: panel
                    .maybe_is_checked("trips modified by experiment")
                    .unwrap_or(true),
                uncapped_trips: panel
                    .maybe_is_checked("trips not affected by congestion caps")
                    .unwrap_or(true),
                capped_trips: panel
                    .maybe_is_checked("trips affected by congestion caps")
                    .unwrap_or(true),
            }
        }),
        apply: Box::new(|state, x| {
            if !state.modes.contains(&x.mode) {
                return false;
            }
            if !state.off_map_starts && x.starts_off_map {
                return false;
            }
            if !state.off_map_ends && x.ends_off_map {
                return false;
            }
            if !state.unmodified_trips && !x.modified {
                return false;
            }
            if !state.modified_trips && x.modified {
                return false;
            }
            if !state.uncapped_trips && !x.capped {
                return false;
            }
            if !state.capped_trips && x.capped {
                return false;
            }
            true
        }),
    };

    let mut table = Table::new(
        finished,
        Box::new(|x| x.id.0.to_string()),
        "Percent waiting",
        filter,
    );
    table.static_col("Trip ID", Box::new(|x| x.id.0.to_string()));
    if app.primary.has_modified_trips {
        table.static_col(
            "Modified",
            Box::new(|x| {
                if x.modified {
                    "Yes".to_string()
                } else {
                    "No".to_string()
                }
            }),
        );
    }
    if any_congestion_caps {
        table.static_col(
            "Capped",
            Box::new(|x| {
                if x.capped {
                    "Yes".to_string()
                } else {
                    "No".to_string()
                }
            }),
        );
    }
    table.column(
        "Type",
        Box::new(|ctx, app, x| {
            Text::from(Line(x.mode.ongoing_verb()).fg(color_for_mode(app, x.mode))).render_ctx(ctx)
        }),
        Col::Static,
    );
    table.column(
        "Departure",
        Box::new(|ctx, _, x| Text::from(Line(x.departure.ampm_tostring())).render_ctx(ctx)),
        Col::Sortable(Box::new(|rows| rows.sort_by_key(|x| x.departure))),
    );
    table.column(
        "Duration",
        Box::new(|ctx, _, x| Text::from(Line(x.duration_after.to_string())).render_ctx(ctx)),
        Col::Sortable(Box::new(|rows| rows.sort_by_key(|x| x.duration_after))),
    );

    if app.has_prebaked().is_some() {
        table.column(
            "Comparison",
            Box::new(|ctx, _, x| {
                Text::from_all(cmp_duration_shorter(x.duration_after, x.duration_before))
                    .render_ctx(ctx)
            }),
            Col::Sortable(Box::new(|rows| {
                rows.sort_by_key(|x| x.duration_after - x.duration_before)
            })),
        );
        table.column(
            "Normalized",
            Box::new(|ctx, _, x| {
                Text::from(Line(if x.duration_after == x.duration_before {
                    format!("same")
                } else if x.duration_after < x.duration_before {
                    format!(
                        "{}% faster",
                        (100.0 * (1.0 - (x.duration_after / x.duration_before))) as usize
                    )
                } else {
                    format!(
                        "{}% slower ",
                        (100.0 * ((x.duration_after / x.duration_before) - 1.0)) as usize
                    )
                }))
                .render_ctx(ctx)
            }),
            Col::Sortable(Box::new(|rows| {
                rows.sort_by_key(|x| (100.0 * (x.duration_after / x.duration_before)) as isize)
            })),
        );
    }

    table.column(
        "Time spent waiting",
        Box::new(|ctx, _, x| Text::from(Line(x.waiting.to_string())).render_ctx(ctx)),
        Col::Sortable(Box::new(|rows| rows.sort_by_key(|x| x.waiting))),
    );
    table.column(
        "Percent waiting",
        Box::new(|ctx, _, x| Text::from(Line(x.percent_waiting.to_string())).render_ctx(ctx)),
        Col::Sortable(Box::new(|rows| rows.sort_by_key(|x| x.percent_waiting))),
    );

    table
}

fn make_table_cancelled_trips(app: &App) -> Table<CancelledTrip, Filters> {
    let (_, cancelled) = produce_raw_data(app);
    // Reuse the same filters, but ignore modified and capped trips
    let filter: Filter<CancelledTrip, Filters> = Filter {
        state: Filters {
            modes: TripMode::all().into_iter().collect(),
            off_map_starts: true,
            off_map_ends: true,
            unmodified_trips: true,
            modified_trips: true,
            uncapped_trips: true,
            capped_trips: true,
        },
        to_controls: Box::new(move |ctx, app, state| {
            Widget::col(vec![
                checkbox_per_mode(ctx, app, &state.modes),
                Widget::row(vec![
                    Checkbox::switch(ctx, "starting off-map", None, state.off_map_starts),
                    Checkbox::switch(ctx, "ending off-map", None, state.off_map_ends),
                ]),
            ])
        }),
        from_controls: Box::new(|panel| {
            let mut modes = BTreeSet::new();
            for m in TripMode::all() {
                if panel.is_checked(m.ongoing_verb()) {
                    modes.insert(m);
                }
            }
            Filters {
                modes,
                off_map_starts: panel.is_checked("starting off-map"),
                off_map_ends: panel.is_checked("ending off-map"),
                unmodified_trips: true,
                modified_trips: true,
                uncapped_trips: true,
                capped_trips: true,
            }
        }),
        apply: Box::new(|state, x| {
            if !state.modes.contains(&x.mode) {
                return false;
            }
            if !state.off_map_starts && x.starts_off_map {
                return false;
            }
            if !state.off_map_ends && x.ends_off_map {
                return false;
            }
            true
        }),
    };

    let mut table = Table::new(
        cancelled,
        Box::new(|x| x.id.0.to_string()),
        "Departure",
        filter,
    );
    table.static_col("Trip ID", Box::new(|x| x.id.0.to_string()));
    table.column(
        "Type",
        Box::new(|ctx, app, x| {
            Text::from(Line(x.mode.ongoing_verb()).fg(color_for_mode(app, x.mode))).render_ctx(ctx)
        }),
        Col::Static,
    );
    table.column(
        "Departure",
        Box::new(|ctx, _, x| Text::from(Line(x.departure.ampm_tostring())).render_ctx(ctx)),
        Col::Sortable(Box::new(|rows| rows.sort_by_key(|x| x.departure))),
    );
    if app.has_prebaked().is_some() {
        table.column(
            "Estimated duration",
            Box::new(|ctx, _, x| Text::from(Line(x.duration_before.to_string())).render_ctx(ctx)),
            Col::Sortable(Box::new(|rows| rows.sort_by_key(|x| x.duration_before))),
        );
    }

    table
}

fn make_table_unfinished_trips(app: &App) -> Table<UnfinishedTrip, Filters> {
    // Only make one pass through prebaked data
    let trip_times_before = if app.has_prebaked().is_some() {
        let mut times = HashMap::new();
        for (_, id, maybe_mode, dt) in &app.prebaked().finished_trips {
            if maybe_mode.is_some() {
                times.insert(*id, *dt);
            }
        }
        Some(times)
    } else {
        None
    };
    let mut unfinished = Vec::new();
    for (id, trip) in app.primary.sim.all_trip_info() {
        if app.primary.sim.finished_trip_time(id).is_none() {
            let duration_before = trip_times_before
                .as_ref()
                .and_then(|times| times.get(&id))
                .cloned()
                .unwrap_or(Duration::ZERO);
            unfinished.push(UnfinishedTrip {
                id,
                mode: trip.mode,
                departure: trip.departure,
                duration_before,
            });
        }
    }

    // Reuse the same filters, but ignore modified and capped trips
    let filter: Filter<UnfinishedTrip, Filters> = Filter {
        state: Filters {
            modes: TripMode::all().into_iter().collect(),
            off_map_starts: true,
            off_map_ends: true,
            unmodified_trips: true,
            modified_trips: true,
            uncapped_trips: true,
            capped_trips: true,
        },
        to_controls: Box::new(move |ctx, app, state| checkbox_per_mode(ctx, app, &state.modes)),
        from_controls: Box::new(|panel| {
            let mut modes = BTreeSet::new();
            for m in TripMode::all() {
                if panel.is_checked(m.ongoing_verb()) {
                    modes.insert(m);
                }
            }
            Filters {
                modes,
                off_map_starts: true,
                off_map_ends: true,
                unmodified_trips: true,
                modified_trips: true,
                uncapped_trips: true,
                capped_trips: true,
            }
        }),
        apply: Box::new(|state, x| {
            if !state.modes.contains(&x.mode) {
                return false;
            }
            true
        }),
    };

    let mut table = Table::new(
        unfinished,
        Box::new(|x| x.id.0.to_string()),
        "Departure",
        filter,
    );
    table.static_col("Trip ID", Box::new(|x| x.id.0.to_string()));
    table.column(
        "Type",
        Box::new(|ctx, app, x| {
            Text::from(Line(x.mode.ongoing_verb()).fg(color_for_mode(app, x.mode))).render_ctx(ctx)
        }),
        Col::Static,
    );
    table.column(
        "Departure",
        Box::new(|ctx, _, x| Text::from(Line(x.departure.ampm_tostring())).render_ctx(ctx)),
        Col::Sortable(Box::new(|rows| rows.sort_by_key(|x| x.departure))),
    );
    if app.has_prebaked().is_some() {
        table.column(
            "Estimated duration",
            Box::new(|ctx, _, x| Text::from(Line(x.duration_before.to_string())).render_ctx(ctx)),
            Col::Sortable(Box::new(|rows| rows.sort_by_key(|x| x.duration_before))),
        );
    }

    table
}

fn trip_category_selector(ctx: &mut EventCtx, app: &App, tab: DashTab) -> Widget {
    let (finished, unfinished) = app.primary.sim.num_trips();
    let mut aborted = 0;
    // TODO Can we avoid iterating through this again?
    for (_, _, maybe_mode, _) in &app.primary.sim.get_analytics().finished_trips {
        if maybe_mode.is_none() {
            aborted += 1;
        }
    }

    let btn = |dash, action, label| {
        if dash == tab {
            Text::from(Line(label).underlined())
                .draw(ctx)
                .centered_vert()
        } else {
            Btn::plaintext(label).build(ctx, action, None)
        }
    };

    Widget::custom_row(vec![
        btn(
            DashTab::FinishedTripTable,
            "finished trips",
            format!(
                "{} ({:.1}%) Finished Trips",
                prettyprint_usize(finished),
                (finished as f64) / ((finished + aborted + unfinished) as f64) * 100.0
            ),
        )
        .margin_right(28),
        btn(
            DashTab::CancelledTripTable,
            "cancelled trips",
            format!("{} Cancelled Trips", prettyprint_usize(aborted)),
        )
        .margin_right(28),
        btn(
            DashTab::UnfinishedTripTable,
            "unfinished trips",
            format!(
                "{} ({:.1}%) Unfinished Trips",
                prettyprint_usize(unfinished),
                (unfinished as f64) / ((finished + aborted + unfinished) as f64) * 100.0
            ),
        ),
    ])
}

fn make_panel_finished_trips(
    ctx: &mut EventCtx,
    app: &App,
    table: &Table<FinishedTrip, Filters>,
) -> Panel {
    Panel::new(Widget::col(vec![
        DashTab::FinishedTripTable.picker(ctx, app),
        trip_category_selector(ctx, app, DashTab::FinishedTripTable),
        table.render(ctx, app),
        Filler::square_width(ctx, 0.15)
            .named("preview")
            .centered_horiz(),
    ]))
    .exact_size_percent(90, 90)
    .build(ctx)
}

fn make_panel_cancelled_trips(
    ctx: &mut EventCtx,
    app: &App,
    table: &Table<CancelledTrip, Filters>,
) -> Panel {
    Panel::new(Widget::col(vec![
        DashTab::CancelledTripTable.picker(ctx, app),
        trip_category_selector(ctx, app, DashTab::CancelledTripTable),
        table.render(ctx, app),
        Filler::square_width(ctx, 0.15)
            .named("preview")
            .centered_horiz(),
    ]))
    .exact_size_percent(90, 90)
    .build(ctx)
}

fn make_panel_unfinished_trips(
    ctx: &mut EventCtx,
    app: &App,
    table: &Table<UnfinishedTrip, Filters>,
) -> Panel {
    Panel::new(Widget::col(vec![
        DashTab::UnfinishedTripTable.picker(ctx, app),
        trip_category_selector(ctx, app, DashTab::UnfinishedTripTable),
        table.render(ctx, app),
        Filler::square_width(ctx, 0.15)
            .named("preview")
            .centered_horiz(),
    ]))
    .exact_size_percent(90, 90)
    .build(ctx)
}
