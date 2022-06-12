use regex::Regex;
use teloxide::{
    adaptors::AutoSend,
    prelude::Requester,
    types::{ChatId, Message},
    Bot,
};

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
            if args.len() == 2 {
                bot.send_message(
                    chat_id,
                    "👀 Lähetä haluttu viesti tai tarra vastauksena alkuperäiseen komentoon.",
                )
                .await?;

                return Ok(());
            }

            if args.len() != 3 {
                bot.send_message(
                    chat_id,
                    "Parametrien määrä väärin. Käytä muotoa: /addmessage <nimi> <regex> <viesti>",
                )
                .await?;

                return Ok(());
            }

            let name = &args[0];
            let pattern_regex = parse_regex(&args, bot, chat_id).await?;

            let pattern_regex = match pattern_regex {
                Some(regex) => regex,
                None => {
                    return Ok(());
                }
            };

            let response = &args[2];

            add_message(
                chat_id,
                name,
                pattern_regex,
                db,
                autoreply_set_map,
                bot,
                AutoreplyResponse::Literal(response.to_string()),
            )
            .await?;
        }
    }

    Ok(())
}

pub async fn handle_add_message_reply(
    bot: &AutoSend<Bot>,
    db: DatabaseRef,
    autoreply_set_map: AutoreplySetMap,
    message: &Message,
    previous_args: &str,
) -> anyhow::Result<()> {
    let previous_args = parse_arguments(previous_args);
    let chat_id = message.chat.id;

    match previous_args {
        Err(err) => {
            bot.send_message(
                chat_id,
                format!("Komennon parametrien parsinta epäonnistui: {}", err),
            )
            .await?;

            return Ok(());
        }
        Ok((_, args)) => {
            if args.len() != 2 {
                bot.send_message(chat_id, "Parametrien määrä väärin.")
                    .await?;

                return Ok(());
            }

            let name = &args[0];
            let pattern_regex = parse_regex(&args, bot, chat_id).await?;

            let pattern_regex = match pattern_regex {
                Some(regex) => regex,
                None => {
                    return Ok(());
                }
            };

            let response = if let Some(sticker) = message.sticker() {
                AutoreplyResponse::Sticker(sticker.file_id.clone())
            } else {
                AutoreplyResponse::Literal(
                    message
                        .text()
                        .map(String::from)
                        .unwrap_or_else(|| String::from("[object Object]")),
                )
            };

            add_message(
                chat_id,
                name,
                pattern_regex,
                db,
                autoreply_set_map,
                bot,
                response,
            )
            .await?;
        }
    }

    Ok(())
}

async fn add_message(
    chat_id: ChatId,
    name: &str,
    pattern_regex: Regex,
    db: DatabaseRef,
    autoreply_set_map: std::sync::Arc<
        tokio::sync::RwLock<std::collections::HashMap<ChatId, AutoreplySet>>,
    >,
    bot: &AutoSend<Bot>,
    response: AutoreplyResponse,
) -> Result<(), anyhow::Error> {
    let autoreply = Autoreply {
        chat_id,
        name: name.to_string(),
        pattern_regex,
        response,
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
    Ok(())
}

async fn parse_regex<'a>(
    args: &[std::borrow::Cow<'a, str>],
    bot: &AutoSend<Bot>,
    chat_id: ChatId,
) -> Result<Option<Regex>, anyhow::Error> {
    let pattern_regex = Regex::new(&*args[1]);
    let pattern_regex = match pattern_regex {
        Ok(regex) => regex,
        Err(err) => {
            bot.send_message(
                chat_id,
                format!("Regex-lausekkeen parsinta epäonnistui: {}", err),
            )
            .await?;

            return Ok(None);
        }
    };
    Ok(Some(pattern_regex))
}
