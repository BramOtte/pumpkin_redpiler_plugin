// TODO: Cleanup

mod fixed_world;
mod pumpkin_plot;

use std::{sync::{atomic::Ordering, Arc}, time::Duration};

use async_trait::async_trait;
use mchprs_blocks::{blocks::{
    Lever, RedstoneComparator, RedstoneRepeater, RedstoneWire, RedstoneWireSide,
}};
use mchprs_redpiler::{BackendVariant, Compiler, CompilerOptions};
use mchprs_world::World;
use pumpkin_api_macros::{plugin_impl, plugin_method, with_runtime};
use pumpkin_data::{
    Block,
    block_properties::{
        self, BarrelLikeProperties, BlockProperties, ComparatorLikeProperties, EastWireConnection,
        EnumVariants, HorizontalFacing, LeverLikeProperties, NorthWireConnection,
        OakTrapdoorLikeProperties, RedstoneOreLikeProperties, RedstoneWireLikeProperties,
        RepeaterLikeProperties, SouthWireConnection, StonePressurePlateLikeProperties,
        WestWireConnection,
    },
};

use pumpkin::{
    command::{
        args::{Arg, ConsumedArgs}, dispatcher::CommandError, tree::{builder::literal, CommandTree}, CommandExecutor, CommandSender
    },
    plugin::{block::{block_break::BlockBreakEvent, block_place::BlockPlaceEvent}, player::{player_interact_event::{InteractAction, PlayerInteractEvent}, player_join::PlayerJoinEvent}, Context, Event, EventHandler, EventPriority},
    server::Server,
};
use pumpkin_util::{
    math::position::BlockPos,
    permission::{Permission, PermissionDefault},
    text::TextComponent,
};
use tokio::sync::RwLock;

use crate::{fixed_world::TestWorld, pumpkin_plot::PumpkinWorld};

pub type RedstoneWireProperties = RedstoneWireLikeProperties;
pub type RWallTorchProps = block_properties::FurnaceLikeProperties;
pub type RTorchProps = block_properties::RedstoneOreLikeProperties;
pub type RedstoneLampProperties = RedstoneOreLikeProperties;

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

    let command = CommandTree::new(
        ["redpiler", "rp"],
        "Compile redstone in selected area for faster execution",
    )
    .then(literal("compile").execute(Exe {
        cmd: Command::Compile,
        data: plugin.data.clone(),
    }))
    .then(literal("pos1").execute(Exe {
        cmd: Command::Pos1,
        data: plugin.data.clone(),
    }))
    .then(literal("pos2").execute(Exe {
        cmd: Command::Pos2,
        data: plugin.data.clone(),
    }))
    .then(literal("deselect").execute(Exe {
        cmd: Command::Deselect,
        data: plugin.data.clone(),
    }));

    server.register_command(command, permission_node).await;
    log::info!("registered redpiler commands");
    
    server.register_event(Arc::new(InputHandler{data: plugin.data.clone()}), EventPriority::Lowest, true).await;
    server.register_event(Arc::new(BreakHandler{data: plugin.data.clone()}), EventPriority::Lowest, true).await;
    server.register_event(Arc::new(PlaceHandler{data: plugin.data.clone()}), EventPriority::Lowest, true).await;

    log::info!("registered redpiler events");

    let thread_server = server.clone();
    let data = plugin.data.clone();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread().enable_time().build().unwrap();
        runtime.block_on(tick_loop(data, thread_server));
    });


    Ok(())
}

async fn tick_loop(data: Arc<RwLock<PluginData>>, context: Arc<Context>) {
    let multiplier = 1000;
    loop {
        std::thread::sleep(Duration::from_millis(100));

        {
            let mut data = data.write().await;
            let Some(plot_data) = &mut data.plot else {
                continue;
            };
            plot_data.compiler.tickn(multiplier);
            let mut world = PumpkinWorld::new(plot_data.base);
            plot_data.compiler.flush(&mut world);
            world.apply(plot_data.world.clone()).await;
        }

    }
}

struct BreakHandler {
    data: Arc<RwLock<PluginData>>
}

