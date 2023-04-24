use std::vec;

use chrono::Utc;
use serenity::{model::prelude::{interaction::application_command::{CommandDataOption, CommandDataOptionValue, ApplicationCommandInteraction}, command::{CommandOptionType}}, builder::{CreateApplicationCommand, CreateEmbed, CreateInteractionResponseData}, utils::Colour};
use sqlx::{FromRow};

use crate::{Bot, common::{default_embed_from_content}};


#[derive(FromRow)]
pub struct LeaderboardRow {
    pub player: String,
    pub position: i32,
    pub score: i32,
    pub game: String
}

impl LeaderboardRow {

    pub fn get_leaderboard_string(&self) -> String {
        format!("\n- {} [{}]: {} {}", self.game, self.position, self.score, self.game_to_score_kind())
    }

    pub fn get_player_string(&self) -> String {
        format!("\n- {} [{}]: {} {}", self.player.replace("_", "\\_"), self.position, self.score, self.game_to_score_kind())
    }

    fn game_to_score_kind(&self) -> String {
        match self.game.as_str() {
            "Team EggWars" | "Solo SkyWars" | "Team EggWars Season 2" | "Lucky Islands" => String::from("wins"),
            "Free For All" => String::from("kills"),
            "Parkour" => String::from("medals"),
            &_ => String::from("unknown")
        }
    }

}

pub async fn run<'a>(bot: &Bot, command: &'a ApplicationCommandInteraction) -> CreateInteractionResponseData<'a> {
    let options = &command.data.options;
    if let Some(sub_option) = options.get(0) {
        match sub_option.name.as_str() {
            "all" => player_command(bot, &command, &sub_option.options).await,
            "game" => leaderboards_command(bot, &command, &sub_option.options).await,
            _ => default_embed_from_content(
                &command.user.name,
                &command.user.avatar_url().unwrap_or_default(),
                String::from("Not a valid sub command. What happened here?"),
                Colour::RED)
        }
    } else {
        default_embed_from_content(
            &command.user.name,
            &command.user.avatar_url().unwrap_or_default(),
            String::from("Not a valid sub command. What happened here?"),
            Colour::RED)
    }
    }

pub fn register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    command
        .name("leaderboard")
        .description("CubeCraft's leaderboard info")
        .create_option(|option| {
            option
                .name("all")
                .description("Get all leaderboards of a player")
                .kind(CommandOptionType::SubCommand)
                .create_sub_option(|suboption| {
                    suboption
                        .name("player")
                        .description("The player")
                        .kind(CommandOptionType::String)
                        .required(true)
                        .min_length(2)
                        .max_length(16)
                })
            })
        .create_option(|option| {
            option
                .name("game")
                .description("Get players on a game's leaderboard")
                .kind(CommandOptionType::SubCommand)
                .create_sub_option(|suboption| {
                    suboption
                        .name("game")
                        .description("The game")
                        .kind(CommandOptionType::String)
                        .add_string_choice("Team EggWars", "tew")
                        .add_string_choice("Team EggWars Season 2", "tew2")
                        .add_string_choice("Solo SkyWars", "s_sw")
                        .add_string_choice("Solo Lucky Islands", "s_li")
                        .add_string_choice("Free For All", "ffa")
                        .add_string_choice("Parkour", "parkour")
                        .required(true)
                })
                .create_sub_option(|suboption| {
                    suboption
                        .name("place")
                        .description("From place, and 10 more")
                        .kind(CommandOptionType::Integer)
                        .min_int_value(1)
                        .max_int_value(200)
                        .required(false)
                })
            })
}

