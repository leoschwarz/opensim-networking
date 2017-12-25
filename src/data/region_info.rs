use types::Uuid;
use messages::all::RegionHandshake;

#[derive(Clone, Debug)]
pub struct RegionInfo {
    // TODO: region_flags, sim_access
    /// Name of the sim.
    pub sim_name: String,

    /// ID of the sim owner.
    pub sim_owner: Uuid,

    /// Whether the client is the estate manager of this sim.
    pub is_estate_manager: bool,

    pub water_height: f32,

    // TODO ? Also do we need this be exposed?
    pub cache_id: Uuid,

    pub terrain_base: [Uuid; 4],
    pub terrain_detail: [Uuid; 4],
    /// order: 00, 01, 10, 11
    pub terrain_start_height: [f32; 4],
    /// order: 00, 01, 10, 11
    pub terrain_height_range: [f32; 4],
}

impl RegionInfo {
    pub fn extract_message(msg: RegionHandshake) -> Self {
        let info = msg.region_info;

        RegionInfo {
            sim_name: String::from_utf8_lossy(&info.sim_name[0..(info.sim_name.len() - 1)])
                .to_string(),
            sim_owner: info.sim_owner,
            is_estate_manager: info.is_estate_manager,
            water_height: info.water_height,
            cache_id: info.cache_id,
            terrain_base: [
                info.terrain_base0,
                info.terrain_base1,
                info.terrain_base2,
                info.terrain_base3,
            ],
            terrain_detail: [
                info.terrain_detail0,
                info.terrain_detail1,
                info.terrain_detail2,
                info.terrain_detail3,
            ],
            terrain_start_height: [
                info.terrain_start_height00,
                info.terrain_start_height01,
                info.terrain_start_height10,
                info.terrain_start_height11,
            ],
            terrain_height_range: [
                info.terrain_height_range00,
                info.terrain_height_range01,
                info.terrain_height_range10,
                info.terrain_height_range11,
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use packet::Packet;
    use messages::MessageInstance;
    use super::*;

    #[test]
    fn extract_message() {
        let raw = include_bytes!("tests/region_handshake.bin");
        let packet = Packet::read(raw).unwrap();
        let message = match packet.message {
            MessageInstance::RegionHandshake(msg) => msg,
            _ => panic!("can't read message"),
        };

        let info = RegionInfo::extract_message(message);
        assert_eq!(info.sim_name, "testland");
        assert_eq!(
            info.sim_owner,
            "10b2de5f-2030-4ac4-ab53-9a8f082af748".parse().unwrap()
        );
    }
}
