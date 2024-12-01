use core::str;
use std::{
    ffi::OsStr,
    io::{BufReader, BufWriter, Write},
    os::unix::ffi::OsStrExt,
};

use tokio::{fs::write, process::Command, select, sync::mpsc, task::JoinSet};
use worker::{Request, Response};

type Error = Box<dyn std::error::Error + Send + Sync>;

const RUNNER_DIR: &'static str = "/runner";

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Error> {
    let (in_tx, in_rx) = mpsc::channel(4);
    let (out_tx, mut out_rx) = mpsc::channel(4);

    let handler = tokio::spawn(handle_messages(in_rx, out_tx));

    let mut io = JoinSet::<Result<(), Error>>::new();

    io.spawn_blocking(move || {
        let stdin = std::io::stdin();
        let mut stdin = BufReader::new(stdin);

        loop {
            let req = bincode::deserialize_from(&mut stdin);
            if req.as_ref().is_err_and(|e| {
                if let bincode::ErrorKind::Io(e) = &**e {
                    return e.kind() == std::io::ErrorKind::UnexpectedEof;
                }
                false
            }) {
                break;
            }
            let req = req?;
            in_tx.blocking_send(req)?;
        }
        Ok(())
    });

    io.spawn_blocking(move || {
        let stdout = std::io::stdout();
        let mut stdout = BufWriter::new(stdout);

        while let Some(res) = out_rx.blocking_recv() {
            bincode::serialize_into(&mut stdout, &res)?;
            stdout.flush()?;
        }
        Ok(())
    });

    select! {
        Some(task) = io.join_next() => {
            task?
        },
        handler = handler => {
            handler?
        }
    }
}

async fn handle_messages(
    mut in_rx: mpsc::Receiver<Request>,
    out_tx: mpsc::Sender<Response>,
) -> Result<(), Error> {
    while let Some(req) = in_rx.recv().await {
        write(format!("{RUNNER_DIR}/src/lib.rs"), req.code).await?;
        let res = benchmark(&req.inputs).await?;
        let res = Response {
            id: req.id,
            outputs: res.0,
            times: res.1,
        };
        out_tx.send(res).await?;
    }

    Ok(())
}

async fn benchmark(inputs: &[Vec<u8>]) -> Result<(Vec<Result<i64, String>>, Vec<u64>), Error> {
    let inputs = inputs
        .iter()
        .enumerate()
        .map(|(i, input)| (format!("INPUT_{i}"), OsStr::from_bytes(input)));
    let output = Command::new("cargo")
        .args(&[
            "bench",
            "--bench",
            "bench",
            "--quiet",
            "--offline",
            "--",
            "--nocapture",
        ])
        .envs(inputs)
        .current_dir(RUNNER_DIR)
        .output()
        .await?;
    if output.status.success() {
        let mut solutions = Vec::with_capacity(3);
        let mut times = Vec::with_capacity(3);
        let output = str::from_utf8(&output.stdout)?;
        for line in output.lines() {
            let line = line.trim_start();
            let (thing, rest) = line.split_once(':').unwrap();
            let rest = rest.trim();
            match thing {
                "Solution" => solutions.push(Ok(rest.parse().unwrap())),
                "Instructions" => {
                    let val = rest.split_once('|').unwrap().0;
                    times.push(val.parse().unwrap());
                }
                _ => (),
            }
        }
        Ok((solutions, times))
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into())
    }
}