async fn leaderboards_command<'a>(bot: &Bot, command: &'a ApplicationCommandInteraction, option: &'a [CommandDataOption]) -> CreateInteractionResponseData<'a> {
    let game = option
        .get(0)
        .expect("There should be a game")
        .resolved
        .as_ref()
        .expect("Expected String");

    let lower =if let None = option.get(1) {&1} else {
        if let Some(CommandDataOptionValue::Integer(i)) = option.get(1).unwrap().resolved.as_ref() {
            i
        } else {
            return default_embed_from_content(
                &command.user.name,
                &command.user.avatar_url().unwrap_or_default(), 
                String::from("Integer was not an integer?"),
                Colour::RED);
        }
        };
    let upper = 200;

    if let CommandDataOptionValue::String(game) = game {
        match sqlx::query_as::<_, LeaderboardRow>("
            SELECT 
                player,position,score,game
            FROM
                leaderboards
            WHERE
                game = $1
            AND
                unix_time_stamp
            = (SELECT
                    MAX(unix_time_stamp)
                FROM
                    submissions
                WHERE
                    valid = TRUE
                AND
                    game = $1)
            ORDER BY
                position
            ASC;")
        .bind(leaderboard_value_to_database_name(game.to_owned()))
        .bind(lower)
        .bind(upper)
        .fetch_all(&bot.db)
        .await {
            Ok(players) => players_to_formatted_embed(
                bot,
                command.user.avatar_url().unwrap_or_default(),
                players, 
                game.to_owned(),
                lower.to_owned(),
                upper),
            Err(err) => {
                println!("{}", err.to_string());
                default_embed_from_content(
                    &command.user.name,
                    &command.user.avatar_url().unwrap_or_default(),
                    String::from("An error occurred trying to fetch the leaderboards. Contact Fesa if this persists"),
                    Colour::RED)
            },
        }
    } else {
        default_embed_from_content(
            &command.user.name,
            &command.user.avatar_url().unwrap_or_default(),
            String::from("Not a valid sub command. What happened here?"),
        Colour::RED)
    }
}

fn players_to_formatted_embed(bot: & Bot, avatar_url: String, players: Vec<LeaderboardRow>, game_name: String, lower: i64, upper: i64) -> CreateInteractionResponseData<'static> {
    let mut embed = CreateEmbed::default();
    embed.footer(|f|
        f.text("Leaderboard Query")
        .icon_url(avatar_url));

    let now = Utc::now();
    embed.timestamp(now.to_rfc3339());

    let mut message = CreateInteractionResponseData::default();

    if players.len() == 0 {
        embed.colour(Colour::RED);
        embed.description(String::from(format!("**{}** currently doesn't have any players on it between {} and {}.", game_name, lower, upper)));
    } else {
        embed.colour(Colour::from_rgb(106, 86, 246));

        let mut pages: Vec<String> = vec![];
        let pretty_name = leaderboard_value_to_database_name(game_name);

        for low in (lower..200).step_by(10) {
            if low > 200 {
                break;
            }
            let up = if low + 9 < 200 {low + 9} else {200};
            let mut s = String::from(format!("Players on {} between {} and {}:", pretty_name, low, up));
            for row in &players[((low-1) as usize)..(up as usize)] {
                s += &row.get_player_string();
            }
            pages.push(s);
        }
        embed.description(&pages[0]);
        if let Ok(mut data) = bot.running_paginator.lock() {
            let next_button_id = format!("{}_next_lb_{}", now.timestamp_millis(), 1);

            message.components(|component| {
                component.create_action_row(|action_row| {
                    action_row.create_button(|b| b.custom_id(&next_button_id).emoji('▶'))
                })
            });

            data.insert(now.timestamp_millis() as u64, pages);
        }
    }
    message.add_embed(embed);
    message
}

fn leaderboard_value_to_database_name(game: String) -> String {
    match game.as_str() {
        "tew"  => String::from("Team EggWars"),
        "tew2" => String::from("Team EggWars Season 2"),
        "s_sw" => String::from("Solo SkyWars"),
        "s_li" => String::from("Lucky Islands"),
        "ffa" => String::from("Free For All"),
        "parkour" => String::from("Parkour"),

        &_ => String::from("Unknown")
    }
}

async fn player_command<'a>(bot: &Bot, command: &'a ApplicationCommandInteraction, option: &'a [CommandDataOption]) -> CreateInteractionResponseData<'a> {
    let player = option
        .get(0)
        .expect("There should be a player")
        .resolved
        .as_ref()
        .expect("Expected String");

        if let CommandDataOptionValue::String(player_name) = player {
            match sqlx::query_as::<_, LeaderboardRow>("
                SELECT 
                    player,position,score,game
                FROM
                    leaderboards
                WHERE
                    (game, unix_time_stamp)
                IN (SELECT
                        game, MAX(unix_time_stamp)
                    FROM
                        submissions
                    WHERE
                        valid = TRUE
                    GROUP BY
                        game)
                AND
                    player = $1
                ORDER BY
                    position
                ASC;")
                    .bind(player_name)
                    .fetch_all(&bot.db)
                    .await {
                        Ok(leaderboards) => leaderboards_to_formatted_embed(
                            command.user.avatar_url().unwrap_or_default().to_owned(),
                            leaderboards,
                            player_name.to_owned()),
                        Err(err) => {
                            println!("{}", err.to_string());
                            default_embed_from_content(
                                &command.user.name,
                                &command.user.avatar_url().unwrap_or_default(),
                                String::from("An error occurred trying to fetch the leaderboards. Contact Fesa if this persists"),
                                Colour::RED)
                        }
                    }
        } else {
            default_embed_from_content(&command.user.name,
                &command.user.avatar_url().unwrap_or_default(),
                String::from("Not a valid sub command. What happened here?"), 
                Colour::RED)
        }
}

fn leaderboards_to_formatted_embed(avatar_url: String, leaderboards: Vec<LeaderboardRow>, player_name: String) -> CreateInteractionResponseData<'static> {
    let mut embed = CreateEmbed::default();
    embed.footer(|f|
        f.text("Leaderboard Query")
        .icon_url(avatar_url));
    embed.timestamp(Utc::now().to_rfc3339());

    if leaderboards.len() == 0 {
        embed.colour(Colour::RED);
        embed.description(String::from(format!("**{}** currently isn't on any leaderboard.", player_name)));
    } else {
        embed.colour(Colour::from_rgb(106, 86, 246));
        let mut s = String::from(format!("**{}** leaderboards ({}):", player_name, &leaderboards.len()));
        for row in leaderboards {
            s += &row.get_leaderboard_string();
        }
        embed.description(s);
    }

    let mut message = CreateInteractionResponseData::default();
    message.add_embed(embed);
    message
}
