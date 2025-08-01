mod fixed_world;

use std::sync::Arc;

use async_trait::async_trait;
use mchprs_blocks::blocks::{Lever, RedstoneWire, RedstoneWireSide};
use mchprs_redpiler::{BackendVariant, Compiler, CompilerOptions};
use pumpkin_api_macros::{plugin_impl, plugin_method};
use pumpkin_data::{block_properties::{BlockProperties, EastWireConnection, EnumVariants, LeverLikeProperties, NorthWireConnection, RedstoneOreLikeProperties, RedstoneWireLikeProperties, SouthWireConnection, WestWireConnection}, Block};
use mchprs_world::World;

type RedstoneWireProperties = RedstoneWireLikeProperties;
type RWallTorchProps = pumpkin_data::block_properties::FurnaceLikeProperties;
type RTorchProps = pumpkin_data::block_properties::RedstoneOreLikeProperties;
type RedstoneLampProperties = RedstoneOreLikeProperties;

#[plugin_method]
async fn on_load(&mut self, server: Arc<Context>) -> Result<(), String> {
    on_load_internal(self, server).await
}

#[inline(always)]
async fn on_load_internal(plugin: &mut MyPlugin, server: Arc<Context>) -> Result<(), String> {
    pumpkin::init_log!(); 

    log::info!("Hello, Pumpkin!");

    let permission_node = "redpiler:compile";
    let permission = Permission::new(permission_node, "<DESCRIPTION>", PermissionDefault::Allow);

    let manager = server.permission_manager.write().await;
    let mut registry = manager.registry.write().await;
    registry.register_permission(permission)?;

    let command = CommandTree::new(["redpiler", "rp"], "Compile redstone in selected area for faster execution") 
        .then(literal("compile").execute(Exe {cmd: Command::Compile, data: plugin.data.clone()}))
        .then(literal("pos1").execute(Exe {cmd: Command::Pos1, data: plugin.data.clone()}))
        .then(literal("pos2").execute(Exe {cmd: Command::Pos2, data: plugin.data.clone()}))
        .then(literal("deselect").execute(Exe {cmd: Command::Deselect, data: plugin.data.clone()}))
    ;
    
    server.register_command(command, permission_node).await;
    log::info!("registered redpiler commands");

    Ok(())
}

#[plugin_impl]
pub struct MyPlugin {
    data: Arc<RwLock<PluginData>>
}

impl MyPlugin {
    pub fn new() -> Self {
        println!("hello from redpiler plugin");

        MyPlugin {
            data: Arc::new(RwLock::new(PluginData::default()))
        }
    }
}

impl Default for MyPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Default)]
struct PluginData {
    pos1: Option<BlockPos>,
    pos2: Option<BlockPos>,
}

use pumpkin::{
    block::blocks::redstone::redstone_wire::RedstoneWireBlock, command::{ 
        args::ConsumedArgs, dispatcher::CommandError, tree::{builder::literal, CommandTree}, CommandExecutor, CommandSender,
    }, data, plugin::{player::player_join::PlayerJoinEvent, Context, EventHandler, EventPriority}, server::Server
};
use pumpkin_util::{math::position::BlockPos, permission::{Permission, PermissionDefault}, text::TextComponent, PermissionLvl};
use tokio::sync::RwLock;

#[derive(Debug, Clone, Copy)]
enum Command {
    Compile,
    Pos1, Pos2,
    Deselect
}

struct Exe {
    data: Arc<RwLock<PluginData>>,
    cmd: Command,
}

const REDSTONE_WIRE: u16 = Block::REDSTONE_WIRE.id;
const REDSTONE_TORCH: u16 = Block::REDSTONE_TORCH.id;
const REDSTONE_WALL_TORCH: u16 = Block::REDSTONE_WALL_TORCH.id;
const LEVER: u16 = Block::LEVER.id;
const REDSTONE_LAMP: u16 = Block::REDSTONE_LAMP.id;

