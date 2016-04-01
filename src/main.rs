extern crate irc;
extern crate hyper;
extern crate rustc_serialize;
extern crate url;

use irc::client::prelude::*;
use irc::client::data::Command::PRIVMSG;
use std::io::Read;
use rustc_serialize::json;
use rustc_serialize::json::Json;
use url::percent_encoding;
use std::collections::HashMap;

fn main() {
    let mut weather_savedlocations : HashMap<String, String> = HashMap::new();
    weather_savedlocations.insert("hi".to_string(), "1".to_string());

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

    let httpclient = hyper::Client::new();
    let apikey = match server.config().options.as_ref()
        .and_then(|i| i.get("weather_apikey")) {
            Some(j) => j,
            None => return Err(()),
    };
    let httpurl = format!(
        "http://api.openweathermap.org/data/2.5/weather?q={}&units=metric&APPID={}",
        percent_encoding::percent_encode(location.as_bytes(), percent_encoding::QUERY_ENCODE_SET),
        apikey);
    let mut httpreq = match httpclient.get(&httpurl)
        .header(hyper::header::Connection::close())
        .send() {
            Ok(req) => req,
            Err(_) => return Err(()),
    };
    let mut httpbody = String::new();
    let _size = httpreq.read_to_string(&mut httpbody);

    let data = match Json::from_str(&httpbody) {
        Ok(d) => d,
        Err(_) => return Err(()),
    };
    println!("{}", data);

    let returncode = match data.find("cod") {
        Some(i) => i,
        None => return Err(()),
    };
    if returncode.is_string() {
        if returncode.as_string().unwrap() == "404" {
            server.send_privmsg(channel,
                                &format!("Location ({}) not found", arguments))
                .unwrap();
        }
        return Err(());
    }
    else if returncode.is_number() {
        if returncode.as_u64().unwrap() != 200 {
            server.send_privmsg(channel, "Error").unwrap();
            return Err(());
        }
    }
    else {
        return Err(());
    }

    saved_locations.insert(user.to_string(), location.to_string());

    let city = match data.find("name")
        .and_then(|i| i.as_string()) {
            Some(j) => j,
            None => return Err(()),
    };
    let country = match data.find("sys")
        .and_then(|i| i.find("country"))
        .and_then(|j| j.as_string()) {
            Some(k) => k,
            None => return Err(()),
    };

    let weather_arr = match data.find("weather")
        .and_then(|i| i.as_array()) {
            Some(j) => j,
            None => return Err(()),
    };
    if weather_arr.len() == 0 {
        return Err(());
    }
    let weather_main = match weather_arr[0].find("main")
        .and_then(|i| i.as_string()) {
            Some(j) => j,
            None => return Err(()),
    };
    let weather_desc = match weather_arr[0].find("description")
        .and_then(|i| i.as_string()) {
            Some(j) => j,
            None => return Err(()),
    };
    let weather_temp = match data.find("main")
        .and_then(|i| i.find("temp"))
        .and_then(|j| j.as_f64()) {
            Some(k) => k,
            None => return Err(()),
    };
    let weather_humidity = match data.find("main")
        .and_then(|i| i.find("humidity")).and_then(|j| j.as_f64()) {
        Some(k) => k,
        None => return Err(()),
    };
    let weather_windspeed = match data.find("wind")
        .and_then(|i| i.find("speed"))
        .and_then(|j| j.as_f64()) {
            Some(k) => k,
            None => return Err(()),
    };

    let reply = format!("{}, {}: {} ({}) {:.0}C, {:.0} km/hr wind, {}% humidity",
        city,
        country,
        weather_main,
        weather_desc,
        weather_temp,
        weather_windspeed*3.6,
        weather_humidity);

    server.send_privmsg(channel, &reply).unwrap();
    return Ok(());
}
