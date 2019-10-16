// These are common OSM keys. Keys used in just one or two places don't really need to be defined
// here.

// These're normal OSM keys.
pub const NAME: &str = "name";
pub const HIGHWAY: &str = "highway";
pub const MAXSPEED: &str = "maxspeed";
pub const PARKING_RIGHT: &str = "parking:lane:right";
pub const PARKING_LEFT: &str = "parking:lane:left";
pub const PARKING_BOTH: &str = "parking:lane:both";

// The rest of these are all inserted by A/B Street to plumb data between different stages of map
// construction. They could be plumbed another way, but this is the most convenient.

// TODO Comparing to Some(&"true".to_string()) is annoying

// Just a copy of OSM IDs, so that things displaying/searching tags will also pick these up.
pub const OSM_WAY_ID: &str = "abst:osm_way_id";
pub const OSM_REL_ID: &str = "abst:osm_rel_id";
// OSM ways are split into multiple roads. The first and last road are marked, which is important
// for interpreting turn restrictions.
pub const ENDPT_FWD: &str = "abst:endpt_fwd";
pub const ENDPT_BACK: &str = "abst:endpt_back";

// Synthetic roads have (some of) these.
pub const SYNTHETIC: &str = "abst:synthetic";
pub const SYNTHETIC_LANES: &str = "abst:synthetic_lanes";
pub const FWD_LABEL: &str = "abst:fwd_label";
pub const BACK_LABEL: &str = "abst:back_label";

// Synthetic buildings may have these.
pub const LABEL: &str = "abst:label";

// Any roads might have these.
pub const INFERRED_PARKING: &str = "abst:parking_inferred";