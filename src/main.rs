use serenity::{Client, prelude::{GatewayIntents, EventHandler, Context}, async_trait, model::prelude::{command::{Command}, Ready, GuildId, Message}, utils::Colour};
use serenity::model::application::interaction::{Interaction, InteractionResponseType};
use sqlx::{postgres::PgPoolOptions, Pool, Postgres};

mod commands;
mod common;

pub struct Bot {
    db: Pool<Postgres>,
    owner_ids: Vec<u64>
}

#[async_trait]
impl EventHandler for Bot {
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::ApplicationCommand(command) = interaction {
            let content = match &command.data.name.as_str() {
                &"leaderboard" => commands::run(self, &command).await,
                _ => common::default_embed_from_content(&command, String::from("This command doesn't exist."), Colour::RED),
            };

            if let Err(why) = command
                .create_interaction_response(&ctx.http, |response| {
                    response
                        .kind(InteractionResponseType::ChannelMessageWithSource)
                        .interaction_response_data(|message| message.add_embed(content))
                })
                .await
            {
                println!("Cannot respond to slash command: {}", why);
            }
        }
    }

    async fn ready(&self, _ctx: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }

    async fn message(&self, ctx: Context, msg: Message) {
        if !self.owner_ids.contains(&msg.author.id.as_u64()) 
        && !msg.content.starts_with("?sync"){
            return;
        }

        let register_type = msg.content.strip_prefix("?sync ").unwrap_or("*");
        match register_type {
            "*" => {
                let guild_id = GuildId(781938561175388190);

                match GuildId::set_application_commands(&guild_id, &ctx.http, |commands| {
                    commands
                        .create_application_command(|command| commands::register(command))
                })
                .await {
                    Ok(v) => {
                        let _ = msg.channel_id.send_message(&ctx.http, |m| m.content(
                            format!("Synced {} commands to the guild.", v.len())
                        )).await;
                    },
                    Err(err) => {
                        let _ = msg.channel_id.send_message(&ctx.http, |m| m.content(err.to_string())).await;
                    }
                };
            },
            "~" => {
                match Command::create_global_application_command(&ctx.http, |command| {
                    commands::register(command)
                })
                .await {
                    Ok(_) => {
                        let _ = msg.channel_id.send_message(&ctx.http, |m| m.content(
                            format!("Synced commands globally.")
                        )).await;
                    },
                    Err(err) => {
                        let _ = msg.channel_id.send_message(&ctx.http, |m| m.content(err.to_string())).await;
                    }
                };
            },
            &_ => {}
        }
    }
}



#[tokio::main]
async fn main() {

    let database_url = "";
    let pool = PgPoolOptions::new()
    .max_connections(5)
    .connect(&database_url)
    .await
    .expect("Error building a connection pool");
    
    let token = "";

    let mut client = Client::builder(token, GatewayIntents::DIRECT_MESSAGES)
        .event_handler(Bot {db: pool.clone(), owner_ids: vec![474319793042751491, 322007790208155650]})
        .await
        .expect("Error creating client");

        if let Err(why) = client.start().await {
            println!("Client error: {:?}", why);
        }
}