use std::sync::Arc;

use mchprs_blocks::{blocks::{Block, RedstoneWireSide}, BlockPos};
use pumpkin;
use pumpkin_data::block_properties::{self, BlockProperties, EastWireConnection, EnumVariants, HorizontalFacing, LeverLikeProperties, NorthWireConnection, OakDoorLikeProperties, OakTrapdoorLikeProperties, RepeaterLikeProperties, SouthWireConnection, StonePressurePlateLikeProperties, WestWireConnection};
use pumpkin_world::world::BlockFlags;

use crate::{RTorchProps, RWallTorchProps, RedstoneLampProperties, RedstoneWireProperties};

pub struct PumpkinWorld {
    pub base: BlockPos,
    pub set_events: Vec<(BlockPos, u32)>
}


impl mchprs_world::World for PumpkinWorld {
    fn get_block_raw(&self, pos: BlockPos) -> u32 {
        todo!()
    }

    fn set_block_raw(&mut self, pos: BlockPos, block: u32) -> bool {
        self.set_events.push((pos, block));

        return true;
    }

    fn delete_block_entity(&mut self, pos: BlockPos) {
        todo!()
    }

    fn get_block_entity(&self, pos: BlockPos) -> Option<&mchprs_blocks::block_entities::BlockEntity> {
        todo!()
    }

    fn set_block_entity(&mut self, pos: BlockPos, block_entity: mchprs_blocks::block_entities::BlockEntity) {
        todo!()
    }

    fn get_chunk(&self, x: i32, z: i32) -> Option<&mchprs_world::storage::Chunk> {
        todo!()
    }

    fn get_chunk_mut(&mut self, x: i32, z: i32) -> Option<&mut mchprs_world::storage::Chunk> {
        todo!()
    }

    fn schedule_tick(&mut self, pos: BlockPos, delay: u32, priority: mchprs_world::TickPriority) {
        todo!()
    }

    fn pending_tick_at(&mut self, pos: BlockPos) -> bool {
        todo!()
    }
}


impl PumpkinWorld {
    pub fn new(base: BlockPos) -> Self {
        PumpkinWorld {
            base,
            set_events: Vec::new(),
        }
    }

