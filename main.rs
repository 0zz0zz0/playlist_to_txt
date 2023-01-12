use tokio;
use rspotify::{Credentials, AuthCodeSpotify, clients::OAuthClient, scopes, model::{PlaylistId, Market, PlayableItem, PlaylistItem}, prelude::BaseClient};
use std::{io};
use url::Url;
use std::fs::File;
use std::io::Write;
use chrono::{self};
use clearscreen;


fn clear() {
    clearscreen::clear().unwrap();
}

fn read_line(input:&mut String) {
    io::stdin().read_line(input).expect("Failed to read line");
    *input = input.trim().to_string();
}

async fn auth_client() -> AuthCodeSpotify {
    let creds = Credentials::new("700a3a99a6664021af9e0f4ed2f94fa7", "13d5f0cd8f5645b984538febcb6154c6");
    let oauth = rspotify::OAuth { redirect_uri: "http://localhost:8888/callback".to_string(), scopes: scopes!("playlist-read-private"), ..Default::default()};

    let spotify = AuthCodeSpotify::new(creds, oauth);

    let url = spotify.get_authorize_url(false).unwrap();
    spotify.prompt_for_token(&url).await.expect("fuck you");
    spotify
}

#[derive(Debug)]
enum RidError {
    NotUrl,
    BadUrl,

}



fn get_idstr(url: Url) -> Result<String, RidError> {
    match url.path_segments() {
        Some(mut segmentiter) => match segmentiter.nth(1) {
            Some(segment) => Ok(segment.to_string()),
            None => Err(RidError::BadUrl)
        },
        None => Err(RidError::BadUrl)
    }
}

fn get_id(idstr: String) -> Result<PlaylistId<'static>, RidError> {
    match PlaylistId::from_id(idstr) {
        Ok(id) => Ok(id),
        Err(_) => Err(RidError::BadUrl)
    }
}

fn get_url(input:&mut String) -> Result<Url, RidError> {
    match Url::parse(input) {
        Ok(url) => Ok(url),
        Err(_) => Err(RidError::NotUrl)
    }
}

async fn url_menu(spotify: &AuthCodeSpotify) -> Option<PlaylistId<'static>> {
    let identity: Option<PlaylistId>;
    loop{
        println!("Press ENTER for a list of your saved playlists or input a link to a playist>");
        let mut input = String::new();
        read_line(&mut input);
        match input.len() {
            0 => {
                identity = pick_list(&spotify).await;
                break
            },
            _ => {
                match get_url(&mut input) {
                    Ok(url) => match get_idstr(url) {
                        Ok(idstr) => match get_id(idstr) {
                            Ok(id) => {
                                identity = Some(id);
                                break
                            },
                            Err(_) => println!("Input was a URL but did not contain a valid spotify id")
                        },
                        Err(_) => println!("Input was a URL but did not contain a valid spotify id")
                    },
                    Err(_) => println!("Input was not a valid URL")
                }
            }
        }
    }
    identity
}

async fn pick_list(spotify: &AuthCodeSpotify) -> Option<PlaylistId<'static>> {
    let mut offset = 0;
    let identity: Option<PlaylistId<'static>>;
    clear();
    loop{
        let playlists = spotify.current_user_playlists_manual(Some(20), Some(offset)).await.expect("Failed to retrieve use playlists");
        let page = (offset/20) + 1;
        println!("Page {}/{} of saved playlists", page, playlists.total/20);

        let mut count = 0;
        for i in &playlists.items {
            println!("{}: Playlist: {}", count, i.name);
            count += 1;
        }

        println!("Page {}/{} of saved playlists", page, playlists.total/20);
        
        if offset < 19 {
            println!("Select a playlist by number. N for Next page.")
        } else {
            println!("Select a playlist by number. N for Next page. P for Previous page.")
        }
        
        let mut input = String::new();
        read_line(&mut input);

        if let Ok(num) = input.parse::<usize>() {
            if num <= 19 {
                identity = Some(playlists.items[num].id.to_owned());
                break
            }
        }

        match input.to_lowercase().as_str() {
            "n" => {offset += 20; clear();},
            "p" => {if offset >= 20 {offset -= 20;} clear();},
            _ => {
                clear();
                println!("Not a valid input")
            }
        }

    }
    identity
}

fn write_item(item: PlaylistItem, counter: &mut i32, file: &mut File) {
    let mut trackname = String::new();
    let mut artist = String::new();
    match item.track.unwrap() {
        PlayableItem::Track(t) => {
            trackname.push_str(&t.name);
            let artist_vec = t.artists;
            let mut iter = artist_vec.len();
            for a in artist_vec {
                iter -=1;
                artist.push_str(&a.name);
                if iter > 0 {
                    artist.push_str(", ")
                }
            }
        },
        PlayableItem::Episode(e) =>{
            trackname = e.name;
            artist = e.show.name;
        }
    }
    file.write_all(format!("\n{}: {}; {}", counter, trackname, artist).as_bytes()).expect("Failed to write to output file");
    *counter += 1;
}

async fn write_data(id: PlaylistId<'_>, spotify: &AuthCodeSpotify) {
    let to_txt = spotify.playlist(id.as_ref(), None, Some(Market::FromToken)).await;
    match to_txt {
        Ok(playlist) => {
            let path = format!("{}.txt", playlist.name);
            let mut file = File::create(&path).expect("Failed to create output file");
            file.write_all(format!("Snapshot Created: {}\n\nPLAYLIST: {}\nDESCRIPTION: {}\nCREATED BY: {}\n\nTRACK NAME; ARTIST", chrono::offset::Local::now(), playlist.name, playlist.description.unwrap_or(String::from("")), playlist.owner.display_name.unwrap_or(String::from("null"))).as_bytes()).expect("Failed to write to output file");
            let total = playlist.tracks.total;
            let mut offset = 0;
            let mut counter = 1;
            while offset < total {
                let items = spotify.playlist_items_manual(id.as_ref(), None, Some(Market::FromToken), Some(100), Some(offset)).await.expect("Failed to retrieve playlist data");
                for i in items.items {
                    write_item(i, &mut counter, &mut file);
                }
                offset += 100;
            }
            println!("Playlist saved in current directory as '{}'", path);
            
        },
        Err(_) => println!("Unable to retrieve playlist data")
    }
}


#[tokio::main]
async fn main() {
    // greeting
    clear();
    let mut input = String::new();
    println!("Welcome to the playlist_to_txt! Press ENTER to sign in.");
    read_line(&mut input);

    // authenticate client
    let spotify = auth_client().await;
    
    // authentication complete
    clear();
    println!("Authentication Complete!");

    // link vs list menu
    let id = url_menu(&spotify).await.unwrap();
    // print playlist to .txt file
    write_data(id, &spotify).await;
    
}
    