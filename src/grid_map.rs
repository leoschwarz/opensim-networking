use messages::all::MapItemReply;
use types::Uuid;

pub mod region_handle {
    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct RegionHandle {
        x: u32,
        y: u32,
        handle: u64,
    }

    impl RegionHandle {
        pub fn from_handle(h: u64) -> RegionHandle {
            let x = h >> 32;
            let y = h & 0xffffffff;

            RegionHandle {
                x: x as u32,
                y: y as u32,
                handle: h,
            }
        }

        pub fn from_xy(x: u32, y: u32) -> RegionHandle {
            let x = x as u64;
            let y = y as u64;
            let x = x - (x % 256);
            let y = y - (y % 256);
            let h = x << 32 | y;

            RegionHandle {
                x: x as u32,
                y: y as u32,
                handle: h,
            }
        }

        pub fn xy(&self) -> (u32, u32) {
            (self.x, self.y)
        }

        pub fn handle(&self) -> u64 {
            self.handle
        }
    }

}

// TODO: Move this macro to a better place, and include an example
// in its accompanying documentation (can just be copied from below).
macro_rules! enum_from_u8
{
    (
        $(#[$enum_attr:meta])* pub enum $enum:ident {
            $(
                $(#[$var_attr:meta])*
                $var:ident = $num:expr
            ),+
            ,
        }
    )
        =>
    {
        $(#[$enum_attr])*
        pub enum $enum {
            $(
                $(#[$var_attr])* $var,
            )+
        }

        impl $enum {
            /// Parse a u8 value, returning None if an invalid number
            /// was provided.
            // TODO: Consider how to expose this to the world.
            pub(crate) fn from_u8(u: u8) -> Option<Self> {
                match u {
                    $(
                        $num => Some($enum::$var),
                    )+
                    _ => None,
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct GridRegion {
    /// Simulator (x,y) position on world map.
    map_pos: (i32, i32),

    /// Simulator name. (Note: Apparently in lowercase.)
    name: String,

    // TODO: (according to libOM) "Unique region identifier, combination of x,y position."
    region_handle: u64,

    /// Water height of the region.
    water_height: u8,

    /// UUID of the map image of the region.
    map_image: Uuid,
}

enum_from_u8! {
    #[derive(Clone, Debug)]
    pub enum ItemType {
        Telehub = 1,
        PgEvent = 2,
        MatureEvent = 3,
        Popular = 4,
        AgentLocations = 6,
        LandForSale = 7,
        Classified = 8,
        AdultEvent = 9,
        AdultLandForSale = 10,
    }
}

#[derive(Clone, Debug)]
pub enum MapItem {
    Telehub {
        global_pos: (u32, u32),
    },
    Event {
        global_pos: (u32, u32),
        description: String,
        rating: ContentRating,
    },
    AgentLocations {
        global_pos: (u32, u32),
        identifier: String,
        avatar_count: u32,
    },
    LandForSale {
        global_pos: (u32, u32),
        sale_id: Uuid,
        name: String,
        size: u32,
        price: u32,
        rating: ContentRating,
    },
}

fn extract_map_item_reply(msg: MapItemReply) -> Result<Vec<MapItem>, ()> {
    // TODO: Verify that this is indeed the right id.
    let _agent_id = msg.agent_data.agent_id;

    // TODO: Return error if None instead of unwrap.
    let item_type = ItemType::from_u8(msg.request_data.item_type as u8).unwrap();

    msg.data
        .into_iter()
        .map(|data| {
            match item_type {
                ItemType::Telehub => Ok(MapItem::Telehub {
                    global_pos: (data.x, data.y),
                }),
                ItemType::PgEvent | ItemType::MatureEvent | ItemType::AdultEvent => {
                    Ok(MapItem::Event {
                        global_pos: (data.x, data.y),
                        description: String::from_utf8_lossy(&data.name).into(),
                        // TODO: Actually handle errors. (unwrap None)
                        rating: ContentRating::from_u8(data.extra2 as u8).unwrap(),
                    })
                }
                ItemType::Popular | ItemType::Classified => {
                    // TODO: unimplemnted
                    Err(())
                }
                ItemType::AgentLocations => Ok(MapItem::AgentLocations {
                    global_pos: (data.x, data.y),
                    identifier: String::from_utf8_lossy(&data.name).into(),
                    avatar_count: data.extra as u32,
                }),
                ItemType::LandForSale | ItemType::AdultLandForSale => Ok(MapItem::LandForSale {
                    global_pos: (data.x, data.y),
                    sale_id: data.id,
                    name: String::from_utf8_lossy(&data.name).into(),
                    size: data.extra as u32,
                    price: data.extra2 as u32,
                    rating: match item_type {
                        ItemType::LandForSale => ContentRating::PG,
                        ItemType::AdultLandForSale => ContentRating::Adult,
                        _ => unreachable!(),
                    },
                }),
            }
        }).collect()
}

enum_from_u8! {
    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum ContentRating {
        PG = 0,
        Mature = 1,
        Adult = 2,
    }
}
