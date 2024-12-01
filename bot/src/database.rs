use std::str::FromStr;

use poise::serenity_prelude::UserId;
use sqlx::{
    Row, SqlitePool,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqliteSynchronous},
};

use crate::{Error, runner::MIN_SOLUTIONS_PER_INPUT};

pub const DEFAULT_PATH: &str = "database.db";
pub const DEFAULT_MIN_SOLUTIONS: usize = 3;

pub struct Database(SqlitePool);

pub struct Score {
    pub user: UserId,
    pub score: f64,
}

impl Database {
    pub async fn init(path: &str) -> Result<Self, Error> {
        let url = format!("sqlite://{path}");
        let options = SqliteConnectOptions::from_str(&url)?
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .optimize_on_close(true, None);
        let pool = SqlitePool::connect_with(options).await?;

        {
            let mut tx = pool.begin().await?;

            sqlx::query(
                "
CREATE TABLE IF NOT EXISTS inputs(
    id INTEGER PRIMARY KEY,
    day INTEGER,
    submitter INTEGER,
    data BLOB
)",
            )
            .execute(&mut *tx)
            .await?;

            sqlx::query(
                "
CREATE TABLE IF NOT EXISTS solutions(
    id INTEGER PRIMARY KEY,
    input_id INTEGER,
    part INTEGER,
    submitter INTEGER,
    answer INTEGER,
    FOREIGN KEY (input_id)
        REFERENCES inputs(id)
        ON DELETE CASCADE
        ON UPDATE SET NULL
)",
            )
            .execute(&mut *tx)
            .await?;

            sqlx::query(
                "
CREATE TABLE IF NOT EXISTS runs(
    id INTEGER PRIMARY KEY,
    submitter INTEGER,
    day INTEGER,
    part INTEGER,
    score NUMERIC,
    code BLOB
)",
            )
            .execute(&mut *tx)
            .await?;

            tx.commit().await?;
        }
        Ok(Self(pool))
    }

    pub async fn insert_input(&self, user: UserId, day: u8, input: &[u8]) -> Result<(), Error> {
        sqlx::query("INSERT INTO inputs (day, submitter, data) VALUES(?, ?, ?)")
            .bind(user.get() as i64)
            .bind(day)
            .bind(input)
            .execute(&self.0)
            .await?;
        Ok(())
    }

    pub async fn insert_solution(
        &self,
        user: UserId,
        input: i64,
        part: u8,
        answer: i64,
    ) -> Result<(), Error> {
        sqlx::query(
            "INSERT INTO solutions (input_id, part, submitter, answer) VALUES (?, ?, ?, ?)",
        )
        .bind(input)
        .bind(part)
        .bind(user.get() as i64)
        .bind(answer)
        .execute(&self.0)
        .await?;
        Ok(())
    }

    pub async fn insert_run(
        &self,
        user: UserId,
        day: u8,
        part: u8,
        code: &[u8],
    ) -> Result<i64, Error> {
        let res = sqlx::query("INSERT INTO runs (submitter, day, part, code) VALUES (?, ?, ?, ?)")
            .bind(user.get() as i64)
            .bind(day)
            .bind(part)
            .bind(code)
            .execute(&self.0)
            .await?;
        Ok(res.last_insert_rowid())
    }

    pub async fn update_run(&self, id: i64, score: i64) -> Result<(), Error> {
        sqlx::query("UPDATE runs SET score = ? WHERE id = ? LIMIT 1")
            .bind(score)
            .bind(id)
            .execute(&self.0)
            .await?;
        Ok(())
    }

    pub async fn fetch_inputs(
        &self,
        day: u8,
        limit: usize,
    ) -> Result<(Vec<i64>, Vec<Vec<u8>>), Error> {
        let res = sqlx::query("SELECT id, data FROM inputs WHERE day = ? LIMIT ?")
            .bind(day)
            .bind(limit as i64)
            .fetch_all(&self.0)
            .await?;
        let res = res
            .iter()
            .map(|row| (row.get::<i64, _>(0), row.get(1)))
            .unzip();
        Ok(res)
    }

    pub async fn inputs_count(&self, day: u8) -> Result<usize, Error> {
        let res = sqlx::query("SELECT COUNT(*) FROM inputs WHERE day = ?")
            .bind(day)
            .fetch_one(&self.0)
            .await?;
        Ok(res.get::<i64, _>(0) as _)
    }

    pub async fn solutions_count(&self, day: u8, part: u8) -> Result<usize, Error> {
        let res = sqlx::query("SELECT COUNT(*) FROM solutions WHERE day = ? AND part = ?")
            .bind(day)
            .bind(part)
            .fetch_one(&self.0)
            .await?;
        Ok(res.get::<i64, _>(0) as _)
    }

    pub async fn solution_consensus(&self, input_id: i64) -> Result<Option<i64>, Error> {
        let count: i64 = sqlx::query("SELECT COUNT(*) FROM solutions WHERE id = ?")
            .bind(input_id)
            .fetch_one(&self.0)
            .await?
            .get(0);
        let majority = count / 2;
        if majority < MIN_SOLUTIONS_PER_INPUT as i64 {
            return Ok(None);
        }

        let res = sqlx::query(
            "SELECT answer, COUNT(answer) FROM solutions WHERE id = ? GROUP BY answer ORDER BY COUNT(answer) DESC",
        ).bind(input_id)
        .fetch_optional(&self.0)
        .await?;
        if let Some(res) = res {
            let count: i64 = res.get(1);
            if count > majority {
                let res = res.get(0);
                return Ok(Some(res));
            }
            return Ok(None);
        }
        Ok(None)
    }

    pub async fn fetch_scores_for_day(&self, day: u8) -> Result<(Vec<Score>, Vec<Score>), Error> {
        // Greatest N Per Group? YAGNI, just run the query twice
        let part1 = sqlx::query(
            "SELECT submitter, score FROM runs 
                WHERE day = ? AND part = 1
                GROUP BY submitter
                HAVING MIN(score) 
                ORDER BY score ASC 
                LIMIT 10",
        )
        .bind(day)
        .fetch_all(&self.0)
        .await?;
        let part2 = sqlx::query(
            "SELECT submitter, score FROM runs 
            WHERE day = ? AND part = 2
            GROUP BY submitter
            HAVING MIN(score) 
            ORDER BY score ASC 
            LIMIT 10",
        )
        .bind(day)
        .fetch_all(&self.0)
        .await?;

        let part1 = part1
            .iter()
            .map(|row| Score {
                user: (row.get::<i64, _>(0) as u64).into(),
                score: row.get(1),
            })
            .collect();
        let part2 = part2
            .iter()
            .map(|row| Score {
                user: (row.get::<i64, _>(0) as u64).into(),
                score: row.get(1),
            })
            .collect();
        Ok((part1, part2))
    }
}
