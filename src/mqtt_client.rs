use crate::LEDMessage;

async fn subscribe_to_channel(mut sender: Sender<LEDMessage>) -> anyhow::Result<()> {
    sender.send(LEDMessage::Off).await.unwrap();
    loop {
        println!("Subscribing to channel...");
        let mut stream = SmolStream::connect("192.168.0.209:1883").await?;
        let connect_packet = Packet::Connect(ConnectPacket {
            client_id: "esp32-c3".to_string(),
            keep_alive: 20000,
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
        let mut write_buf = BytesMut::new();
        encode_mqtt(&connect_packet, &mut write_buf, Default::default());
        stream.write(&write_buf).await?;
        stream.flush().await?;
        let mut read_buf = BytesMut::new();
        println!("Waiting for ConnAck");
        loop {
            println!("Reading...");
            stream.read(&mut read_buf).await?;
            let packet = decode_mqtt(&mut read_buf, Default::default()).unwrap();
            match packet {
                Some(Packet::ConnectAck(connack)) => {
                    println!("Got ConnAck! {:?}", connack);
                    break;
                }
                _ => {}
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
        loop {
            stream.read_exact(&mut read_buf).await?;
            let packet = decode_mqtt(&mut read_buf, Default::default()).unwrap();
            match packet {
                Some(Packet::SubscribeAck(suback)) => {
                    println!("Suback: {:?}", suback);
                    break;
                }
                _ => {}
            }
        }
        let hopefullysuback = decode_mqtt(&mut read_buf, Default::default()).unwrap();
        println!("Hopefully suback: {:?}", hopefullysuback);
        loop {
            let mut read_buf = BytesMut::new();
            stream.read_exact(&mut read_buf).await?;
            unsafe {
                esp_task_wdt_reset();
            }
            let packet = decode_mqtt(&mut read_buf, Default::default()).unwrap();
            match packet {
                Some(Packet::Publish(publish)) => {
                    println!("Publish: {:?}", publish);
                    let payload = publish.payload;
                    if let Ok(color_message) = serde_json::de::from_slice(&payload) {
                        sender.send(color_message).await?;
                    }
                }
                _ => {}
            }
        }
    }
}