#[async_trait] 
impl CommandExecutor for Exe {
    async fn execute<'a>(
        &self,
        sender: &mut CommandSender,
        server: &Server,
        args: &ConsumedArgs<'a>,
    ) -> Result<(), CommandError> {
        let world = sender.world().await;
        log::info!("hello execute {:?}", self.cmd);
        log::info!("has world {:?}", world.is_some());
        log::info!("player {:?}", sender.as_player().map(|p| p.position().to_block_pos()));

        let (Some(world), Some(player)) = (world, sender.as_player()) else {
            log::error!("Redpiler commands must be run by a player");
            return Err(CommandError::PermissionDenied);
        };

        let mut data = self.data.write().await;

        
        match self.cmd {
            Command::Compile => {
                let (Some(p1), Some(p2)) = (data.pos1, data.pos2) else {
                    return Err(CommandError::PermissionDenied);
                };
                
                let x1 = p1.0.x.min(p2.0.x);
                let x2 = p1.0.x.max(p2.0.x);
                let y1 = p1.0.y.min(p2.0.y);
                let y2 = p1.0.y.max(p2.0.y);
                let z1 = p1.0.z.min(p2.0.z);
                let z2 = p1.0.z.max(p2.0.z);
                
                let mut plot = fixed_world::TestWorld::new(x2-x1, y2-y1, z2-z1);
                
                let id_air = 0;

                for z in z1..=z2 {
                    for y in y1..=y2 {
                        for x in x1..=x2 {
                            let pos = BlockPos::new(x, y, z);
                            let mchprs_pos = mchprs_blocks::BlockPos::new(x - x1, y - y1, z - z1);
                            
                            let (b, s) = world.get_block_and_state(&pos).await;

                            if b.id == id_air {
                                continue;
                            }
                            
                            let mchprs_block = match b.id {
                                REDSTONE_WIRE => {
                                    let props = RedstoneWireProperties::from_state_id(s.id, b);
                                    sender.send_message(TextComponent::text(format!("{:?} {:?}", pos, props))).await;   
                                
                                    mchprs_blocks::blocks::Block::RedstoneWire { wire: RedstoneWire::new(
                                        match props.north {
                                            NorthWireConnection::Up => RedstoneWireSide::Up,
                                            NorthWireConnection::Side => RedstoneWireSide::Side,
                                            NorthWireConnection::None => RedstoneWireSide::None,
                                        },
                                        match props.south {
                                            SouthWireConnection::Up => RedstoneWireSide::Up,
                                            SouthWireConnection::Side => RedstoneWireSide::Side,
                                            SouthWireConnection::None => RedstoneWireSide::None,
                                        },
                                        match props.east {
                                            EastWireConnection::Up => RedstoneWireSide::Up,
                                            EastWireConnection::Side => RedstoneWireSide::Side,
                                            EastWireConnection::None => RedstoneWireSide::None,
                                        },
                                        match props.west {
                                            WestWireConnection::Up => RedstoneWireSide::Up,
                                            WestWireConnection::Side => RedstoneWireSide::Side,
                                            WestWireConnection::None => RedstoneWireSide::None,
                                        },
                                        props.power.to_index() as u8
                                    )}
                                }
                                LEVER => {
                                    let props = LeverLikeProperties::from_state_id(s.id, b);
                                    sender.send_message(TextComponent::text(format!("{:?} {:?}", pos, props))).await;

                                    mchprs_blocks::blocks::Block::Lever { lever: Lever::new(
                                        match props.face {
                                            pumpkin_data::block_properties::BlockFace::Floor => mchprs_blocks::blocks::LeverFace::Floor,
                                            pumpkin_data::block_properties::BlockFace::Wall => mchprs_blocks::blocks::LeverFace::Wall,
                                            pumpkin_data::block_properties::BlockFace::Ceiling => mchprs_blocks::blocks::LeverFace::Ceiling,
                                        },
                                        direction_to_mchprs(props.facing),
                                        props.powered
                                    )}
                                }
                                REDSTONE_LAMP => {
                                    let props = RedstoneLampProperties::from_state_id(s.id, b);
                                    sender.send_message(TextComponent::text(format!("{:?} {:?}", pos, props))).await;

                                    mchprs_blocks::blocks::Block::RedstoneLamp { lit: props.lit }
                                }
                                REDSTONE_TORCH => {
                                    let props = RTorchProps::from_state_id(s.id, b);
                                    sender.send_message(TextComponent::text(format!("{:?} {:?}", pos, props))).await;

                                    continue;
                                }
                                REDSTONE_WALL_TORCH => {
                                    let props = RWallTorchProps::from_state_id(s.id, b);
                                    sender.send_message(TextComponent::text(format!("{:?} {:?}", pos, props))).await;

                                    continue;
                                }
                                _ => {
                                    sender.send_message(TextComponent::text(format!("{:?} {:?} skipped", pos, b.name))).await;
                                    continue;
                                },
                            };

                            plot.set_block(mchprs_pos, mchprs_block);
                        }
                    }

                    let min_pos = mchprs_blocks::BlockPos::new(0, 0, 0);
                    let max_pos =  mchprs_blocks::BlockPos::new(plot.size_x-1, plot.size_y-1, plot.size_z-1);

                    let mut compiler = Compiler::default();
                    let bounds = (min_pos, max_pos);
                    let options = CompilerOptions {
                        optimize: true,
                        io_only: true,
                        wire_dot_out: true,
                        backend_variant: BackendVariant::Direct,
                        export_dot_graph: true,
                        ..Default::default()
                    };
                    let ticks = plot.to_be_ticked.drain(..).collect();
                    let monitor = Default::default();
                    compiler.compile(&mut plot, bounds, options, ticks, monitor);
                }

            },
            Command::Pos1 => {
                data.pos1 = Some(player.position().sub_raw(0.5, 0.5, 0.5).to_block_pos());
            },
            Command::Pos2 => {
                data.pos2 = Some(player.position().sub_raw(0.5, 0.5, 0.5).to_block_pos());
            },
            Command::Deselect => {
                data.pos1 = None;
                data.pos2 = None;
            },
        }

        Ok(())
    }
}

