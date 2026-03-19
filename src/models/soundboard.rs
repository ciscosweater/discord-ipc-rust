use serde::{Deserialize, Serialize};

/// <https://discord.com/developers/docs/topics/rpc#get_soundboard_sounds-get-soundboard-sounds-argument-structure>
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GetSoundboardSoundsArgs {
    /// string - id of the guild to get soundboard sounds for
    pub guild_id: String,
}

/// <https://discord.com/developers/docs/topics/rpc#soundboard-sounds-soundboard-sounds-event-structure>
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SoundboardSound {
    /// snowflake - id of the sound
    pub sound_id: String,
    /// snowflake - id of the guild the sound is from
    pub guild_id: String,
    /// string - name of the sound
    pub name: String,
    /// number - volume of the sound (0.0 to 1.0)
    pub volume: f64,
    /// snowflake - id of the emoji attached to the sound
    pub emoji_id: Option<String>,
    /// string - name of the emoji attached to the sound
    pub emoji_name: Option<String>,
    /// boolean - whether the sound is available
    pub available: bool,
    /// partial user object - user who created the sound
    pub user: Option<SoundboardSoundUser>,
}

/// Partial user object for soundboard sound creator
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SoundboardSoundUser {
    /// snowflake - id of the user
    pub id: String,
    /// string - username of the user
    pub username: String,
    /// string - discriminator of the user
    pub discriminator: String,
    /// string - global display name of the user
    pub global_name: Option<String>,
}
