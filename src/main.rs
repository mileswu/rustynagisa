extern crate irc;
extern crate hyper;
extern crate rustc_serialize;
extern crate url;

use irc::client::prelude::*;
use irc::client::data::Command::PRIVMSG;
use rustc_serialize::json;
use rustc_serialize::json::Json;
use url::percent_encoding;
use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;

fn main() {
    let mut weather_savedlocations : HashMap<String, String> = HashMap::new();
    let mut weather_savedlocations_json = String::new();
    match File::open("weather_savedlocations.json")
               .and_then(|mut i| i.read_to_string(&mut weather_savedlocations_json)) {
        Ok(_) => {
            match json::decode(&weather_savedlocations_json) {
                Ok(j) => {weather_savedlocations = j;},
                Err(_) => {},
            };
        },
        Err(_) => {},
    }

    let server = IrcServer::new("config.json").unwrap();
    server.identify().unwrap();
    for message in server.iter() {
        let msg = message.unwrap();
        print!("{}", msg);
        if let PRIVMSG(ref channel, ref text) = msg.command {
            // Ignore direct messages
            if channel == server.config().nickname() {
                continue;
            }

            let user = match msg.source_nickname() {
                Some(i) => i,
                None => continue,
            };

            let command = text.split_whitespace().next().unwrap();
            let (_, arguments) = text.split_at(command.len());
            let _result = match command {
                "!w" => weather(&server, &mut weather_savedlocations, channel, user, arguments.trim()),
                _ => Ok(()),
            };
        }
    }
    return;
}

fn get_lonlat(server: &IrcServer, location: &str) -> Result<(f64, f64, String), ()> {
    let apikey = match server.config().options.as_ref()
        .and_then(|i| i.get("gmaps_geocoding_apikey")) {
            Some(j) => j,
            None => return Err(()),
    };

    let httpurl = format!(
        "https://maps.googleapis.com/maps/api/geocode/json?address={}&key={}",
        percent_encoding::percent_encode(location.as_bytes(), percent_encoding::QUERY_ENCODE_SET),
        apikey);
    let httpclient = hyper::Client::new();
    let mut httpreq = match httpclient.get(&httpurl)
        .header(hyper::header::Connection::close())
        .send() {
            Ok(req) => req,
            Err(_) => return Err(()),
    };

    if httpreq.status != hyper::status::StatusCode::Ok {
        return Err(());
    }

    let mut httpbody = String::new();
    let _size = httpreq.read_to_string(&mut httpbody);

    let data = match Json::from_str(&httpbody) {
        Ok(d) => d,
        Err(_) => return Err(()),
    };

    let status = match data.find("status")
        .and_then(|i| i.as_string()) {
            Some(j) => j,
            None => return Err(()),
    };
    if status != "OK" {
        return Err(());
    }

    let results = match data.search("results")
        .and_then(|i| i.as_array()) {
            Some(j) => j,
            None => return Err(()),
    };

    if results.len() == 0 {
        return Err(());
    }

    let formatted_address = match results[0].find("formatted_address")
        .and_then(|i| i.as_string()) {
            Some(j) => j,
            None => return Err(()),
    };

    let location = match results[0].find("geometry")
        .and_then(|i| i.find("location")) {
            Some(j) => j,
            None => return Err(()),
    };

    let lat = match location.find("lat")
        .and_then(|i| i.as_f64()) {
            Some(j) => j,
            None => return Err(()),
    };
    let lon = match location.find("lng")
        .and_then(|i| i.as_f64()) {
            Some(j) => j,
            None => return Err(()),
    };

    return Ok((lat, lon, formatted_address.to_string()));
}

fn weather(server: &IrcServer, saved_locations: &mut HashMap<String, String>,
           channel: &str, user: &str, arguments: &str)
           -> Result<(), ()> {
    let mut location = arguments.to_string();
    if arguments.len() == 0 {
        location = match saved_locations.get(user) {
            Some(i) => i.clone(),
            None => {
                server.send_privmsg(channel, "No saved location").unwrap();
                return Err(());
            },
        }
    }

    let (lon, lat, formatted_address) = match get_lonlat(server, &location) {
        Ok(i) => i,
        Err(_) => return Err(()),
    };

    saved_locations.insert(user.to_string(), location.to_string());
    let saved_locations_json = json::encode(saved_locations).unwrap();
    File::create("weather_savedlocations.json")
        .and_then(|mut i| i.write_all(saved_locations_json.as_bytes()))
        .unwrap();

    let httpclient = hyper::Client::new();
    let forecast_apikey = match server.config().options.as_ref()
        .and_then(|i| i.get("forecast_apikey")) {
            Some(j) => j,
            None => return Err(()),
    };

    let httpurl = format!(
        "https://api.forecast.io/forecast/{}/{},{}?units=si",
        forecast_apikey,
        lon, lat);
    let mut httpreq = match httpclient.get(&httpurl)
        .header(hyper::header::Connection::close())
        .send() {
            Ok(req) => req,
            Err(_) => return Err(()),
    };

    if httpreq.status != hyper::status::StatusCode::Ok {
        return Err(());
    }

    let mut httpbody = String::new();
    let _size = httpreq.read_to_string(&mut httpbody);

    let data = match Json::from_str(&httpbody) {
        Ok(d) => d,
        Err(_) => return Err(()),
    };

    let data_current = match data.find("currently") {
        Some(i) => i,
        None => return Err(()),
    };

    let summary = match data_current.find("summary")
        .and_then(|i| i.as_string()) {
            Some(j) => j,
            None => return Err(()),
    };

    let temperature = match data_current.find("temperature")
        .and_then(|i| i.as_f64()) {
            Some(j) => j,
            None => return Err(()),
    };

    let humidity = match data_current.find("humidity")
        .and_then(|i| i.as_f64()) {
            Some(j) => j,
            None => return Err(()),
    };

    let windspeed = match data_current.find("windSpeed")
        .and_then(|i| i.as_f64()) {
            Some(j) => j,
            None => return Err(()),
    };

    let reply = format!("{}: {} {:.0}C, {:.0} km/hr wind, {:.0}% humidity",
        formatted_address,
        summary,
        temperature,
        windspeed*3.6,
        humidity*100.0);

    server.send_privmsg(channel, &reply).unwrap();
    return Ok(());
}
