use std::process::Stdio;

use poise::serenity_prelude::futures::channel::oneshot;
use tokio::{
    io::{AsyncWriteExt, BufReader},
    process::Command,
    task::JoinSet,
};
use tokio_util::io::SyncIoBridge;
use worker::{Request, Response};

use crate::Error;

pub const MIN_INPUTS: usize = 3;
pub const MIN_SOLUTIONS_PER_INPUT: usize = 3;

pub async fn run_benchmark(
    id: i64,
    inputs: Vec<Vec<u8>>,
    code: Vec<u8>,
) -> Result<Response, Error> {
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
