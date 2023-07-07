use anyhow::Result;
use axum::{
    extract::{Path, State},
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use std::{io::BufReader, sync::Arc};
use xml::reader::XmlEvent;
use xml::EventReader;

struct Podcast {
    title: String,
    description: String,
    audio_file: Option<String>,
}

impl Podcast {
    fn new() -> Self {
        Self {
            title: String::new(),
            description: String::new(),
            audio_file: None,
        }
    }

    // 💡 Note for experienced developers: Not happy that I use new for an empty struct and to_html for an HTML representation? You are absolutely right! This is a challenge for you, make this code more idiomatic and closer to what we usually see in Rust. TODO:
    // - Use the Default trait instead of new
    // - Implement IntoResponse for Podcast instead of having its own method.
    fn to_html(&self) -> String {
        format!(
            r#"
<html>
    <head>
        <title>Nava podcast #{}</title>
        <link rel="stylesheet" href="https://unpkg.com/mvp.css">
        <style>
            .fa-chevron-right {{
                display: none;
            }}
            .post-more-link {{
                margin-left: 0.5rem;
            }}
        </style>
    </head>
    <body>
        <main>
            <article>
                <a href="/">&larr; all episodes</a>
                <h1># {}</h1>
                <audio controls src="{}"></audio>
                <h2>## description & transcript</h2>
                <p>{}</p>
            </article>
        </main>
    </body>
</html>
        "#,
            self.title,
            self.title,
            match self.audio_file {
                Some(ref file) => file,
                None => "No audio available",
            },
            self.description
        )
    }
}

async fn podcast(State(app_state): State<AppState>, Path(id): Path<usize>) -> impl IntoResponse {
    let podcast = app_state.get(id);
    Html(match podcast {
        Some(podcast) => podcast.to_html(),
        None => "No podcast found".to_string(),
    })
}

async fn root(State(app_state): State<AppState>) -> impl IntoResponse {
    let res = format!(
        r#"
<html>
    <head>
        <title>Naval podcast feed</title>
        <link rel="stylesheet" href="https://unpkg.com/mvp.css">
    </head>
    <body>
        <main>
            <h1>Naval podcast feed</h1>
            <div>
                <ul>
                    {}
                </ul>
            </div>
        </main>
    </body>
</html>
        "#,
        app_state
            .iter()
            .enumerate()
            .map(|(id, podcast)| { format!(r#"<li><a href="/{}">{}</a></li>"#, id, podcast.title) })
            .collect::<Vec<String>>()
            .join("\n")
    );
    Html(res)
}

enum ParseState {
    Start,
    InTitle,
    InDescription,
}

async fn read_prodcast_from_xml(url: &str) -> Result<Vec<Podcast>> {
    let mut results = Vec::new();
    let data = reqwest::get(url).await?.text().await?;
    let parser = EventReader::new(BufReader::new(data.as_bytes()));
    // TODO: reverse the counter, need total length of episode tags
    let mut episode_counter = 1;
    let mut podcast = Podcast::new();
    let mut state = ParseState::Start;
    for event in parser {
        match event {
            Ok(XmlEvent::StartElement {
                name, attributes, ..
            }) => match name.local_name.as_str() {
                "title" => state = ParseState::InTitle,
                "description" => state = ParseState::InDescription,
                "enclosure" => {
                    podcast.audio_file = attributes.into_iter().find_map(|attr| {
                        if attr.name.local_name == "url" {
                            Some(attr.value)
                        } else {
                            None
                        }
                    });
                }
                _ => {}
            },
            Ok(XmlEvent::CData(content)) => match state {
                ParseState::InTitle => {
                    podcast.title = format!("episod #{}", episode_counter);
                    episode_counter += 1;
                    state = ParseState::Start;
                }
                ParseState::InDescription => {
                    podcast.description = content;
                    state = ParseState::Start;
                }
                _ => {}
            },
            Ok(XmlEvent::EndElement { name }) => {
                if name.local_name == "item" {
                    results.push(podcast);
                    podcast = Podcast::new();
                    state = ParseState::Start;
                }
            }
            _ => {}
        }
    }
    Ok(results)
}

type AppState = Arc<Vec<Podcast>>;

#[shuttle_runtime::main]
async fn axum() -> shuttle_axum::ShuttleAxum {
    let podcasts = read_prodcast_from_xml("https://nav.al/feed").await?;
    let app_state = Arc::new(podcasts);
    let router = Router::new()
        .route("/", get(root))
        .route("/:id", get(podcast))
        .with_state(app_state);

    Ok(router.into())
}
