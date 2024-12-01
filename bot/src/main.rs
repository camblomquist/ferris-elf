use std::{env, sync::Arc};

use database::Database;
use poise::serenity_prelude::{self as serenity};

use tokio::{
    signal::unix::{SignalKind, signal},
    sync::watch,
};

mod commands;
mod database;
mod runner;
mod utils;

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Arc<Data>, Error>;

pub struct Data {
    database: Database,
    input_watch: watch::Sender<u8>,
    consensus_watch: watch::Sender<i64>,
}

#[poise::command(slash_command)]
async fn stub(_: Context<'_>) -> Result<(), Error> {
    Ok(())
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let token = env::var("DISCORD_TOKEN").expect("missing DISCORD_TOKEN");

    let db_path = env::var("DATABASE_PATH").unwrap_or_else(|_| database::DEFAULT_PATH.into());

    let commands = vec![commands::aoc(), commands::input()];

    let options = poise::FrameworkOptions {
        commands,
        ..Default::default()
    };

    let framework = poise::Framework::builder()
        .setup(move |_ctx, _ready, framework| {
            Box::pin(async move {
                let database = Database::init(&db_path).await?;

                let shard_manager = framework.shard_manager().clone();
                tokio::spawn(async move {
                    let mut signal = signal(SignalKind::terminate()).unwrap();
                    signal.recv().await.unwrap();

                    log::info!("Stopping client...");

                    shard_manager.shutdown_all().await;
                });

                let (input_watch, _) = watch::channel(0);
                let (consensus_watch, _) = watch::channel(0);

                Ok(Arc::new(Data {
                    database,
                    input_watch,
                    consensus_watch,
                }))
            })
        })
        .options(options)
        .build();
    let intents = serenity::GatewayIntents::non_privileged();
    let client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .await;

    log::info!("Starting client...");

    client.unwrap().start().await.unwrap();

    log::info!("Client stopped");
}
