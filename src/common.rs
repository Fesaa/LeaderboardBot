use chrono::Utc;
use serenity::{builder::{CreateEmbed, CreateInteractionResponseData}, utils::Colour};

pub fn default_embed_from_content(username: &String, avatar_url: &String, content: String, colour: Colour) -> CreateInteractionResponseData<'static> {
    let mut embed = CreateEmbed::default();

    embed.author(|a| 
        a
        .name(username)
        .icon_url(avatar_url));
    embed.colour(colour);
    embed.description(content);
    embed.timestamp(Utc::now().to_rfc3339());

    let mut message = CreateInteractionResponseData::default();
    message.add_embed(embed);
    message
}