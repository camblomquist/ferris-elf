use poise::serenity_prelude::UserId;
use time::{OffsetDateTime, macros::offset};

use crate::Context;

/// Today as defined by the Advent of Code Timezone
pub fn aoc_today() -> u8 {
    OffsetDateTime::now_utc().to_offset(offset!(-5:00)).day()
}

pub async fn get_name(ctx: &Context<'_>, user: UserId) -> String {
    let user = user.to_user(ctx).await;
    match user {
        Ok(user) => {
            if let Some(gid) = ctx.guild_id() {
                if let Some(name) = user.nick_in(ctx, gid).await {
                    name
                } else {
                    user.global_name.unwrap_or(user.name)
                }
            } else {
                user.global_name.unwrap_or(user.name)
            }
        }
        _ => "Unknown User".to_owned(),
    }
}
