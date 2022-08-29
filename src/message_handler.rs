use std::{io::Cursor, sync::Arc};

use anyhow::Context;
use image::ImageOutputFormat;
use teloxide::{
    net::Download, payloads::SendPhoto, prelude::*, requests::MultipartRequest, types::InputFile,
    utils::command::BotCommands,
};

use crate::{
    autoreplies::{AutoreplyResponse, AutoreplySetMap, StickerCache},
    chat_config::ChatConfigModel,
    db::DatabaseRef,
    handlers, Command,
};

pub async fn handle_message(
    bot: AutoSend<Bot>,
    db: DatabaseRef,
    message: Message,
    autoreply_set_map: AutoreplySetMap,
    chat_config_map: Arc<ChatConfigModel>,
    sticker_cache: Arc<StickerCache>,
) -> anyhow::Result<()> {
    let chat_id = message.chat.id;
    let mut is_reply_to_me = false;

    if let Some(original_message) = message.reply_to_message() {
        let me = bot
            .get_me()
            .await
            .context("Expected get_me to never fail")?;

        if let Some(from) = original_message.from() {
            if from.id == me.id {
                is_reply_to_me = true;
            }
        }

        let username = me.username.as_deref().unwrap_or_default();
        let original_as_cmd = Command::parse(original_message.text().unwrap_or_default(), username);

        match original_as_cmd {
            // If the message was a response to a command BUT both messages were not sent by the same user,
            // then we don't want to handle it.
            Ok(_) if message.from() != original_message.from() => {
                bot.send_message(chat_id, "😳").await?;
                return Ok(());
            }
            Ok(Command::AddMessage(args)) => {
                handlers::handle_add_message_reply(&bot, db, autoreply_set_map, &message, &args)
                    .await?;
                return Ok(());
            }
            _ => {}
        }
    }

    let chat_config = chat_config_map.get(chat_id).await?;

    if let Some(sticker) = message.sticker() {
        if sticker.set_name.is_none() {
            // If this is a sticker set without a set name, it is acccshually a WebP image sent as a sticker
            let sticker_file = bot
                .get_file(&sticker.file_id)
                .await
                .context("Failed to get sticker")?;
            let mut sticker_buffer = Vec::new();
            bot.download_file(&sticker_file.file_path, &mut sticker_buffer)
                .await?;

            let image_png = re_encode_image(sticker_buffer, ImageOutputFormat::Jpeg(95))?;

            let mut payload = SendPhoto::new(chat_id, InputFile::memory(image_png));
            payload.reply_to_message_id = Some(message.id);
            payload.caption = Some(String::from("Hieno sticker veliseni"));
            MultipartRequest::new(bot.inner().clone(), payload)
                .send()
                .await
                .context("Failed to send photo response")?;
        } else {
            // Only stickers with emoji (are there any without ??) are eligible for autoreply
            if let Some(emoji) = &sticker.emoji {
                let response_sticker = sticker_cache
                    .update_and_get_response_sticker(chat_id, emoji, sticker)
                    .await
                    .context("update_and_get_response_sticker")?;

                match response_sticker {
                    None => {}
                    Some(reply_sticker) => {
                        let p: f64 = rand::random();

                        if p < chat_config.autoreply_chance {
                            bot.send_sticker(chat_id, InputFile::file_id(reply_sticker.file_id))
                                .await
                                .context("Failed to send response sticker")?;
                        }
                    }
                }
            }
        }

        return Ok(());
    }

    let text = message.text().unwrap_or_default();

    let autoreply_set_map = autoreply_set_map.read().await;
    let autoreply_set = autoreply_set_map.get(&chat_id);

    let autoreply_set = match autoreply_set {
        None => {
            return Ok(());
        }
        Some(autoreply_set) => autoreply_set,
    };

    let mut reply_message = String::new();
    let autoreply_chance = if is_reply_to_me {
        1.0
    } else {
        chat_config.autoreply_chance
    };

    for reply in autoreply_set.get_matches(text) {
        let p: f64 = rand::random();

        if p > autoreply_chance {
            continue;
        }

        match &reply.response {
            AutoreplyResponse::Literal(text) => {
                if !reply_message.is_empty() {
                    reply_message.push(' ');
                }
                reply_message.push_str(text);
            }
            AutoreplyResponse::Sticker(sticker_id) => {
                bot.send_sticker(chat_id, InputFile::file_id(sticker_id))
                    .await?;
                return Ok(());
            }
        }
    }

    if !reply_message.is_empty() {
        bot.send_message(chat_id, reply_message).await?;
    }

    Ok(())
}

fn re_encode_image(sticker_buffer: Vec<u8>, format: ImageOutputFormat) -> anyhow::Result<Vec<u8>> {
    let image =
        image::load_from_memory(&sticker_buffer).context("Failed to load sticker image :(")?;
    let mut image_jpg = Vec::new();
    let mut image_jpg_cursor = Cursor::new(&mut image_jpg);
    image.write_to(&mut image_jpg_cursor, format)?;
    Ok(image_jpg)
}
