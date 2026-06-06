use gloo_net::http::Request;
use serde::{Deserialize, Deserializer, Serialize};
use std::any::Any;
use std::collections::HashMap;
use std::fmt;

fn null_as_zero<'de, D>(deserializer: D) -> Result<i32, D::Error>
where
    D: Deserializer<'de>,
{
    // Option<i32> will deserialize null as None, or a number as Some(value)
    let opt = Option::<i32>::deserialize(deserializer)?;
    Ok(opt.unwrap_or_default())
}

#[derive(Deserialize, Debug, PartialEq, Clone, Serialize, Default)]
#[serde(default)]
#[allow(non_snake_case)]
pub struct Episode {
    pub podcastid: i32,
    #[serde(alias = "feedTitle")]
    pub podcastname: String,
    #[serde(alias = "Episodetitle")]
    #[serde(alias = "title")]
    pub episodetitle: String,
    //pub description: String,
    #[serde(alias = "feedImage")]
    pub artworkurl: String,
    #[serde(alias = "feedAuthor")]
    pub author: String,
    pub categories: Option<HashMap<String, String>>,
    #[serde(alias = "Episodedescription")]
    #[serde(alias = "description")]
    pub episodedescription: String,
    pub episodecount: Option<i32>,
    #[serde(alias = "feedUrl")]
    pub feedurl: String,
    #[serde(alias = "link")]
    pub websiteurl: String,
    pub explicit: i32,
    pub userid: i32,
    #[serde(alias = "Episodeid")]
    pub episodeid: i32,
    #[serde(alias = "Episodeurl")]
    #[serde(alias = "enclosure_url")]
    #[serde(alias = "enclosureUrl")]
    pub episodeurl: String,
    #[serde(alias = "Episodeartwork")]
    #[serde(alias = "artwork")]
    #[serde(alias = "image")]
    pub episodeartwork: String,
    #[serde(alias = "Episodepubdate")]
    #[serde(alias = "pub_date")]
    pub episodepubdate: String,
    #[serde(alias = "Episodeduration")]
    #[serde(alias = "duration")]
    pub episodeduration: i32,
    #[serde(alias = "Listenduration", deserialize_with = "null_as_zero")]
    pub listenduration: i32,
    #[serde(alias = "Completed")]
    pub completed: bool,
    #[serde(alias = "is_saved")]
    pub saved: bool,
    #[serde(alias = "is_queued")]
    pub queued: bool,
    #[serde(alias = "is_downloaded")]
    pub downloaded: bool,
    pub is_youtube: bool,
    pub is_video: bool,
    pub guid: String,
    pub queueposition: Option<i32>,
    pub downloadedlocation: Option<String>,
    pub listendate: Option<String>,
    pub savedate: Option<String>,
}

impl Episode {
    pub fn as_any(&self) -> &dyn Any {
        self
    }
}
