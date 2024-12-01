use std::sync::Arc;

use crate::{
    Context, Error,
    runner::handle_benchmark,
    utils::{aoc_today, get_name},
};

use poise::{
    CreateReply,
    serenity_prelude::{
        self as serenity, CreateEmbed,
        futures::{StreamExt, stream},
    },
};

#[poise::command(slash_command, subcommands("input", "run", "leaderboard"))]
pub async fn aoc(_: Context<'_>) -> Result<(), Error> {
    Ok(())
}

#[poise::command(slash_command)]
pub async fn input(
    ctx: Context<'_>,
    #[description = "File containing the input for the day."] file: serenity::Attachment,
    #[description = "The day this input is for. Defaults to today."] day: Option<u8>,
) -> Result<(), Error> {
    let today = aoc_today();
    let day = day.unwrap_or(today);
    if day > today {
        ctx.say("Cannot accept input from the future!").await?;
        return Ok(());
    }

    let database = &ctx.data().database;
    if database.inputs_count(day).await? >= 3 {
        ctx.say("There's enough inputs for today! Thank you anyway!")
            .await?;
        return Ok(());
    }

    let user = ctx.author().id;
    let input = file.download().await?;

    let (_, inputs) = database.fetch_inputs(day, 3).await?;
    if inputs.iter().any(|i| *i == input) {
        ctx.say("Already have this input! Thank you anyway!")
            .await?;
        return Ok(());
    }

    database.insert_input(user, day, &input).await?;

    let sender = &ctx.data().input_watch;
    if !sender.is_closed() {
        sender.send(day)?;
    }

    ctx.say("Thank you for your input!").await?;

    Ok(())
}

#[poise::command(slash_command)]
async fn run(
    ctx: Context<'_>,
    #[description = "File containing the code to run."] file: serenity::Attachment,
    #[description = "The day this code is for. Defaults to today."] day: Option<u8>,
    #[description = "The part this code is for."] part: u8,
) -> Result<(), Error> {
    let today = aoc_today();
    let day = day.unwrap_or(today);
    if day > today {
        ctx.say("Cannot run from the future!").await?;
        return Ok(());
    };

    let user = ctx.author().id;

    let code = file.download().await?;

    let http = Arc::clone(&ctx.serenity_context().http);
    let data = Arc::clone(ctx.data());
    tokio::spawn(async move { handle_benchmark(&http, &data, user, day, part, code).await });

    ctx.say("Your submission has been queued.").await?;

    Ok(())
}

#[poise::command(slash_command)]
pub async fn leaderboard(
    ctx: Context<'_>,
    #[description = ""] day: Option<u8>,
) -> Result<(), Error> {
    let today = aoc_today();
    let day = day.unwrap_or(today);
    if day > today {
        ctx.say("Cannot get leaderboard from the future!").await?;
        return Ok(());
    }

    let (part1, part2) = ctx.data().database.fetch_scores_for_day(day).await?;
    // Assume if theres no part 1, then there couldn't be a part 2
    if part1.is_empty() {
        ctx.say("No runs on the leaderboard yet. Be the first!")
            .await?;
        return Ok(());
    }
    let part1 = stream::iter(part1)
        .then(|score| async move {
            let name = get_name(&ctx, score.user).await;
            format!("\t{}: **{}**\n", name, score.score)
        })
        .collect::<String>()
        .await;
    let part2 = if !part2.is_empty() {
        stream::iter(part2)
            .then(|score| async move {
                let name = get_name(&ctx, score.user).await;
                format!("\t{}: **{}**\n", name, score.score)
            })
            .collect::<String>()
            .await
    } else {
        "**None**".to_owned()
    };

    let embed = CreateEmbed::new()
        .title(format!("Top 10 Fastest Toboggans For Day {day}"))
        .colour(0xE84611)
        .field("Part 1", part1, true)
        .field("Part 2", part2, true);
    ctx.send(CreateReply::default().embed(embed)).await?;

    Ok(())
}
