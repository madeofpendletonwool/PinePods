//! Podcasting 2.0 tags, read once per feed at refresh time and stored, so podcast and episode
//! pages never fetch a feed to show hosts, guests, podroll, funding, value, chapters or
//! transcripts.
//!
//! One streaming pass with quick-xml: tag order, line layout, namespace prefix and escaping don't
//! matter, and a malformed tail still yields everything before it.

use quick_xml::events::{BytesStart, Event};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FeedPerson {
    pub name: String,
    pub role: Option<String>,
    pub group: Option<String>,
    pub img: Option<String>,
    pub href: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Transcript {
    pub url: String,
    pub mime_type: Option<String>,
    pub language: Option<String>,
    pub rel: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ItemTags {
    pub guid: Option<String>,
    pub enclosure_url: Option<String>,
    pub people: Vec<FeedPerson>,
    pub chapters_url: Option<String>,
    pub transcripts: Vec<Transcript>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PodrollItem {
    pub feed_guid: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Funding {
    pub url: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ValueRecipient {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub address: String,
    pub split: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ValueBlock {
    #[serde(rename = "type")]
    pub kind: String,
    pub method: String,
    pub suggested: Option<String>,
    pub recipients: Vec<ValueRecipient>,
}

/// Channel-level data stored as JSON on the podcast row (`Podcasts.Podcasting2Data`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChannelExtras {
    pub podroll: Vec<PodrollItem>,
    pub funding: Vec<Funding>,
    pub value: Vec<ValueBlock>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct FeedTags {
    pub podcast_guid: Option<String>,
    pub people: Vec<FeedPerson>,
    pub extras: ChannelExtras,
    /// Only items carrying at least one tag we store.
    pub items: Vec<ItemTags>,
}

impl FeedTags {
    /// Stable fingerprint, stored with the podcast so an unchanged feed costs no writes.
    pub fn fingerprint(&self) -> String {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.hash(&mut h);
        format!("{:016x}", h.finish())
    }

    pub fn has_people(&self) -> bool {
        !self.people.is_empty() || self.items.iter().any(|i| !i.people.is_empty())
    }
}

fn attr(e: &BytesStart, key: &str) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.as_ref() == key)
        .and_then(|a| a.normalized_value(quick_xml::XmlVersion::Implicit1_0).ok().map(|v| v.trim().to_string()))
        .filter(|v| !v.is_empty())
}

/// `podcast:<local>` under any namespace prefix.
fn is_p20(e: &BytesStart, local: &str) -> bool {
    let q = e.name();
    q.prefix().is_some() && q.local_name().as_ref() == local
}

fn http_url(raw: Option<String>) -> Option<String> {
    let raw = raw?;
    let lower = raw.to_ascii_lowercase();
    (lower.starts_with("http://") || lower.starts_with("https://")).then_some(raw)
}

fn clean_name(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(200).collect()
}

pub fn parse(feed: &str) -> FeedTags {
    let mut reader = quick_xml::Reader::from_str(feed);
    let mut tags = FeedTags::default();
    let mut item: Option<ItemTags> = None;
    // Text of the element currently being captured (person name, guid, funding label).
    let mut text = String::new();
    let mut capturing = false;
    let mut open_person: Option<BytesStart<'static>> = None;
    let mut open_funding: Option<String> = None;
    let mut in_guid = false;
    let mut in_podroll = false;
    let mut open_value: Option<ValueBlock> = None;

    loop {
        let event = match reader.read_event() {
            Ok(Event::Eof) => break,
            Ok(ev) => ev,
            Err(e) => {
                tracing::debug!("Podcasting 2.0 parse stopped early: {e}");
                break;
            }
        };
        match event {
            Event::Start(e) => {
                let name = e.name();
                if name.as_ref() == "item" {
                    item = Some(ItemTags::default());
                } else if name.as_ref() == "guid" && item.is_some() {
                    in_guid = true;
                    capturing = true;
                    text.clear();
                } else if is_p20(&e, "person") {
                    open_person = Some(e.into_owned());
                    capturing = true;
                    text.clear();
                } else if is_p20(&e, "funding") && item.is_none() {
                    open_funding = http_url(attr(&e, "url"));
                    capturing = true;
                    text.clear();
                } else if is_p20(&e, "guid") && item.is_none() {
                    capturing = true;
                    text.clear();
                } else if is_p20(&e, "podroll") && item.is_none() {
                    in_podroll = true;
                } else if is_p20(&e, "value") && item.is_none() {
                    open_value = Some(value_block(&e));
                } else {
                    handle_empty(&e, &mut item, &mut tags, in_podroll, &mut open_value);
                }
            }
            Event::Empty(e) => {
                if is_p20(&e, "value") && item.is_none() {
                    // A value block with no recipients isn't useful; skip it.
                } else if is_p20(&e, "funding") && item.is_none() {
                    if let Some(url) = http_url(attr(&e, "url")) {
                        tags.extras.funding.push(Funding { url, description: String::new() });
                    }
                } else {
                    handle_empty(&e, &mut item, &mut tags, in_podroll, &mut open_value);
                }
            }
            Event::Text(t) if capturing => text.push_str(&t.xml10_content()),
            Event::GeneralRef(r) if capturing => {
                text.push('&');
                text.push_str(&r.xml10_content());
                text.push(';');
            }
            Event::CData(c) if capturing => text.push_str(&quick_xml::escape::escape(c.into_inner().into_owned())),
            Event::End(e) => {
                let name = e.name();
                let take = |text: &mut String| {
                    let raw = std::mem::take(text);
                    let v = quick_xml::escape::unescape(&raw).map(|c| c.into_owned()).unwrap_or(raw);
                    v.trim().to_string()
                };
                if name.as_ref() == "item" {
                    if let Some(it) = item.take() {
                        if !it.people.is_empty() || it.chapters_url.is_some() || !it.transcripts.is_empty() {
                            tags.items.push(it);
                        }
                    }
                } else if in_guid && name.as_ref() == "guid" {
                    in_guid = false;
                    capturing = false;
                    let v = take(&mut text);
                    if let Some(it) = item.as_mut() {
                        it.guid = (!v.is_empty()).then_some(v);
                    }
                } else if name.local_name().as_ref() == "person" && open_person.is_some() {
                    capturing = false;
                    let start = open_person.take().unwrap();
                    let n = clean_name(&take(&mut text));
                    if !n.is_empty() {
                        let p = FeedPerson {
                            name: n,
                            role: attr(&start, "role"),
                            group: attr(&start, "group"),
                            img: http_url(attr(&start, "img")),
                            href: http_url(attr(&start, "href")),
                        };
                        let list = match item.as_mut() {
                            Some(it) => &mut it.people,
                            None => &mut tags.people,
                        };
                        if !list.contains(&p) {
                            list.push(p);
                        }
                    }
                } else if name.local_name().as_ref() == "funding" && item.is_none() && capturing {
                    capturing = false;
                    let description = take(&mut text);
                    if let Some(url) = open_funding.take() {
                        tags.extras.funding.push(Funding { url, description });
                    }
                } else if name.local_name().as_ref() == "guid" && name.prefix().is_some() && item.is_none() && capturing {
                    capturing = false;
                    let v = take(&mut text);
                    tags.podcast_guid = (!v.is_empty()).then_some(v);
                } else if name.local_name().as_ref() == "podroll" {
                    in_podroll = false;
                } else if name.local_name().as_ref() == "value" && item.is_none() {
                    if let Some(block) = open_value.take() {
                        if !block.recipients.is_empty() {
                            tags.extras.value.push(block);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    tags
}

fn value_block(e: &BytesStart) -> ValueBlock {
    ValueBlock {
        kind: attr(e, "type").unwrap_or_default(),
        method: attr(e, "method").unwrap_or_default(),
        suggested: attr(e, "suggested"),
        recipients: Vec::new(),
    }
}

/// Self-closing (or childless) elements we read attributes from.
fn handle_empty(e: &BytesStart, item: &mut Option<ItemTags>, tags: &mut FeedTags, in_podroll: bool, open_value: &mut Option<ValueBlock>) {
    if let Some(it) = item.as_mut() {
        if e.name().as_ref() == "enclosure" {
            if it.enclosure_url.is_none() {
                it.enclosure_url = attr(e, "url");
            }
        } else if is_p20(e, "chapters") {
            if it.chapters_url.is_none() {
                it.chapters_url = http_url(attr(e, "url"));
            }
        } else if is_p20(e, "transcript") {
            if let Some(url) = http_url(attr(e, "url")) {
                it.transcripts.push(Transcript { url, mime_type: attr(e, "type"), language: attr(e, "language"), rel: attr(e, "rel") });
            }
        }
        return;
    }
    if in_podroll && is_p20(e, "remoteItem") {
        if let Some(feed_guid) = attr(e, "feedGuid") {
            tags.extras.podroll.push(PodrollItem { feed_guid });
        }
    } else if is_p20(e, "valueRecipient") {
        if let Some(block) = open_value.as_mut() {
            block.recipients.push(ValueRecipient {
                name: attr(e, "name").unwrap_or_default(),
                kind: attr(e, "type").unwrap_or_default(),
                address: attr(e, "address").unwrap_or_default(),
                split: attr(e, "split").unwrap_or_default(),
            });
        }
    }
}

/// Name key for matching people: accent-folded, lowercased, punctuation dropped, whitespace
/// collapsed. Must stay identical to `normalize_name` in database_functions/migration_definitions.py
/// (which backfills existing rows) and to PodPeopleDB's.
pub fn normalize_name(name: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    let folded: String = name
        .nfkd()
        .filter(|c| !unicode_normalization::char::is_combining_mark(*c))
        .flat_map(|c| c.to_lowercase())
        .map(|c| match c {
            'ß' => 's',
            'ø' => 'o',
            'æ' => 'a',
            'đ' | 'ð' => 'd',
            'ł' => 'l',
            other => other,
        })
        .collect();
    let mut out = String::with_capacity(folded.len());
    for c in folded.chars() {
        if c.is_alphanumeric() {
            out.push(c);
        } else if c.is_whitespace() || c == '-' || c == '_' {
            out.push(' ');
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Lowercased Podcasting 2.0 role; missing means "host" per the spec.
pub fn normalize_role(role: Option<&str>) -> String {
    let r = role.unwrap_or("").trim().to_lowercase();
    let r = r.split_whitespace().collect::<Vec<_>>().join(" ");
    match r.as_str() {
        "" | "hosts" => "host".into(),
        "cohost" | "co host" => "co-host".into(),
        "guests" => "guest".into(),
        _ => r.chars().take(60).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FEED: &str = r#"<rss xmlns:podcast="https://podcastindex.org/namespace/1.0"><channel>
<title>T</title>
<podcast:guid>abc-123</podcast:guid>
<podcast:person role="host" img="https://x/a.jpg">Alice &amp; Co</podcast:person>
<podcast:person
   role="Guest">Bob
   Builder</podcast:person>
<podcast:person href="javascript:alert(1)">Mallory</podcast:person>
<podcast:funding url="https://fund.example">Support us</podcast:funding>
<podcast:podroll><podcast:remoteItem feedGuid="g-1"/><podcast:remoteItem feedGuid="g-2"/></podcast:podroll>
<podcast:value type="lightning" method="keysend" suggested="0.00000005000">
  <podcast:valueRecipient name="Alice" type="node" address="03ab" split="90"/>
  <podcast:valueRecipient name="App" type="node" address="02cd" split="10"/>
</podcast:value>
<item><title>E2</title><guid isPermaLink="false">e-2</guid><enclosure url="https://cdn/e2.mp3?a=1&amp;b=2"/>
  <podcast:person role="guest">Carol</podcast:person>
  <podcast:chapters url="https://cdn/e2.json" type="application/json+chapters"/>
  <podcast:transcript url="https://cdn/e2.srt" type="application/srt" language="en"/>
</item>
<item><title>E1</title><guid>e-1</guid><enclosure url="https://cdn/e1.mp3"/></item>
</channel></rss>"#;

    #[test]
    fn parses_everything() {
        let t = parse(FEED);
        assert_eq!(t.podcast_guid.as_deref(), Some("abc-123"));
        assert_eq!(t.people.len(), 3);
        assert_eq!(t.people[0].name, "Alice & Co");
        assert_eq!(t.people[0].img.as_deref(), Some("https://x/a.jpg"));
        assert_eq!(t.people[1].name, "Bob Builder");
        assert_eq!(t.people[2].href, None, "non-http links are dropped");
        assert_eq!(t.extras.funding, vec![Funding { url: "https://fund.example".into(), description: "Support us".into() }]);
        assert_eq!(t.extras.podroll.len(), 2);
        assert_eq!(t.extras.value[0].recipients.len(), 2);
        assert_eq!(t.items.len(), 1, "items without tags aren't kept");
        let it = &t.items[0];
        assert_eq!(it.guid.as_deref(), Some("e-2"));
        assert_eq!(it.enclosure_url.as_deref(), Some("https://cdn/e2.mp3?a=1&b=2"));
        assert_eq!(it.people[0].name, "Carol");
        assert_eq!(it.chapters_url.as_deref(), Some("https://cdn/e2.json"));
        assert_eq!(it.transcripts[0].language.as_deref(), Some("en"));
        assert!(t.has_people());
        assert_eq!(parse(&FEED.replace('\n', "")).fingerprint(), t.fingerprint(), "layout doesn't matter");
    }

    #[test]
    fn normalizes() {
        assert_eq!(normalize_name("  Griffin   McElroy "), "griffin mcelroy");
        assert_eq!(normalize_name("Beyoncé"), "beyonce");
        assert_eq!(normalize_name("J.R.R. Tolkien"), "jrr tolkien");
        assert_eq!(normalize_role(Some("Host")), "host");
        assert_eq!(normalize_role(None), "host");
        assert_eq!(normalize_role(Some("CoHost")), "co-host");
    }
}
