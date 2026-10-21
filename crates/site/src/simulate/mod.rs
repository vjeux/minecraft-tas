//! The simulations a page needs, as pure functions of a design (what src/cache.rs stores), one
//! file per cache domain: a clock member generated and simulated (members.rs), the circuit
//! judge's run of a design (judge.rs), the 3D view's own session (view.rs: whether it runs a
//! design, a shulker machine's run, where a circuit's demo stops, a clock's chart and lamp check,
//! a thumbnail's still frame).
//!
//! Each domain's cache hashes its own file and this one (build.rs): an edit to one recomputes that
//! domain's results only (a change to the view's code keeps the 2-minute member pass). Text a
//! visitor reads stays out of what is computed here (phase labels, sentences: the pages write
//! them from these results), so wording edits elsewhere keep the cache.

mod judge;
mod members;
mod view;

pub use judge::{Details, circuit_run};
pub use members::{MemberGen, dims_of, generate_member};
pub use view::{Chart, ChartErr, ClockView, Lamp, VIEW_RUN, ViewCheck, clock_view, demo_halt, still, view_check};
