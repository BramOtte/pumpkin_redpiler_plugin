use std::sync::Arc;

use async_trait::async_trait;
use pumpkin_api_macros::{plugin_impl, plugin_method};


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
        .then(literal("compile").execute(RockPaperScissorsExecutor));
    
    server.register_command(command, permission_node).await;
    println!("registered redpiler command");

    Ok(())
}

#[plugin_impl]
pub struct MyPlugin;

impl MyPlugin {
    pub fn new() -> Self {
        println!("hello from redpiler plugin");

        MyPlugin
    }
}

impl Default for MyPlugin {
    fn default() -> Self {
        Self::new()
    }
}

use pumpkin::{
    command::{ 
        args::ConsumedArgs, dispatcher::CommandError, tree::builder::literal, tree::CommandTree,
        CommandExecutor, CommandSender,
    },
    plugin::{player::player_join::PlayerJoinEvent, Context, EventHandler, EventPriority},
    server::Server,
};
use pumpkin_util::{math::position::BlockPos, permission::{Permission, PermissionDefault}, PermissionLvl};

struct RockPaperScissorsExecutor; 

#[async_trait] 
impl CommandExecutor for RockPaperScissorsExecutor {
    async fn execute<'a>(
        &self,
        sender: &mut CommandSender,
        _: &Server,
        _: &ConsumedArgs<'a>,
    ) -> Result<(), CommandError> {
        let world = sender.world().await.unwrap();

        log::info!("hello execute");
        println!("redpiler execute");

        Ok(())
    }
}