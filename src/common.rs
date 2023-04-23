use chrono::Utc;
use serenity::{model::prelude::interaction::application_command::ApplicationCommandInteraction, builder::CreateEmbed, utils::Colour};

pub fn default_embed_from_content(command: &ApplicationCommandInteraction, content: String, colour: Colour) -> CreateEmbed {
    let mut embed = CreateEmbed::default();

    embed.author(|a| 
        a
        .name(&command.user.name)
        .icon_url(command.user.avatar_url().unwrap_or_default()));
    embed.colour(colour);
    embed.description(content);
    embed.timestamp(Utc::now().to_rfc3339());
    embed
}