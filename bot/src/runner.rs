use std::process::Stdio;

use poise::serenity_prelude::{CreateMessage, Http, UserId, futures::channel::oneshot};
use tokio::{io::BufReader, process::Command, task::JoinSet};
use tokio_util::io::SyncIoBridge;
use worker::{Request, Response};

use crate::{Data, Error};

pub async fn handle_benchmark(
    http: &Http,
    data: &Data,
    user: UserId,
    day: u8,
    part: u8,
    code: Vec<u8>,
) -> Result<(), Error> {
    let database = &data.database;

    let uuser = user.to_user(http).await?;

    let rid = database.insert_run(user, day, part, &code).await?;

    let mut input_watch = data.input_watch.subscribe();
    let mut consensus_watch = data.consensus_watch.subscribe();

    while database.inputs_count(day).await? < 3 {
        input_watch.wait_for(|&d| day == d).await?;
    }

    let (ids, inputs) = database.fetch_inputs(day, 3).await?;

    let res = run_container(rid, inputs, code).await?;

    if let Some(Err(err)) = res.outputs.iter().find(|res| res.is_err()) {
        uuser
            .direct_message(
                http,
                CreateMessage::new().content(format!(
                    "Run failed with the following output:\n```{err}```"
                )),
            )
            .await?;
        return Ok(());
    }

    let outputs = res
        .outputs
        .into_iter()
        .map(|o| o.unwrap())
        .collect::<Vec<_>>();
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
            uuser
                .direct_message(
                    http,
                    CreateMessage::new().content(format!(
                        "The solution provided by your code did not match the consensus solution."
                    )),
                )
                .await?;
            return Ok(());
        }
        avg_time += time;
    }
    avg_time /= res.times.len() as u64;

    database.update_run(rid, avg_time as _).await?;

    Ok(())
}

async fn run_container(id: i64, inputs: Vec<Vec<u8>>, code: Vec<u8>) -> Result<Response, Error> {
    let name = format!("runner-{id}");
    let mut child = Command::new("docker")
        .args(&[
            "run",
            "--cap-drop",
            "--net",
            "none",
            "--cpus",
            "2",
            "--memory",
            "512m",
            "--memory-swap",
            "640m",
            "--oom-score-adj",
            "1000",
        ])
        .args(&["--name", &name])
        .arg("-i")
        .args(["-a", "stdin", "-a", "stdout", "-a", "stderr"])
        .arg("--rm")
        .arg("runner")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let mut io = JoinSet::<Result<(), Error>>::new();

    let stdin = child.stdin.take().unwrap();
    let stdout = BufReader::new(child.stdout.take().unwrap());
    let (tx, rx) = oneshot::channel();

    io.spawn_blocking(move || {
        let mut stdin = SyncIoBridge::new(stdin);
        let req = Request { id, inputs, code };

        bincode::serialize_into(&mut stdin, &req)?;
        Ok(())
    });
    io.spawn_blocking(move || {
        let mut stdout = SyncIoBridge::new(stdout);
        let res = bincode::deserialize_from(&mut stdout)?;
        tx.send(res).unwrap();
        Ok(())
    });

    io.join_all().await;

    let res = rx.await?;
    Ok(res)
}
