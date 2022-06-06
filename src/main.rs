use anyhow::Context;
use teloxide::{
    dispatching::{UpdateFilterExt, UpdateHandler},
    prelude::*,
    types::Update,
    utils::command::BotCommands,
};

mod handlers;

#[derive(BotCommands, Clone)]
#[command(rename = "lowercase", description = "Tuetut komennot:")]
enum Command {
    #[command(description = "Miksi härveli ei toimi?")]
    GetExcuse,

    #[command(description = "Moro äijät mitäs äijät :D")]
    DudeCarpet,

    #[command(description = "Hiljaisuus päättyy")]
    BreakSilence,
}

async fn handle_command(
    bot: AutoSend<Bot>,
    message: Message,
    command: Command,
) -> anyhow::Result<()> {
    let result = match command {
        Command::GetExcuse => handlers::handle_get_excuse(&bot, &message)
            .await
            .context("handle_get_excuse"),
        Command::DudeCarpet => handlers::handle_dude_carpet(&bot, &message)
            .await
            .context("handle_dude_carpet"),
        Command::BreakSilence => Ok(()),
    };

    match result {
        Ok(()) => Ok(()),
        Err(err) => {
            log::error!("{}", err);
            bot.send_message(message.chat.id, "Jotain meni pieleen :(")
                .await?;
            Ok(())
        }
    }
}

async fn handle_message(bot: AutoSend<Bot>, message: Message) -> anyhow::Result<()> {
    let text = message.text().unwrap_or_default();

    if text.contains("kalja") {
        bot.send_message(message.chat.id, "oispa kaljaa").await?;
    }

    Ok(())
}

fn schema() -> UpdateHandler<anyhow::Error> {
    Update::filter_message()
        .branch(teloxide::filter_command::<Command, _>().chain(dptree::endpoint(handle_command)))
        .branch(dptree::endpoint(handle_message))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();
    pretty_env_logger::init();

    log::info!("Starting haloobot2...");

    let token =
        std::env::var("TELEGRAM_TOKEN").context("TELEGRAM_TOKEN not found in environment")?;

    let bot = Bot::new(token).auto_send();

    // https://github.com/teloxide/teloxide/blob/86657f55ffa1f10baa18a6fdca2c72c30db33519/src/dispatching/repls/commands_repl.rs#L82
    let ignore_update = |_upd| Box::pin(async {});

    Dispatcher::builder(bot, schema())
        .default_handler(ignore_update)
        .build()
        .setup_ctrlc_handler()
        .dispatch()
        .await;

    Ok(())
}
