use dimensioned::si;
use geom::{Line, Pt2D};
use ordered_float::NotNaN;
use std::collections::{BinaryHeap, HashMap, VecDeque};
use {LaneID, LaneType, Map, Traversable, TurnID};

// TODO Make copy and return copies from all the Path queries, so we can stop dereferencing
#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum PathStep {
    // Original direction
    Lane(LaneID),
    // Sidewalks only!
    ContraflowLane(LaneID),
    Turn(TurnID),
}

// TODO All of these feel a bit hacky.
impl PathStep {
    pub fn is_contraflow(&self) -> bool {
        match self {
            PathStep::ContraflowLane(_) => true,
            _ => false,
        }
    }

    pub fn as_traversable(&self) -> Traversable {
        match self {
            PathStep::Lane(id) => Traversable::Lane(*id),
            PathStep::ContraflowLane(id) => Traversable::Lane(*id),
            PathStep::Turn(id) => Traversable::Turn(*id),
        }
    }

    pub fn as_turn(&self) -> TurnID {
        self.as_traversable().as_turn()
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct Path {
    // TODO way to encode start/end dist? I think it's needed for trace_route later...
    // actually not start dist -- that really changes all the time
    steps: VecDeque<PathStep>,
}

// TODO can have a method to verify the path is valid
impl Path {
    fn new(map: &Map, steps: Vec<PathStep>) -> Path {
        // Can disable this after trusting it.
        validate(map, &steps);
        Path {
            steps: VecDeque::from(steps),
        }
    }

    pub fn num_lanes(&self) -> usize {
        let mut count = 0;
        for s in &self.steps {
            match s {
                PathStep::Lane(_) | PathStep::ContraflowLane(_) => count += 1,
                _ => {}
            };
        }
        count
    }

    pub fn is_last_step(&self) -> bool {
        self.steps.len() == 1
    }

    pub fn isnt_last_step(&self) -> bool {
        self.steps.len() > 1
    }

    pub fn shift(&mut self) -> PathStep {
        self.steps.pop_front().unwrap()
    }

    pub fn add(&mut self, step: PathStep) {
        self.steps.push_back(step);
    }

    pub fn current_step(&self) -> &PathStep {
        &self.steps[0]
    }

    pub fn next_step(&self) -> &PathStep {
        &self.steps[1]
    }

    pub fn last_step(&self) -> &PathStep {
        &self.steps[self.steps.len() - 1]
    }
}

pub enum Pathfinder {
    ShortestDistance { goal_pt: Pt2D, is_bike: bool },
    UsingTransit,
}

impl Pathfinder {
    // Returns an inclusive path, aka, [start, ..., end]
    pub fn shortest_distance(
        map: &Map,
        start: LaneID,
        start_dist: si::Meter<f64>,
        end: LaneID,
        end_dist: si::Meter<f64>,
        is_bike: bool,
    ) -> Option<Path> {
        // TODO using first_pt here and in heuristic_dist is particularly bad for walking
        // directions
        let goal_pt = map.get_l(end).first_pt();
        Pathfinder::ShortestDistance { goal_pt, is_bike }
            .pathfind(map, start, start_dist, end, end_dist)
    }

    fn expand(&self, map: &Map, current: LaneID) -> Vec<(LaneID, NotNaN<f64>)> {
        match self {
            Pathfinder::ShortestDistance { goal_pt, is_bike } => {
                let current_length = NotNaN::new(map.get_l(current).length().value_unsafe).unwrap();
                map.get_next_turns_and_lanes(current)
                    .into_iter()
                    .filter_map(|(_, next)| {
                        if !is_bike && next.lane_type == LaneType::Biking {
                            None
                        } else {
                            // TODO cost and heuristic are wrong. need to reason about PathSteps,
                            // not LaneIDs, I think. :\
                            let heuristic_dist = NotNaN::new(
                                Line::new(next.first_pt(), *goal_pt).length().value_unsafe,
                            ).unwrap();
                            Some((next.id, current_length + heuristic_dist))
                        }
                    }).collect()
            }
            Pathfinder::UsingTransit => {
                // No heuristic, because it's hard to make admissible.
                // Cost is distance spent walking, so any jumps made using a bus are FREE. This is
                // unrealistic, but a good way to start exercising peds using transit.
                let current_lane = map.get_l(current);
                let current_length = NotNaN::new(current_lane.length().value_unsafe).unwrap();
                let mut results: Vec<(LaneID, NotNaN<f64>)> = Vec::new();
                for (_, next) in &map.get_next_turns_and_lanes(current) {
                    results.push((next.id, current_length));
                }
                // TODO Need to add a PathStep for riding a bus between two stops.
                /*
                for stop1 in &current_lane.bus_stops {
                    for stop2 in &map.get_connected_bus_stops(*stop1) {
                        results.push((stop2.sidewalk, current_length));
                    }
                }
                */
                results
            }
        }
    }

    fn pathfind(
        &self,
        map: &Map,
        start: LaneID,
        start_dist: si::Meter<f64>,
        end: LaneID,
        end_dist: si::Meter<f64>,
    ) -> Option<Path> {
        assert_eq!(map.get_l(start).lane_type, map.get_l(end).lane_type);
        if start == end {
            if start_dist > end_dist {
                assert_eq!(map.get_l(start).lane_type, LaneType::Sidewalk);
                return Some(Path::new(map, vec![PathStep::ContraflowLane(start)]));
            }
            return Some(Path::new(map, vec![PathStep::Lane(start)]));
        }

        // This should be deterministic, since cost ties would be broken by LaneID.
        let mut queue: BinaryHeap<(NotNaN<f64>, LaneID)> = BinaryHeap::new();
        queue.push((NotNaN::new(-0.0).unwrap(), start));

        let mut backrefs: HashMap<LaneID, LaneID> = HashMap::new();

        while !queue.is_empty() {
            let (cost_sofar, current) = queue.pop().unwrap();

            // Found it, now produce the path
            if current == end {
                let mut reversed_lanes: Vec<LaneID> = Vec::new();
                let mut lookup = current;
                loop {
                    reversed_lanes.push(lookup);
                    if lookup == start {
                        reversed_lanes.reverse();
                        assert_eq!(reversed_lanes[0], start);
                        assert_eq!(*reversed_lanes.last().unwrap(), end);
                        return Some(lanes_to_path(map, VecDeque::from(reversed_lanes)));
                    }
                    lookup = backrefs[&lookup];
                }
            }

            // Expand
            for (next, cost) in self.expand(map, current).into_iter() {
                if !backrefs.contains_key(&next) {
                    backrefs.insert(next, current);
                    // Negate since BinaryHeap is a max-heap.
                    queue.push((NotNaN::new(-1.0).unwrap() * (cost + cost_sofar), next));
                }
            }
        }

        // No path
        None
    }
}

fn validate(map: &Map, steps: &Vec<PathStep>) {
    for pair in steps.windows(2) {
        let from = match pair[0] {
            PathStep::Lane(id) => map.get_l(id).last_pt(),
            PathStep::ContraflowLane(id) => map.get_l(id).first_pt(),
            PathStep::Turn(id) => map.get_t(id).last_pt(),
        };
        let to = match pair[1] {
            PathStep::Lane(id) => map.get_l(id).first_pt(),
            PathStep::ContraflowLane(id) => map.get_l(id).last_pt(),
            PathStep::Turn(id) => map.get_t(id).first_pt(),
        };
        let len = Line::new(from, to).length();
        if len > 0.0 * si::M {
            panic!(
                "pathfind() returned path that warps {} from {:?} to {:?}",
                len, pair[0], pair[1]
            );
        }
    }
}

// TODO Tmp hack. Need to rewrite the A* implementation to natively understand PathSteps.
fn lanes_to_path(map: &Map, mut lanes: VecDeque<LaneID>) -> Path {
    assert!(lanes.len() > 1);
    let mut steps: Vec<PathStep> = Vec::new();

    if is_contraflow(map, lanes[0], lanes[1]) {
        steps.push(PathStep::ContraflowLane(lanes[0]));
    } else {
        steps.push(PathStep::Lane(lanes[0]));
    }
    let mut current_turn = pick_turn(lanes[0], lanes[1], map);
    steps.push(PathStep::Turn(current_turn));

    lanes.pop_front();
    lanes.pop_front();

    loop {
        if lanes.is_empty() {
            break;
        }

        assert!(lanes[0] != current_turn.dst);

        let next_turn = pick_turn(current_turn.dst, lanes[0], map);
        if current_turn.parent == next_turn.parent {
            // Don't even cross the current lane!
        } else if leads_to_end_of_lane(current_turn, map) {
            steps.push(PathStep::ContraflowLane(current_turn.dst));
        } else {
            steps.push(PathStep::Lane(current_turn.dst));
        }
        steps.push(PathStep::Turn(next_turn));

        lanes.pop_front();
        current_turn = next_turn;
    }

    if leads_to_end_of_lane(current_turn, map) {
        steps.push(PathStep::ContraflowLane(current_turn.dst));
    } else {
        steps.push(PathStep::Lane(current_turn.dst));
    }
    Path::new(map, steps)
}

fn pick_turn(from: LaneID, to: LaneID, map: &Map) -> TurnID {
    let l = map.get_l(from);
    let endpoint = if is_contraflow(map, from, to) {
        l.src_i
    } else {
        l.dst_i
    };

    for t in map.get_turns_from_lane(from) {
        if t.parent == endpoint && t.dst == to {
            return t.id;
        }
    }
    panic!("No turn from {} ({} end) to {}", from, endpoint, to);
}

fn is_contraflow(map: &Map, from: LaneID, to: LaneID) -> bool {
    map.get_l(from).dst_i != map.get_l(to).src_i
}

fn leads_to_end_of_lane(turn: TurnID, map: &Map) -> bool {
    is_contraflow(map, turn.src, turn.dst)
}
