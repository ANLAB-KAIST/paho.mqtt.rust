// paho-mqtt/examples/rpc_math_cli.rs
//
//! This is a Paho MQTT v5 C++ sample application.
//!
//! It's an example of how to create a client for performing remote procedure
//! calls using MQTT with the 'response topic' and 'correlation data'
//! properties.
//!
//! The sample demonstrates:
//!  - Creating a dynamic RPC client for MQTT v5
//!  - Connecting to an MQTT v5 server/broker
//!  - Using MQTT v5 properties
//!  - Publishing RPC request messages
//!  - Using asynchronous tokens
//!	 - Subscribing to reply topic
//

/*******************************************************************************
 * Copyright (c) 2019 Frank Pagliughi <fpagliughi@mindspring.com>
 *
 * All rights reserved. This program and the accompanying materials
 * are made available under the terms of the Eclipse Public License v1.0
 * and Eclipse Distribution License v1.0 which accompany this distribution.
 *
 * The Eclipse Public License is available at
 *    http://www.eclipse.org/legal/epl-v10.html
 * and the Eclipse Distribution License is available at
 *   http://www.eclipse.org/org/documents/edl-v10.php.
 *
 * Contributors:
 *    Frank Pagliughi - initial implementation and documentation
 *******************************************************************************/

extern crate futures;
extern crate log;
extern crate env_logger;
extern crate serde_json;
extern crate paho_mqtt as mqtt;

use std::{env, process};
use futures::Future;
use serde_json::json;

fn main() -> mqtt::MqttResult<()> {
    // Initialize the logger from the environment
    env_logger::init();

    // We use the broker on this host.
    let host = "localhost";

    // Command-line option(s)
    let args: Vec<String> = env::args().skip(1).collect();

    if args.len() < 3 {
        println!("USAGE: rpc_math_cli <add|mult> <num1> <num2> [... numN]");
        process::exit(1);
    }

    const QOS: i32 = 1;
    const MQTTV5: u32 = 5;

    const REQ_TOPIC_HDR: &'static str = "requests/math";
    const REP_TOPIC_HDR: &'static str = "replies/math";

    // Create a client to the specified host, no persistence
    let create_opts = mqtt::CreateOptionsBuilder::new()
        .mqtt_version(MQTTV5)
        .server_uri(host)
        .persistence(mqtt::PersistenceType::None)
        .finalize();


    let mut cli = mqtt::AsyncClient::new(create_opts).unwrap_or_else(|err| {
        eprintln!("Error creating the client: {}", err);
        process::exit(1);
    });

    // Initialize the consumer before connecting.
    // With a clean session/start, this order isn't important,
    // but it's still a good habit to start consuming first.
    let rx = cli.start_consuming();

    // Connect with default options
    let conn_opts = mqtt::ConnectOptionsBuilder::new()
        .mqtt_version(MQTTV5)
        .clean_start(true)
        .finalize();

    // Connect and wait for it to complete or fail

    let rsp = cli.connect(conn_opts).wait().unwrap_or_else(|err| {
        eprintln!("Unable to connect: {:?}", err);
        process::exit(1);
    });

    // We get the assigned Client ID from the properties in the connection
    // response. The Client ID will help form a unique "reply to" topic
    // for us.

    /*
    if let Some((_uri, _ver, session)) = rsp.connect_response() {
        println!("Existing session: {}", session);
    }
    */

    let client_id = rsp.properties()
        .get(mqtt::PropertyCode::ASSIGNED_CLIENT_IDENTIFER).unwrap()
        .get_string().unwrap_or("<unknown>".to_string());

    //println!("Client ID: {}", client_id);

    // We form a unique reply topic based on the Client ID,
    // and then subscribe to that topic.
    // (Be sure to subscribe *before* starting to send requests)
    let reply_topic = format!("{}/{}", REP_TOPIC_HDR, client_id);
    cli.subscribe(&reply_topic, QOS).wait()?;

    // The request topic will be of the form:
    //     "requests/math/<operation>"
    // where we get <operation> ("add", "mult", etc) from the command line.

    let req_topic = format!("{}/{}", REQ_TOPIC_HDR, args[0]);
    let corr_id = "1".as_bytes();

    let mut props = mqtt::Properties::new();
    props.push_string(mqtt::PropertyCode::RESPONSE_TOPIC, &reply_topic).unwrap();
    props.push_binary(mqtt::PropertyCode::CORRELATION_DATA, corr_id).unwrap();

    // The payload is the JSON array of arguments for the operation.
    // These are the remaining arguments from the command line.

    let math_args: Vec<f64> = args[1..].iter().map(|x| x.parse::<f64>().unwrap()).collect();
    let payload = json!(math_args).to_string();

    // Create a message and publish it
    let msg = mqtt::MessageBuilder::new()
        .topic(req_topic)
        .payload(payload)
        .qos(QOS)
        .properties(props)
        .finalize();

    let tok = cli.publish(msg);

    if let Err(e) = tok.wait() {
        eprintln!("Error sending message: {:?}", e);
        cli.disconnect(None).wait().unwrap();
        process::exit(2);
    }

    // Wait for the reply

    if let Some(msg) = rx.recv().unwrap() {
        println!("Reply Message: {:?}", msg);
        let reply_corr_id = msg.properties()
            .get(mqtt::PropertyCode::CORRELATION_DATA).unwrap()
            .get_binary().unwrap_or(vec![]);
        println!("Reply correlation: {:?}", reply_corr_id);

        let ret: f64 = serde_json::from_str(&msg.payload_str()).unwrap();
        println!("{}", ret);
    }
    else {
        eprintln!("Error receiving reply.");
    }

    // Disconnect from the broker
    cli.disconnect(None).wait()?;
    Ok(())
}