#[with_runtime(global)]
#[async_trait]
impl EventHandler<BlockBreakEvent> for BreakHandler {
    async fn handle_blocking(&self, _server: &Arc<Server>, event: &mut BlockBreakEvent) {
        let pos = event.block_position;

        let mut data = self.data.write().await;

        let Some(plot_data) = &data.plot else {
            return;
        };

        let mchprs_pos = mchprs_blocks::BlockPos::new(
            pos.0.x - plot_data.base.x,
            pos.0.y - plot_data.base.y,
            pos.0.z - plot_data.base.z,
        );

        if !plot_data.plot.block_in_world(mchprs_pos) {
            return;
        }

        data.plot = None;
        log::info!("Invalidated plot");
    }
}

struct PlaceHandler {
    data: Arc<RwLock<PluginData>>
}

#[with_runtime(global)]
#[async_trait]
impl EventHandler<BlockPlaceEvent> for PlaceHandler {
    async fn handle_blocking(&self, _server: &Arc<Server>, event: &mut BlockPlaceEvent) {
        // TODO: only invalidate plot when change happens inside of it
        let mut data = self.data.write().await;

        data.plot = None;

        log::info!("Invalidated plot");
    }
}

struct InputHandler{
    data: Arc<RwLock<PluginData>>
}

#[with_runtime(global)]
#[async_trait]
impl EventHandler<PlayerInteractEvent> for InputHandler {
    async fn handle_blocking(&self, _server: &Arc<Server>, event: &mut PlayerInteractEvent) {
        let Some(pos) = event.clicked_pos else {
            return;
        };

        let mut data = self.data.write().await;

        let Some(plot_data) = &mut data.plot else {
            return;
        };

        let mchprs_pos = mchprs_blocks::BlockPos::new(
            pos.0.x - plot_data.base.x,
            pos.0.y - plot_data.base.y,
            pos.0.z - plot_data.base.z,
        );

        if !plot_data.plot.block_in_world(mchprs_pos) {
            log::info!("outside interact with block at {:?}", mchprs_pos);
            return;
        }
        
        if event.action.is_right_click() {
            data.plot = None;
            log::info!("Invalidated plot");
            return;
        }
        
        log::info!("interact with block at {:?}", mchprs_pos);


        if matches!(plot_data.plot.get_block(mchprs_pos), mchprs_blocks::blocks::Block::Air {  }) {
            return;
        }
        
        plot_data.compiler.on_use_block(mchprs_pos);

    }
}

#[plugin_impl]
pub struct MyPlugin {
    data: Arc<RwLock<PluginData>>,
}

