use chrono::Utc;
use serenity::{model::prelude::{interaction::application_command::{CommandDataOption, CommandDataOptionValue, ApplicationCommandInteraction}, command::{CommandOptionType}}, builder::{CreateApplicationCommand, CreateEmbed}, utils::Colour};
use sqlx::{FromRow};

use crate::{Bot, common::default_embed_from_content};


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

pub async fn run(bot: &Bot, command: &ApplicationCommandInteraction) -> CreateEmbed {
    let options = &command.data.options;
    if let Some(sub_option) = options.get(0) {
        match sub_option.name.as_str() {
            "all" => player_command(bot, &command, &sub_option.options).await,
            "game" => leaderboards_command(bot, &command, &sub_option.options).await,
            _ => default_embed_from_content(&command, String::from("Not a valid sub command. What happened here?"), Colour::RED)
        }
    } else {
        default_embed_from_content(&command, String::from("Not a valid sub command. What happened here?"), Colour::RED)
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

async fn leaderboards_command(bot: &Bot, command: &ApplicationCommandInteraction, option: &[CommandDataOption]) -> CreateEmbed {
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
            return default_embed_from_content(command, String::from("Integer was not an integer?"), Colour::RED);
        }
        };
    let upper = if lower + 9 > 200 {200} else {lower + 9};

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
            AND
                position
            BETWEEN
                $2
            AND
                $3
            ORDER BY
                position
            ASC;")
        .bind(leaderboard_value_to_database_name(game))
        .bind(lower)
        .bind(upper)
        .fetch_all(&bot.db)
        .await {
            Ok(players) => players_to_formatted_embed(command, players, game, lower, upper),
            Err(err) => {
                println!("{}", err.to_string());
                default_embed_from_content(command, String::from("An error occurred trying to fetch the leaderboards. Contact Fesa if this persists"), Colour::RED)
            },
        }
    } else {
        default_embed_from_content(command, String::from("Not a valid sub command. What happened here?"), Colour::RED)
    }
}

fn players_to_formatted_embed(command: &ApplicationCommandInteraction, players: Vec<LeaderboardRow>, game_name: &String, lower: &i64, upper: i64) -> CreateEmbed {
    let mut embed = CreateEmbed::default();
    embed.footer(|f|
        f.text("Leaderboard Query")
        .icon_url(command.user.avatar_url().unwrap_or_default()));
    embed.timestamp(Utc::now().to_rfc3339());

    if players.len() == 0 {
        embed.colour(Colour::RED);
        embed.description(String::from(format!("**{}** currently doesn't have any players on it between {} and {}.", game_name.to_owned(), lower, upper)));
        return embed;
    }

    embed.colour(Colour::from_rgb(106, 86, 246));
    let mut s = String::from(format!("Players on {} between {} and {}:", leaderboard_value_to_database_name(game_name), lower, upper));
    for row in players {
        s += &row.get_player_string();
    }
    embed.description(s);

    embed
}

fn leaderboard_value_to_database_name(game: &String) -> String {
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

async fn player_command(bot: &Bot, command: &ApplicationCommandInteraction, option: &[CommandDataOption]) -> CreateEmbed {
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
                        Ok(leaderboards) => leaderboards_to_formatted_embed(command, leaderboards, player_name),
                        Err(err) => {
                            println!("{}", err.to_string());
                            default_embed_from_content(command, String::from("An error occurred trying to fetch the leaderboards. Contact Fesa if this persists"), Colour::RED)
                        }
                    }
        } else {
            default_embed_from_content(command, String::from("Not a valid sub command. What happened here?"), Colour::RED)
        }
}

fn leaderboards_to_formatted_embed(command: &ApplicationCommandInteraction, leaderboards: Vec<LeaderboardRow>, player_name: &String) -> CreateEmbed {
    let mut embed = CreateEmbed::default();
    embed.footer(|f|
        f.text("Leaderboard Query")
        .icon_url(command.user.avatar_url().unwrap_or_default()));
    embed.timestamp(Utc::now().to_rfc3339());

    if leaderboards.len() == 0 {
        embed.colour(Colour::RED);
        embed.description(String::from(format!("**{}** currently isn't on any leaderboard.", player_name.to_owned())));
        return embed;
    }

    embed.colour(Colour::from_rgb(106, 86, 246));
    let mut s = String::from(format!("**{}** leaderboards ({}):", player_name.to_owned(), &leaderboards.len()));
    for row in leaderboards {
        s += &row.get_leaderboard_string();
    }
    embed.description(s);

    embed
}
