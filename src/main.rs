use std::{collections::HashMap, sync::{Mutex, Arc}, time::Duration};

use chrono::Utc;
use serenity::{Client, prelude::{GatewayIntents, EventHandler, Context}, async_trait, model::prelude::{command::{Command}, Ready, GuildId, Message, component::ActionRowComponent}, utils::Colour, builder::{CreateEmbed, CreateInteractionResponseData, CreateComponents}};
use serenity::model::application::interaction::{Interaction, InteractionResponseType};
use sqlx::{postgres::PgPoolOptions, Pool, Postgres};
use tokio::time::sleep;
use toml::Value;

mod commands;
mod common;

pub struct Bot {
    db: Pool<Postgres>,
    owner_ids: Vec<u64>,
    running_paginator: Arc<Mutex<HashMap<u64, Vec<String>>>>,
}

#[async_trait]
impl EventHandler for Bot {
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::ApplicationCommand(command) = interaction {
            let content = match &command.data.name.as_str() {
                &"leaderboard" => commands::run(self.clone(), &command).await,
                _ => common::default_embed_from_content(
                    &command.user.name,
                    &command.user.avatar_url().unwrap_or_default(),
                    String::from("This command doesn't exist."), Colour::RED),
            };

            match command
            .create_interaction_response(&ctx.http, |response| {
                response
                    .kind(InteractionResponseType::ChannelMessageWithSource)
                    .interaction_response_data(|message| {
                        message.clone_from(&content);
                        message
                    })
            })
            .await {
                Ok(_) => {
                    if let Ok(response) = command.get_interaction_response(&ctx.http).await {
                        let http = ctx.http.clone();
                        let paginators = Arc::new(self.running_paginator.clone());
                        tokio::spawn(async move {
                            sleep(Duration::from_secs(60)).await;
                            let Some(row) = response.components.get(0) else {return;};
                            let Some(ActionRowComponent::Button(button)) = row.components.get(0) else {return;};
                            let mut info = button.custom_id.as_ref().unwrap().split("_");
                            let Some(key_string) = info.next() else {return;};
                            let Ok(key) = key_string.parse::<u64>() else {return;};

                            if let Ok(mut data) = paginators.lock() {
                                data.remove(&key);
                            }

                            if let Err(why) = response.channel_id.edit_message(http, response.id, |m| {
                                m.set_components(CreateComponents::default())
                            }).await {
                                println!("Cannot remove components: {}", why);
                            }
                        });
                    }
                },
                Err(why) => {
                    println!("Cannot respond to slash command: {}", why);
                },
            }
            
        } else if let Interaction::MessageComponent(component) = interaction {
            let custom_id = &component.data.custom_id;
            if !custom_id.contains("lb") {
                return;
            }
            let mut message = CreateInteractionResponseData::default();

            let mut info = custom_id.split("_");
            let Some(key_string) = info.next() else {return;};
            let Ok(key) = key_string.parse::<u64>() else {return;};
            if let Ok(data) = self.running_paginator.lock() {
                let Some(pages) = data.get(&key) else {return;};
                let Some(index_string) = info.last() else {return;};
                let Ok(index) = index_string.parse::<usize>() else {return;};
                let Some(description) = pages.get(index) else {return;};
            
                let mut embed = CreateEmbed::default();
                embed.footer(|f|
                    f.text("Leaderboard Query")
                    .icon_url(component.user.avatar_url().unwrap_or_default()));
                embed.timestamp(Utc::now().to_rfc3339());
                embed.colour(Colour::from_rgb(106, 86, 246));
                embed.description(description);

                let next_button_id = format!("{}_next_lb_{}", key, index + 1);
                let prev_button_id = format!("{}_prev_lb_{}", key, index - 1);
                let max_len = pages.len();

                message.add_embed(embed);
                message.components(|component| {
                    component.create_action_row(|action_row| {
                        if index != 0 {
                            action_row.create_button(|b| b.custom_id(&prev_button_id).emoji('◀'));
                        }
                        if index != max_len {
                            action_row.create_button(|b| b.custom_id(&next_button_id).emoji('▶'));
                        }
                        action_row
                    })
                });
                            }

            if let Err(why) = component
                .create_interaction_response(&ctx.http, |response| {
                    response
                        .kind(InteractionResponseType::UpdateMessage)
                        .interaction_response_data(|m| {
                            m.clone_from(&message);
                            m
                        })
                }).await {
                    println!("Cannot respond to slash command: {}", why);
                }
        }
    }

    async fn ready(&self, _ctx: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }

    async fn message(&self, ctx: Context, msg: Message) {
        if !self.owner_ids.contains(&msg.author.id.as_u64()) 
        && !msg.content.starts_with("?"){
            return;
        }

        if msg.content.starts_with("?sync") {
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
                                "Synced commands globally."
                            )).await;
                        },
                        Err(err) => {
                            let _ = msg.channel_id.send_message(&ctx.http, |m| m.content(err.to_string())).await;
                        }
                    };
                },
                &_ => {}
            }
        } else if msg.content.starts_with("?invalidate") {
            let Some(id_string) = msg.content.strip_prefix("?invalidate ") else {return;};
            let Ok(id) = id_string.parse::<i64>() else {return;};

            match sqlx::query("
            UPDATE
                submissions
            SET
                valid = false
            WHERE
                unix_time_stamp = $1;
            ")
            .bind(id)
            .execute(&self.db)
            .await {
                Ok(_) => {
                    let _ = msg.channel_id.send_message(&ctx.http, |m| m.content(
                        format!("Invalidated submission with ID: {}", id)
                    )).await;
                },
                Err(err) => {
                    let _ = msg.channel_id.send_message(&ctx.http, |m| m.content(err.to_string())).await;
                },
            }
        }
    }
}



#[tokio::main]
async fn main() {

    let config_file = include_str!("../config.toml");
    let config = config_file.parse::<Value>().unwrap();

    let database_url = config["database_url"].as_str().unwrap();
    let pool = PgPoolOptions::new()
    .max_connections(5)
    .connect(&database_url)
    .await
    .expect("Error building a connection pool");
    
    let token = config["token"].as_str().unwrap();

    let mut client = Client::builder(token, GatewayIntents::DIRECT_MESSAGES)
        .event_handler(Bot {
            db: pool.clone(),
            owner_ids: vec![474319793042751491, 322007790208155650],
            running_paginator: Arc::new(Mutex::new(HashMap::new()))})
        .await
        .expect("Error creating client");

        if let Err(why) = client.start().await {
            println!("Client error: {:?}", why);
        }
}