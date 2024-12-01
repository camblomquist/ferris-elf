use std::io::{self, ErrorKind};

use crate::{
    Context, Error,
    runner::run_benchmark,
    utils::{aoc_today, get_name},
};

use poise::{
    CreateReply,
    serenity_prelude::{
        self as serenity, CreateEmbed,
        futures::{StreamExt, stream},
    },
};
use tokio::spawn;

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
    let min_inputs = ctx.data().min_inputs;
    if database.inputs_count(day).await? >= min_inputs {
        ctx.say("There's enough inputs for today! Thank you anyway!")
            .await?;
        return Ok(());
    }

    let user = ctx.author().id;
    let input = file.download().await?;

    let inputs = database.fetch_inputs(day, min_inputs).await?;
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
    let database = &ctx.data().database;

    let code = file.download().await?;

    let rid = database.insert_run(user, day, part, &code).await?;

    let mut input_watch = ctx.data().input_watch.subscribe();
    let mut consensus_watch = ctx.data().consensus_watch.subscribe();
    tokio::spawn(async move {
        while database.inputs_count(day).await? < 3 {
            input_watch.wait_for(|&d| day == d).await?;
        }

        let (ids, inputs) = database.fetch_inputs(day, 3).await?;

        let res = run_benchmark(rid, inputs, code).await?;

        if res.outputs.iter().any(|res| res.is_err()) {
            // TODO: Whine about this
            return Ok(());
        }

        let outputs = res.outputs.iter().map(|o| o.unwrap()).collect::<Vec<_>>();
        for (&res, &id) in outputs.iter().zip(&ids) {
            database.insert_solution(user, id, part, res).await?;
        }

        let mut avg_time = 0;
        for ((&res, &id), &time) in outputs.iter().zip(&ids).zip(&res.times) {
            while database.solution_consensus(id).await?.is_none() {
                consensus_watch.wait_for(|&i| id == i).await?;
            }
            let consensus = database.solution_consensus(id).await?.unwrap();
            if res != consensus {
                // TODO: Whine that the answer is incorrect
                return Ok(());
            }
            avg_time += time;
        }
        avg_time /= res.times.len() as u64;

        database.update_run(rid, avg_time as _).await?;

        Ok(())
    });

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
