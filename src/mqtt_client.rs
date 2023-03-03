use crate::LEDMessage;
use bytes::{Bytes, BytesMut};
use esp_idf_sys::esp_task_wdt_reset;
use mqtt_v5::decoder::decode_mqtt;
use mqtt_v5::encoder::encode_mqtt;
use mqtt_v5::topic::Topic;
use mqtt_v5::types::{ConnectPacket, FinalWill, Packet, SubscribePacket, SubscriptionTopic};
use smol::channel::Sender;
use smol::io::{AsyncReadExt, AsyncWriteExt};
use smol::net::TcpStream as SmolStream;
use smol::stream::StreamExt;
use std::io::BufRead;
use std::str::FromStr;

pub async fn subscribe_to_channel(mut sender: Sender<LEDMessage>) -> anyhow::Result<()> {
    sender
        .send(LEDMessage::Color((255, 255, 255)))
        .await
        .unwrap();
    loop {
        println!("Subscribing to channel...");
        let mut stream = SmolStream::connect("192.168.0.209:1883").await?;
        let connect_packet = Packet::Connect(ConnectPacket {
            client_id: "esp32-c3".to_string(),
            keep_alive: 200,
            will: Some(FinalWill {
                topic: Topic::from_str("esp32-c3").unwrap(),
                payload: Bytes::from("offline"),
                qos: Default::default(),
                should_retain: false,
                will_delay_interval: None,
                payload_format_indicator: None,
                message_expiry_interval: None,
                content_type: None,
                response_topic: None,
                correlation_data: None,
                user_properties: vec![],
            }),
            ..Default::default()
        });
        let mut write_buf = BytesMut::with_capacity(512);
        encode_mqtt(&connect_packet, &mut write_buf, Default::default());
        stream.write(&write_buf).await?;
        stream.flush().await?;
        let mut read_buf = [0u8; 512];
        println!("Waiting for ConnAck..");
        for i in 0..100 {
            stream.read(&mut read_buf).await?;
            let packet =
                decode_mqtt(&mut BytesMut::from(read_buf.as_slice()), Default::default()).unwrap();
            match packet {
                Some(Packet::ConnectAck(connack)) => {
                    println!("Got ConnAck! {:?}", connack);
                    break;
                }
                Some(x) => {
                    println!("Got {:?}", x);
                }
                None => {}
            }
        }
        let subscribe_packet =
            Packet::Subscribe(SubscribePacket::new(vec![SubscriptionTopic::new_concrete(
                "esp32-c3",
            )]));
        encode_mqtt(&subscribe_packet, &mut write_buf, Default::default());
        stream.write_all(&write_buf).await?;
        stream.flush().await?;
        println!("Waiting for suback...");
        for i in 0..100 {
            stream.read_exact(&mut read_buf).await?;
            let packet =
                decode_mqtt(&mut BytesMut::from(read_buf.as_slice()), Default::default()).unwrap();
            match packet {
                Some(Packet::SubscribeAck(suback)) => {
                    println!("Suback: {:?}", suback);
                    break;
                }
                Some(x) => {
                    println!("Got {:?}", x);
                }
                None => {}
            }
        }
        let hopefullysuback =
            decode_mqtt(&mut BytesMut::from(read_buf.as_slice()), Default::default()).unwrap();
        println!("Hopefully suback: {:?}", hopefullysuback);
        loop {
            stream.read_exact(&mut read_buf).await?;
            unsafe {
                esp_task_wdt_reset();
            }
            let packet =
                decode_mqtt(&mut BytesMut::from(read_buf.as_slice()), Default::default()).unwrap();
            match packet {
                Some(Packet::Publish(publish)) => {
                    println!("Publish: {:?}", publish);
                    let payload = publish.payload;
                    // We need to cut the payload at the first 0 byte
                    match serde_json::de::from_slice(payload.split(|&x| x == 0).next().unwrap()) {
                        Ok(msg) => {
                            println!("Sending message: {:?}", msg);
                            sender.send(msg).await.unwrap();
                        }
                        Err(e) => {
                            println!("Error: {:?}", e);
                        }
                    }
                }
                _ => {}
            }
        }
    }
}