    pub async fn apply(&mut self, world: Arc<pumpkin::world::World>) {
        for (pos, block) in self.set_events.drain(..) {
            let pumpkin_pos = pumpkin_util::math::position::BlockPos::new(
                self.base.x + pos.x,
                self.base.y + pos.y,
                self.base.z + pos.z,
            );

            let block = Block::from_id(block);

            let pumpkin_block = world.get_block(&pumpkin_pos).await;

            let state = match block {
                Block::RedstoneWire { wire } => RedstoneWireProperties {
                    north: match wire.north {
                            RedstoneWireSide::Up   => NorthWireConnection::Up  ,
                            RedstoneWireSide::Side => NorthWireConnection::Side,
                            RedstoneWireSide::None => NorthWireConnection::None,
                    },
                    south: match wire.south {
                        RedstoneWireSide::Up   => SouthWireConnection::Up  ,
                        RedstoneWireSide::Side => SouthWireConnection::Side,
                        RedstoneWireSide::None => SouthWireConnection::None,
                    },
                    east: match wire.east {
                        RedstoneWireSide::Up   => EastWireConnection::Up  ,
                        RedstoneWireSide::Side => EastWireConnection::Side,
                        RedstoneWireSide::None => EastWireConnection::None,
                    },
                    west: match wire.west {
                        RedstoneWireSide::Up   => WestWireConnection::Up  ,
                        RedstoneWireSide::Side => WestWireConnection::Side,
                        RedstoneWireSide::None => WestWireConnection::None,
                    },
                    power: pumpkin_data::block_properties::Integer0To15::from_index(wire.power as u16)
                }.to_state_id(&pumpkin_block),
                Block::Lever { lever } => LeverLikeProperties {
                    face: match lever.face {
                        mchprs_blocks::blocks::LeverFace::Floor => {
                            block_properties::BlockFace::Floor
                        }
                        mchprs_blocks::blocks::LeverFace::Wall => {
                            block_properties::BlockFace::Wall
                        }
                        mchprs_blocks::blocks::LeverFace::Ceiling => {
                            block_properties::BlockFace::Ceiling
                        }
                    },
                    facing: direction_to_pumpkin(lever.facing),
                    powered: lever.powered
                }.to_state_id(pumpkin_block),
                Block::StoneButton { button } => LeverLikeProperties {
                    face: match button.face {
                        mchprs_blocks::blocks::ButtonFace::Floor => {
                            block_properties::BlockFace::Floor
                        }
                        mchprs_blocks::blocks::ButtonFace::Wall => {
                            block_properties::BlockFace::Wall
                        }
                        mchprs_blocks::blocks::ButtonFace::Ceiling => {
                            block_properties::BlockFace::Ceiling
                        }
                    },
                    facing: direction_to_pumpkin(button.facing),
                    powered: button.powered
                }.to_state_id(pumpkin_block),
                Block::RedstoneTorch { lit } => RTorchProps {
                    lit
                }.to_state_id(pumpkin_block),
                Block::RedstoneWallTorch { lit, facing } => RWallTorchProps {
                    facing: direction_to_pumpkin(facing),
                    lit,
                }.to_state_id(pumpkin_block),
                Block::RedstoneRepeater { repeater } => RepeaterLikeProperties {
                    delay: block_properties::Integer1To4::from_index(repeater.delay as u16 - 1),
                    facing: direction_to_pumpkin(repeater.facing),
                    locked: repeater.locked,
                    powered: repeater.powered,
                }.to_state_id(pumpkin_block),
                Block::RedstoneLamp { lit } => RedstoneLampProperties {
                    lit,
                }.to_state_id(pumpkin_block),
                Block::IronTrapdoor { facing, half, powered } => OakTrapdoorLikeProperties {
                    facing: direction_to_pumpkin(facing),
                    half: match half {
                        mchprs_blocks::blocks::TrapdoorHalf::Top => {
                            block_properties::BlockHalf::Top
                        }
                        mchprs_blocks::blocks::TrapdoorHalf::Bottom => {
                            block_properties::BlockHalf::Bottom
                        }
                    },
                    open: powered,
                    powered,
                    waterlogged: false,
                }.to_state_id(pumpkin_block),
                Block::NoteBlock { instrument, note, powered } => todo!(),
                Block::StonePressurePlate { powered } => StonePressurePlateLikeProperties {
                    powered
                }.to_state_id(pumpkin_block),
                Block::Observer { facing } => todo!(),
                Block::RedstoneComparator { comparator } => todo!(),
                _ => continue,
            };

            println!("updating block at {:?} to {:?}", pumpkin_pos, block);


            world.set_block_state(&pumpkin_pos, state, BlockFlags::empty()).await;

        }
    }
}


fn facing_to_pumpkin(face: mchprs_blocks::BlockFacing) -> block_properties::Facing {
    match face {
        mchprs_blocks::BlockFacing::North => block_properties::Facing::North,
        mchprs_blocks::BlockFacing::East => block_properties::Facing::East,
        mchprs_blocks::BlockFacing::South => block_properties::Facing::South,
        mchprs_blocks::BlockFacing::West => block_properties::Facing::West,
        mchprs_blocks::BlockFacing::Up => block_properties::Facing::Up,
        mchprs_blocks::BlockFacing::Down => block_properties::Facing::Down,
    }
}

fn direction_to_pumpkin(face: mchprs_blocks::BlockDirection) -> HorizontalFacing {
    match face {
        mchprs_blocks::BlockDirection::North => HorizontalFacing::North,
        mchprs_blocks::BlockDirection::East => HorizontalFacing::East,
        mchprs_blocks::BlockDirection::South => HorizontalFacing::South,
        mchprs_blocks::BlockDirection::West => HorizontalFacing::West,
    }
}