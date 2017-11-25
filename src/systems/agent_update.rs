// TODO: The idea for this module is to implement some logic which takes care of maintaining a
// local instance about agent state and having this be automatically sent to the sim at a regular
// interval.

use {Quaternion, Uuid, Vector3};
use messages::{AgentUpdate, AgentUpdate_AgentData};

bitflags! {
    /// Agent Updates contain a set of flags which inform the sim about the current status
    /// of the agent and build the foundation for interactions like agent movement.
    pub struct ControlFlags: u32 {
        const EMPTY = 0;
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

pub enum MoveDirection {
    Forward,
    Backward,
    Left,
    Right,
    Up,
    Down,
}

impl MoveDirection {
    pub fn inverse(&self) -> MoveDirection {
        match *self {
            MoveDirection::Forward => MoveDirection::Backward,
            MoveDirection::Backward => MoveDirection::Forward,
            MoveDirection::Left => MoveDirection::Right,
            MoveDirection::Right => MoveDirection::Left,
            MoveDirection::Up => MoveDirection::Down,
            MoveDirection::Down => MoveDirection::Up,
        }
    }
}

pub enum Modality {
    /// The default.
    Walking,

    /// Fast version of walking.
    Running,

    /// Flying.
    Flying,

    /// Sitting on the ground.
    Sitting,
}

// TODO: how to represent orientation? with vectors or quaternions.
pub struct AgentState {
    /// The region local coordinates.
    pub position: Vector3<f32>,

    /// The direction in which to move.
    pub move_direction: Option<MoveDirection>,

    /// The current modality of the movement.
    pub modality: Modality,

    pub body_rotation: Quaternion<f32>,
    pub head_rotation: Quaternion<f32>,
}

impl AgentState {
    pub fn to_control_flags(&self) -> ControlFlags {
        let mut flags = match self.move_direction {
            None => ControlFlags::EMPTY,
            Some(MoveDirection::Forward) => ControlFlags::MOVE_FWD_POS,
            Some(MoveDirection::Backward) => ControlFlags::MOVE_FWD_NEG,
            Some(MoveDirection::Left) => ControlFlags::MOVE_LEFT_POS,
            Some(MoveDirection::Right) => ControlFlags::MOVE_LEFT_NEG,
            Some(MoveDirection::Up) => ControlFlags::MOVE_UP_POS,
            Some(MoveDirection::Down) => ControlFlags::MOVE_UP_NEG,
        };

        // TODO: fast movement / running

        match self.modality {
            Modality::Walking => {}
            Modality::Running => panic!("running is not yet implemented"), // TODO
            Modality::Flying => flags.insert(ControlFlags::FLY),
            Modality::Sitting => {}
        }

        flags
    }

    pub fn to_update_message(&self, agent_id: Uuid, session_id: Uuid) -> AgentUpdate {
        AgentUpdate {
            agent_data: AgentUpdate_AgentData {
                agent_id: agent_id,
                session_id: session_id,
                body_rotation: self.body_rotation.clone(),
                head_rotation: self.head_rotation.clone(),
                state: 0,
                camera_center: self.position,
                camera_at_axis: self.position + Vector3::new(1., 0., 0.),
                camera_left_axis: self.position + Vector3::new(0., 1., 0.),
                camera_up_axis: self.position + Vector3::new(0., 0., 1.),
                far: 0.,
                control_flags: self.to_control_flags().bits(),
                flags: 0,
            },
        }
    }
}
