use regex::Regex;
use teloxide::types::{ChatId, Message};

use crate::{
    argument_parser::parse_arguments,
    autoreplies::{Autoreply, AutoreplyResponse, AutoreplySet, AutoreplySetMap},
    command_handler::{fail, succeed, succeed_with_message, HandlerError, HandlerResult},
    db::DatabaseRef,
};

pub async fn handle_add_message(
    chat_id: ChatId,
    db: DatabaseRef,
    autoreply_set_map: AutoreplySetMap,
    args: &str,
) -> HandlerResult {
    let args = parse_arguments(args);

    match args {
        Err(err) => {
            return fail(format!("Parametrien parsinta epäonnistui: {}", err));
        }
        Ok((_, args)) => {
            if args.len() == 2 {
                return succeed_with_message(
                    "👀 Lähetä haluttu viesti tai tarra vastauksena alkuperäiseen komentoon.",
                );
            }

            if args.len() != 3 {
                return fail(
                    "Parametrien määrä väärin. Käytä muotoa: /addmessage <nimi> <regex> <viesti>",
                );
            }

            let name = &args[0];
            let pattern_regex = parse_regex(&args)?;

            let pattern_regex = match pattern_regex {
                Some(regex) => regex,
                None => {
                    return succeed();
                }
            };

            let response = &args[2];

            add_message(
                chat_id,
                name,
                pattern_regex,
                db,
                autoreply_set_map,
                AutoreplyResponse::Literal(response.to_string()),
            )
            .await?;
        }
    }

    succeed()
}

pub async fn handle_add_message_reply(
    db: DatabaseRef,
    autoreply_set_map: AutoreplySetMap,
    message: &Message,
    previous_args: &str,
) -> HandlerResult {
    let previous_args = parse_arguments(previous_args);
    let chat_id = message.chat.id;

    match previous_args {
        Err(err) => fail(format!(
            "Komennon parametrien parsinta epäonnistui: {}",
            err
        )),
        Ok((_, args)) => {
            if args.len() != 2 {
                return fail("Parametrien määrä väärin.");
            }

            let name = &args[0];
            let pattern_regex = parse_regex(&args)?;

            let pattern_regex = match pattern_regex {
                Some(regex) => regex,
                None => {
                    return succeed();
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
                response,
            )
            .await
        }
    }
}

async fn add_message(
    chat_id: ChatId,
    name: &str,
    pattern_regex: Regex,
    db: DatabaseRef,
    autoreply_set_map: std::sync::Arc<
        tokio::sync::RwLock<std::collections::HashMap<ChatId, AutoreplySet>>,
    >,
    response: AutoreplyResponse,
) -> HandlerResult {
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

    succeed_with_message(format!("🎉 Lisätty automaattinen vastaus {}", name))
}

fn parse_regex<'a>(args: &[std::borrow::Cow<'a, str>]) -> Result<Option<Regex>, HandlerError> {
    let pattern_regex = Regex::new(&*args[1]);
    let pattern_regex = match pattern_regex {
        Ok(regex) => regex,
        Err(err) => {
            return fail(format!("Regex-lausekkeen parsinta epäonnistui: {}", err));
        }
    };
    Ok(Some(pattern_regex))
}