fn facing_to_mchprs(face: pumpkin_data::block_properties::Facing) -> mchprs_blocks::BlockFacing {
    match face {
        pumpkin_data::block_properties::Facing::North => mchprs_blocks::BlockFacing::North,
        pumpkin_data::block_properties::Facing::East  => mchprs_blocks::BlockFacing::East ,
        pumpkin_data::block_properties::Facing::South => mchprs_blocks::BlockFacing::South,
        pumpkin_data::block_properties::Facing::West  => mchprs_blocks::BlockFacing::West ,
        pumpkin_data::block_properties::Facing::Up    => mchprs_blocks::BlockFacing::Up   ,
        pumpkin_data::block_properties::Facing::Down  => mchprs_blocks::BlockFacing::Down ,
    }
}


fn facing_to_pumpkin(face: mchprs_blocks::BlockFacing) -> pumpkin_data::block_properties::Facing {
    match face {
        mchprs_blocks::BlockFacing::North => pumpkin_data::block_properties::Facing::North,
        mchprs_blocks::BlockFacing::East  => pumpkin_data::block_properties::Facing::East ,
        mchprs_blocks::BlockFacing::South => pumpkin_data::block_properties::Facing::South,
        mchprs_blocks::BlockFacing::West  => pumpkin_data::block_properties::Facing::West ,
        mchprs_blocks::BlockFacing::Up    => pumpkin_data::block_properties::Facing::Up   ,
        mchprs_blocks::BlockFacing::Down  => pumpkin_data::block_properties::Facing::Down ,
    }
}

fn direction_to_mchprs(face: pumpkin_data::block_properties::HorizontalFacing) -> mchprs_blocks::BlockDirection {
    match face {
        pumpkin_data::block_properties::HorizontalFacing::North => mchprs_blocks::BlockDirection::North,
        pumpkin_data::block_properties::HorizontalFacing::East  => mchprs_blocks::BlockDirection::East ,
        pumpkin_data::block_properties::HorizontalFacing::South => mchprs_blocks::BlockDirection::South,
        pumpkin_data::block_properties::HorizontalFacing::West  => mchprs_blocks::BlockDirection::West ,
    }
}


fn direction_to_pumpkin(face: mchprs_blocks::BlockDirection) -> pumpkin_data::block_properties::HorizontalFacing {
    match face {
        mchprs_blocks::BlockDirection::North => pumpkin_data::block_properties::HorizontalFacing::North,
        mchprs_blocks::BlockDirection::East  => pumpkin_data::block_properties::HorizontalFacing::East ,
        mchprs_blocks::BlockDirection::South => pumpkin_data::block_properties::HorizontalFacing::South,
        mchprs_blocks::BlockDirection::West  => pumpkin_data::block_properties::HorizontalFacing::West ,
    }
}