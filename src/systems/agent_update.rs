// TODO: The idea for this module is to implement some logic which takes care of maintaining a
// local instance about agent state and having this be automatically sent to the sim at a regular
// interval.

use Vector3;

bitflags! {
    /// Agent Updates contain a set of flags which inform the sim about the current status
    /// of the agent and build the foundation for interactions like agent movement.
    pub struct ControlFlags: u32 {
        const MOVE_FWD_POS = 1 << 0;
        const MOVE_FWD_NEG = 1 << 1;
        const MOVE_LEFT_POS = 1 << 2;
        const MOVE_LEFT_NEG = 1 << 3;
        const MOVE_UP_POS = 1 << 4;
        const MOVE_UP_NEG = 1 << 5;
        const FAST_FWD = 1 << 10;
        const FAST_LEFT = 1 << 11;
        const FAST_UP = 1 << 12;
        const FLY = 1 << 13;
        const STOP = 1 << 14;
        const FINISH_ANIM = 1 << 15;
        const STAND_UP = 1 << 16;
        const SIT_ON_GROUND = 1 << 17;
        const MOUSELOOK = 1 << 18;
        const TURN_LEFT = 1 << 25;
        const TURN_RIGHT = 1 << 26;
        const AWAY = 1 << 27;
        const LBUTTON_DOWN = 1 << 28;
        const LBUTTON_UP = 1 << 29;
        const ML_LBUTTON_DOWN = 1 << 30;
        const ML_LBUTTON_UP = 1 << 31;
    }
}

// TODO: how to represent orientation? with vectors or quaternions.
pub struct AgentState {
    /// The region local coordinates.
    pub position: Vector3<f32>,

    
}