impl MyPlugin {
    pub fn new() -> Self {
        println!("hello from redpiler plugin");

        MyPlugin {
            data: Arc::new(RwLock::new(PluginData::default())),
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
    plot: Option<PlotData>,
}

struct PlotData {
    world: Arc<pumpkin::world::World>,
    base: mchprs_blocks::BlockPos,
    plot: TestWorld,
    compiler: Compiler,
}

#[derive(Debug, Clone, Copy)]
enum Command {
    Compile,
    Pos1,
    Pos2,
    Deselect,
}

struct Exe {
    data: Arc<RwLock<PluginData>>,
    cmd: Command,
}

const AIR: u16 = Block::AIR.id;
const LEVER: u16 = Block::LEVER.id;
const STONE_BUTTON: u16 = Block::STONE_BUTTON.id;
const STONE_PRESSURE_PLATE: u16 = Block::STONE_PRESSURE_PLATE.id;
const REDSTONE_BLOCK: u16 = Block::REDSTONE_BLOCK.id;

const REDSTONE_LAMP: u16 = Block::REDSTONE_LAMP.id;
const IRON_TRAPDOOR: u16 = Block::IRON_TRAPDOOR.id;

const REDSTONE_WIRE: u16 = Block::REDSTONE_WIRE.id;
const REDSTONE_TORCH: u16 = Block::REDSTONE_TORCH.id;
const REDSTONE_WALL_TORCH: u16 = Block::REDSTONE_WALL_TORCH.id;

const REPEATER: u16 = Block::REPEATER.id;

const COMPARATOR: u16 = Block::COMPARATOR.id;
const BARREL: u16 = Block::BARREL.id;

const TARGET: u16 = Block::TARGET.id;

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
        log::info!(
            "player {:?}",
            sender.as_player().map(|p| p.position().to_block_pos())
        );

        let (Some(world), Some(player)) = (world, sender.as_player()) else {
            log::error!("Redpiler commands must be run by a player");
            return Err(CommandError::PermissionDenied);
        };

        match self.cmd {
            Command::Compile => {
                // TODO: Add all components including all containers
                // TODO: Pass along pending ticks
                // TODO: Parse compile flags

                for (a, b) in args.iter() {
                    if let Arg::Simple(simple) = b {
                        log::info!("{:?} {:?}", a, simple);
                    } else {
                        log::info!("{:?}", a);
                    }
                }

                let mut data = self.data.write().await;
                let (Some(p1), Some(p2)) = (data.pos1, data.pos2) else {
                    return Err(CommandError::PermissionDenied);
                };                

                let x1 = p1.0.x.min(p2.0.x);
                let x2 = p1.0.x.max(p2.0.x);
                let y1 = p1.0.y.min(p2.0.y);
                let y2 = p1.0.y.max(p2.0.y);
                let z1 = p1.0.z.min(p2.0.z);
                let z2 = p1.0.z.max(p2.0.z);


                sender.send_message(TextComponent::text(format!(
                        "Compiling selection {}, {}, {} ; {}, {}, {}",
                        x1, y1, z1, x2, y2, z2
                    )))
                    .await;

                let mut plot = fixed_world::TestWorld::new(x2 - x1, y2 - y1, z2 - z1);

                for z in z1..=z2 {
                    for y in y1..=y2 {
                        for x in x1..=x2 {
                            let pos = BlockPos::new(x, y, z);
                            let mchprs_pos = mchprs_blocks::BlockPos::new(x - x1, y - y1, z - z1);

                            let (b, s) = world.get_block_and_state(&pos).await;

                            let mchprs_block = match b.id {
                                AIR => continue,
                                REDSTONE_WIRE => {
                                    let props = RedstoneWireProperties::from_state_id(s.id, b);

                                    mchprs_blocks::blocks::Block::RedstoneWire {
                                        wire: RedstoneWire::new(
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
                                            props.power.to_index() as u8,
                                        ),
                                    }
                                }
                                STONE_BUTTON => {
                                    let props = LeverLikeProperties::from_state_id(s.id, b);

                                    mchprs_blocks::blocks::Block::StoneButton {
                                        button: mchprs_blocks::blocks::StoneButton::new(
                                            match props.face {
                                                block_properties::BlockFace::Floor => {
                                                    mchprs_blocks::blocks::ButtonFace::Floor
                                                }
                                                block_properties::BlockFace::Wall => {
                                                    mchprs_blocks::blocks::ButtonFace::Wall
                                                }
                                                block_properties::BlockFace::Ceiling => {
                                                    mchprs_blocks::blocks::ButtonFace::Ceiling
                                                }
                                            },
                                            direction_to_mchprs(props.facing),
                                            props.powered,
                                        ),
                                    }
                                }
                                LEVER => {
                                    let props = LeverLikeProperties::from_state_id(s.id, b);

                                    mchprs_blocks::blocks::Block::Lever {
                                        lever: Lever::new(
                                            match props.face {
                                                block_properties::BlockFace::Floor => {
                                                    mchprs_blocks::blocks::LeverFace::Floor
                                                }
                                                block_properties::BlockFace::Wall => {
                                                    mchprs_blocks::blocks::LeverFace::Wall
                                                }
                                                block_properties::BlockFace::Ceiling => {
                                                    mchprs_blocks::blocks::LeverFace::Ceiling
                                                }
                                            },
                                            direction_to_mchprs(props.facing),
                                            props.powered,
                                        ),
                                    }
                                }
                                STONE_PRESSURE_PLATE => {
                                    let props =
                                        StonePressurePlateLikeProperties::from_state_id(s.id, b);
                                    mchprs_blocks::blocks::Block::StonePressurePlate {
                                        powered: props.powered,
                                    }
                                }
                                REDSTONE_BLOCK => mchprs_blocks::blocks::Block::RedstoneBlock {},
                                REDSTONE_LAMP => {
                                    let props = RedstoneLampProperties::from_state_id(s.id, b);

                                    mchprs_blocks::blocks::Block::RedstoneLamp { lit: props.lit }
                                }
                                IRON_TRAPDOOR => {
                                    let props = OakTrapdoorLikeProperties::from_state_id(s.id, b);

                                    mchprs_blocks::blocks::Block::IronTrapdoor {
                                        facing: direction_to_mchprs(props.facing),
                                        half: match props.half {
                                            block_properties::BlockHalf::Top => {
                                                mchprs_blocks::blocks::TrapdoorHalf::Top
                                            }
                                            block_properties::BlockHalf::Bottom => {
                                                mchprs_blocks::blocks::TrapdoorHalf::Bottom
                                            }
                                        },
                                        powered: props.powered,
                                    }
                                }
                                REDSTONE_TORCH => {
                                    let props = RTorchProps::from_state_id(s.id, b);

                                    mchprs_blocks::blocks::Block::RedstoneTorch { lit: props.lit }
                                }
                                REDSTONE_WALL_TORCH => {
                                    let props = RWallTorchProps::from_state_id(s.id, b);

                                    mchprs_blocks::blocks::Block::RedstoneWallTorch {
                                        lit: props.lit,
                                        facing: direction_to_mchprs(props.facing),
                                    }
                                }
                                REPEATER => {
                                    let props = RepeaterLikeProperties::from_state_id(s.id, b);

                                    mchprs_blocks::blocks::Block::RedstoneRepeater {
                                        repeater: RedstoneRepeater {
                                            delay: 1 + props.delay.to_index() as u8,
                                            facing: direction_to_mchprs(props.facing),
                                            locked: props.locked,
                                            powered: props.powered,
                                        },
                                    }
                                }
                                COMPARATOR => {
                                    let props = ComparatorLikeProperties::from_state_id(s.id, b);

                                    if let Some(entity) = world.get_block_entity(&pos).await {
                                        if let Some(entity) = entity.as_any().downcast_ref::<pumpkin_world::block::entities::comparator::ComparatorBlockEntity>() {
                                            plot.set_block_entity(mchprs_pos, mchprs_blocks::block_entities::BlockEntity::Comparator { output_strength: entity.output_signal.load(Ordering::Relaxed) });
                                        }
                                    }

                                    mchprs_blocks::blocks::Block::RedstoneComparator {
                                        comparator: RedstoneComparator::new(
                                            direction_to_mchprs(props.facing),
                                            match props.mode {
                                                block_properties::ComparatorMode::Compare => {
                                                    mchprs_blocks::blocks::ComparatorMode::Compare
                                                }
                                                block_properties::ComparatorMode::Subtract => {
                                                    mchprs_blocks::blocks::ComparatorMode::Subtract
                                                }
                                            },
                                            props.powered,
                                        ),
                                    }
                                }
                                BARREL => {
                                    let props = BarrelLikeProperties::from_state_id(s.id, b);

                                    if let Some(entity) = world.get_block_entity(&pos).await {
                                        if let Some(entity) = entity.as_any().downcast_ref::<pumpkin_world::block::entities::barrel::BarrelBlockEntity>() {
                                            let ty = mchprs_blocks::block_entities::ContainerType::Barrel;
                                            let num_slots = entity.items.len();
                                            let mut fullness_sum: f32 = 0.0;
                                            // TODO: fill inventory
                                            let inventory = Vec::new();

        
                                            for slot in &entity.items {
                                                let slot = slot.lock().await;
                                                let count = slot.item_count;
                                                if count == 0 {
                                                    continue;
                                                }

                                                let mut max_stack_size = 64;
                                                let item = slot.item;
                                                for component in item.components {
                                                    if component.0 == pumpkin_data::data_component::DataComponent::MaxStackSize {
                                                        if let Some(size) = component.1.as_any().downcast_ref::<pumpkin_data::data_component_impl::MaxStackSizeImpl>() {
                                                            max_stack_size = size.size;
                                                        }
                                                    }
                                                }
                                                
                                                fullness_sum += count as f32 / max_stack_size as f32;
                                            }

                                            let comparator_override = (if fullness_sum > 0.0 { 1.0 } else { 0.0 }
                                                + (fullness_sum / num_slots as f32) * 14.0)
                                                .floor() as u8;

                                            plot.set_block_entity(mchprs_pos, mchprs_blocks::block_entities::BlockEntity::Container { comparator_override, inventory, ty });
                                        }
                                    }

                                    mchprs_blocks::blocks::Block::Barrel {}
                                }
                                TARGET => mchprs_blocks::blocks::Block::Target {},
                                _ => {
                                    if let Some(block) =
                                        mchprs_blocks::blocks::Block::from_name(b.name)
                                    {
                                        block
                                    } else {
                                        let solid = s.is_solid();

                                        // sender.send_message(TextComponent::text(format!("Unknown block {:?}", solid))).await;

                                        if solid {
                                            mchprs_blocks::blocks::Block::IronBlock {}
                                        } else {
                                            mchprs_blocks::blocks::Block::Glass {}
                                        }
                                    }
                                }
                            };

                            plot.set_block(mchprs_pos, mchprs_block);
                        }
                    }
                }

                let min_pos = mchprs_blocks::BlockPos::new(0, 0, 0);
                let max_pos =
                    mchprs_blocks::BlockPos::new(plot.size_x - 1, plot.size_y - 1, plot.size_z - 1);

                let mut compiler = Compiler::default();
                let bounds = (min_pos, max_pos);
                let options = CompilerOptions {
                    optimize: false,
                    io_only: false,
                    wire_dot_out: true,
                    backend_variant: BackendVariant::Direct,
                    export_dot_graph: true,
                    ..Default::default()
                };
                let ticks = plot.to_be_ticked.drain(..).collect();
                let monitor = Default::default();
                compiler.compile(&mut plot, bounds, options, ticks, monitor);
                
                data.plot = Some(PlotData { base: mchprs_blocks::BlockPos::new(x1, y1, z1), plot, compiler, world: world.clone() });

                sender
                    .send_message(TextComponent::text(format!("Compiled successfully")))
                    .await;
            }
            Command::Pos1 => {
                let mut data = self.data.write().await;
                data.pos1 = Some(player.position().sub_raw(0.5, 0.5, 0.5).to_block_pos());
            }
            Command::Pos2 => {
                let mut data = self.data.write().await;
                data.pos2 = Some(player.position().sub_raw(0.5, 0.5, 0.5).to_block_pos());
            }
            Command::Deselect => {
                let mut data = self.data.write().await;
                data.pos1 = None;
                data.pos2 = None;
            }
        }

        Ok(())
    }
}

fn facing_to_mchprs(face: block_properties::Facing) -> mchprs_blocks::BlockFacing {
    match face {
        block_properties::Facing::North => mchprs_blocks::BlockFacing::North,
        block_properties::Facing::East => mchprs_blocks::BlockFacing::East,
        block_properties::Facing::South => mchprs_blocks::BlockFacing::South,
        block_properties::Facing::West => mchprs_blocks::BlockFacing::West,
        block_properties::Facing::Up => mchprs_blocks::BlockFacing::Up,
        block_properties::Facing::Down => mchprs_blocks::BlockFacing::Down,
    }
}


fn direction_to_mchprs(face: HorizontalFacing) -> mchprs_blocks::BlockDirection {
    match face {
        HorizontalFacing::North => mchprs_blocks::BlockDirection::North,
        HorizontalFacing::East => mchprs_blocks::BlockDirection::East,
        HorizontalFacing::South => mchprs_blocks::BlockDirection::South,
        HorizontalFacing::West => mchprs_blocks::BlockDirection::West,
    }
}
