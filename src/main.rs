use std::env;

use poise::serenity_prelude::{self as serenity};

use tokio::signal::unix::{SignalKind, signal};

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

pub struct Data {}

#[poise::command(slash_command)]
async fn stub(_: Context<'_>) -> Result<(), Error> {
    Ok(())
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let token = env::var("DISCORD_TOKEN").expect("missing DISCORD_TOKEN");

    let commands = vec![stub()];

    let options = poise::FrameworkOptions {
        commands,
        ..Default::default()
    };

    let framework = poise::Framework::builder()
        .setup(move |_ctx, _ready, framework| {
            Box::pin(async move {
                let shard_manager = framework.shard_manager().clone();
                tokio::spawn(async move {
                    let mut signal = signal(SignalKind::terminate()).unwrap();
                    signal.recv().await.unwrap();

                    log::info!("Stopping client...");

                    shard_manager.shutdown_all().await;
                });
                Ok(Data {})
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
