use web3::futures::StreamExt;
use web3::types::{Address, FilterBuilder, Log, H256};
use web3::ethabi;
use hex_literal::hex;
use ethabi::{Event, EventParam, ParamType, RawLog};
use crate::config::*;
use crate::model::PoM;

async fn listen_events() {
    let web3 = web3::Web3::new(
        web3::transports::WebSocket::new(ETH_ADDR)
            .await
            .unwrap(),
    );
    let contract_address: Address = TENET_CONTRACT_L1_ADDR.parse().unwrap();

    let filter = FilterBuilder::default()
        .address(vec![contract_address])
        .topics(Some(vec![H256::from_slice(&hex!(
            "b99104c672a3595b68bee7f956d813e12ceef4312cf778729dcbb860c3473e5a"
        ))]), None, None, None)
        .build();
    let mut subscription =
        web3.eth_subscribe().subscribe_logs(filter).await;
    println!("Subscribed to logs with subscription id: {:?}", subscription);

    while let Some(log) = 
        subscription.as_mut().expect("subscription").next().await {
        match log {
            Ok(log) => handle_log(log),
            Err(e) => eprintln!("Error fetching log: {}", e),
        }
    }

}


fn handle_log(log: Log) {
    let event = Event {
        name: "ChallengeEvent".to_owned(),
        inputs: vec![
            EventParam {
                name: "data".to_owned(),
                kind: ParamType::Bytes,
                indexed: false,
            },
        ],
        anonymous: false,
    };

    let raw_log = RawLog {
        topics: log.topics,
        data: log.data.0,
    };
    let decoded_logs = event.parse_log(raw_log).expect("Failed to parse log");
    let json_bytes: Vec<u8> = decoded_logs.params[0].value.clone().into_bytes().unwrap();
    let json_string = String::from_utf8(json_bytes).unwrap();
    let pom = PoM::from_json(json_string.as_ref());
    println!("Recived PoM: {:?}", pom.clone());
    if pom.state == crate::fsm::State::Challenging {
        crate::call_tree::handle_challenge(pom.clone());
    } else if pom.state == crate::fsm::State::Responsed {
        crate::call_tree::handle_response(pom.clone());
    }
    crate::call_tree::check_timeout_and_punish(pom);
}


#[tokio::test]
async fn test_listen_events() -> Result<(), Box<dyn std::error::Error>> {
    listen_events().await;
    Ok(())
}

#[tokio::test]
async fn calc_topic() -> Result<(), Box<dyn std::error::Error>> {
    let event = Event {
        name: "ChallengeEvent".to_string(),
        inputs: vec![EventParam {
            name: "data".to_string(), 
            kind: ParamType::Bytes,
            indexed: false,
        }],
        anonymous: false,
    };

    let topic = event.signature();
    println!("Event topic: {:?}", topic);

    Ok(())
}

