use regex::Regex;
use teloxide::{adaptors::AutoSend, prelude::Requester, types::ChatId, Bot};

use crate::{
    argument_parser::parse_arguments,
    autoreplies::{Autoreply, AutoreplyResponse, AutoreplySet, AutoreplySetMap},
    db::DatabaseRef,
};

pub async fn handle_add_message(
    bot: &AutoSend<Bot>,
    chat_id: ChatId,
    db: DatabaseRef,
    autoreply_set_map: AutoreplySetMap,
    args: &str,
) -> anyhow::Result<()> {
    let args = parse_arguments(args);

    match args {
        Err(err) => {
            bot.send_message(
                chat_id,
                format!("Parametrien parsinta epäonnistui: {}", err),
            )
            .await?;

            return Ok(());
        }
        Ok((_, args)) => {
            if args.len() != 3 {
                bot.send_message(
                    chat_id,
                    "Parametrien määrä väärin. Käytä muotoa: /addmessage <nimi> <regex> <viesti>",
                )
                .await?;

                return Ok(());
            }

            let name = &args[0];
            let pattern_regex = Regex::new(&args[1]);

            let pattern_regex = match pattern_regex {
                Ok(regex) => regex,
                Err(err) => {
                    bot.send_message(
                        chat_id,
                        format!("Regex-lausekkeen parsinta epäonnistui: {}", err),
                    )
                    .await?;

                    return Ok(());
                }
            };

            let response = &args[2];

            let autoreply = Autoreply {
                chat_id,
                name: name.to_string(),
                pattern_regex,
                response: AutoreplyResponse::Literal(response.to_string()),
            };

            db.add_autoreply(&autoreply).await?;

            let mut autoreply_set_map = autoreply_set_map.write().await;
            autoreply_set_map
                .entry(chat_id)
                .or_insert_with(AutoreplySet::empty)
                .add_autoreply(autoreply);

            bot.send_message(
                chat_id,
                format!("🎉 Lisätty automaattinen vastaus {}", name),
            )
            .await?;
        }
    }

    Ok(())
}
