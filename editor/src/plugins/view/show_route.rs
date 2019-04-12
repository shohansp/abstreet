use crate::objects::{DrawCtx, ID};
use crate::plugins::{AmbientPlugin, PluginCtx};
use ezgui::{Color, GfxCtx, Key};
use geom::{Duration, PolyLine};
use map_model::LANE_THICKNESS;
use sim::{AgentID, TripID};

pub enum ShowRouteState {
    Inactive,
    Hovering(Duration, AgentID, PolyLine),
    Active(Duration, TripID, Option<PolyLine>),
    DebugAllRoutes(Duration, Vec<PolyLine>),
}

impl ShowRouteState {
    pub fn new() -> ShowRouteState {
        ShowRouteState::Inactive
    }
}

impl AmbientPlugin for ShowRouteState {
    fn ambient_event(&mut self, ctx: &mut PluginCtx) {
        match self {
            ShowRouteState::Inactive => {
                if let Some(agent) = ctx.primary.current_selection.and_then(|id| id.agent_id()) {
                    if let Some(trace) = ctx.primary.sim.trace_route(agent, &ctx.primary.map, None)
                    {
                        *self = ShowRouteState::Hovering(ctx.primary.sim.time(), agent, trace);
                    }
                };
            }
            ShowRouteState::Hovering(time, agent, _) => {
                // Argh, borrow checker.
                let agent = *agent;

                if *time != ctx.primary.sim.time()
                    || ctx.primary.current_selection != Some(ID::from_agent(agent))
                {
                    *self = ShowRouteState::Inactive;
                    if let Some(new_agent) =
                        ctx.primary.current_selection.and_then(|id| id.agent_id())
                    {
                        if let Some(trace) =
                            ctx.primary
                                .sim
                                .trace_route(new_agent, &ctx.primary.map, None)
                        {
                            *self =
                                ShowRouteState::Hovering(ctx.primary.sim.time(), new_agent, trace);
                        }
                    }
                }

                // If there's a current route, then there must be a trip.
                let trip = ctx.primary.sim.agent_to_trip(agent).unwrap();
                // TODO agent might be stale here! Really need a second match after this or
                // something. Expressing a state machine like this isn't really great.
                if ctx
                    .input
                    .contextual_action(Key::R, &format!("show {}'s route", agent))
                {
                    *self = show_route(trip, ctx);
                }
            }
            ShowRouteState::Active(time, trip, _) => {
                ctx.input.set_mode_with_prompt(
                    "Agent Route Debugger",
                    format!("Agent Route Debugger for {}", trip),
                    &ctx.canvas,
                );
                if ctx.input.modal_action("quit") {
                    *self = ShowRouteState::Inactive;
                } else if ctx.input.modal_action("show route for all agents") {
                    *self = debug_all_routes(ctx);
                } else if *time != ctx.primary.sim.time() {
                    *self = show_route(*trip, ctx);
                }
            }
            ShowRouteState::DebugAllRoutes(time, _) => {
                ctx.input.set_mode_with_prompt(
                    "Agent Route Debugger",
                    "Agent Route Debugger for all routes".to_string(),
                    &ctx.canvas,
                );
                if ctx.input.modal_action("quit") {
                    *self = ShowRouteState::Inactive;
                } else if *time != ctx.primary.sim.time() {
                    *self = debug_all_routes(ctx);
                }
            }
        };
    }

    fn draw(&self, g: &mut GfxCtx, ctx: &DrawCtx) {
        match self {
            ShowRouteState::Hovering(_, _, ref trace) => {
                g.draw_polygon(
                    ctx.cs.get("route").alpha(0.5),
                    &trace.make_polygons(LANE_THICKNESS),
                );
            }
            ShowRouteState::Active(_, _, Some(ref trace)) => {
                g.draw_polygon(
                    ctx.cs.get_def("route", Color::RED.alpha(0.8)),
                    &trace.make_polygons(LANE_THICKNESS),
                );
            }
            ShowRouteState::DebugAllRoutes(_, ref traces) => {
                for t in traces {
                    g.draw_polygon(ctx.cs.get("route"), &t.make_polygons(LANE_THICKNESS));
                }
            }
            _ => {}
        }
    }
}

fn show_route(trip: TripID, ctx: &mut PluginCtx) -> ShowRouteState {
    let time = ctx.primary.sim.time();
    if let Some(agent) = ctx.primary.sim.trip_to_agent(trip) {
        // Trace along the entire route by passing in max distance
        if let Some(trace) = ctx.primary.sim.trace_route(agent, &ctx.primary.map, None) {
            ShowRouteState::Active(time, trip, Some(trace))
        } else {
            println!("{} has no trace right now", agent);
            ShowRouteState::Active(time, trip, None)
        }
    } else {
        println!(
            "{} has no agent associated right now; is the trip done?",
            trip
        );
        ShowRouteState::Active(time, trip, None)
    }
}

fn debug_all_routes(ctx: &mut PluginCtx) -> ShowRouteState {
    let mut traces: Vec<PolyLine> = Vec::new();
    let trips: Vec<TripID> = ctx
        .primary
        .sim
        .get_stats(&ctx.primary.map)
        .canonical_pt_per_trip
        .keys()
        .cloned()
        .collect();
    for trip in trips {
        if let Some(agent) = ctx.primary.sim.trip_to_agent(trip) {
            if let Some(trace) = ctx.primary.sim.trace_route(agent, &ctx.primary.map, None) {
                traces.push(trace);
            }
        }
    }
    ShowRouteState::DebugAllRoutes(ctx.primary.sim.time(), traces)
}
