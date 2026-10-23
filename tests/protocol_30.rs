use base64::Engine;
use serde_json::json;
use soroban_client::address::{Address, AddressTrait};
use soroban_client::soroban_rpc::EventResponse;
use soroban_client::xdr::{self, Limits, WriteXdr};
use soroban_client::{Durability, Options, Pagination, Server};

#[test]
fn protocol_30_events_accept_removed_success_field() {
    let event: EventResponse = serde_json::from_value(json!({
        "type": "contract",
        "ledger": 42,
        "ledgerClosedAt": "2026-11-03T17:00:00Z",
        "contractId": "CA7QYNF7SOWQ3GLR2BGMZEHXAVIRZA4KVWLTJJFC7MGXUA74P7UJUWDA",
        "id": "42-0",
        "txHash": "00".repeat(32),
        "topic": [],
        "value": "AAAAAQ=="
    }))
    .expect("RPC 30 omits inSuccessfulContractCall");
    assert_eq!(event.ledger, 42);
    assert_eq!(event.in_successful_contract_call, None);
}

#[test]
fn protocol_30_events_preserve_legacy_success_flag_and_muxed_values() {
    let encoded = "WA7QYNF7SOWQ3GLR2BGMZEHXAVIRZA4KVWLTJJFC7MGXUA74P7UJVAAAAAAAAAAAACWJY";
    let value = Address::new(encoded).unwrap().to_sc_val().unwrap();
    let event: EventResponse = serde_json::from_value(json!({
        "type": "contract", "ledger": 42, "ledgerClosedAt": "2026-11-03T17:00:00Z",
        "contractId": "CA7QYNF7SOWQ3GLR2BGMZEHXAVIRZA4KVWLTJJFC7MGXUA74P7UJUWDA",
        "id": "42-0", "txHash": "00".repeat(32), "topic": [],
        "inSuccessfulContractCall": false,
        "value": value.to_xdr_base64(Limits::none()).unwrap()
    }))
    .unwrap();
    assert_eq!(event.in_successful_contract_call, Some(false));
    assert_eq!(
        Address::from_sc_val(&event.value()).unwrap().to_string(),
        encoded
    );
}

#[tokio::test]
async fn protocol_30_get_ledgers_decodes_headers_and_metadata() {
    use wiremock::matchers::{body_partial_json, method};
    use wiremock::{Mock, MockServer, ResponseTemplate};
    let mock = MockServer::start().await;
    let mut ledgers = Vec::new();
    for (name, header, metadata) in [
        (
            "signed_ms",
            include_bytes!("fixtures/signed_ms.xdr").as_slice(),
            include_bytes!("fixtures/signed_ms_meta.xdr").as_slice(),
        ),
        (
            "empty_tx_set_ms",
            include_bytes!("fixtures/empty_tx_set_ms.xdr").as_slice(),
            include_bytes!("fixtures/empty_tx_set_ms_meta.xdr").as_slice(),
        ),
    ] {
        ledgers.push(json!({
            "hash": name, "sequence": 42, "ledgerCloseTime": "1700000000",
            "headerXdr": base64::engine::general_purpose::STANDARD.encode(header),
            "metadataXdr": base64::engine::general_purpose::STANDARD.encode(metadata),
        }));
    }
    Mock::given(method("POST"))
        .and(body_partial_json(
            json!({"method": "getLedgers", "params": {"startLedger": 42}}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "jsonrpc": "2.0", "id": 1, "result": {
                "latestLedger": 43, "latestLedgerCloseTime": 1700000000_i64,
                "oldestLedger": 42, "oldestLedgerCloseTime": 1700000000_i64,
                "cursor": "43", "ledgers": ledgers,
            }
        })))
        .expect(1)
        .mount(&mock)
        .await;
    let rpc = Server::new(
        &mock.uri(),
        Options {
            allow_http: true,
            ..Default::default()
        },
    )
    .unwrap();
    let response = rpc.get_ledgers(Pagination::From(42), 2).await.unwrap();
    for (ledger, discriminant) in response.ledgers.into_iter().zip([3, 4]) {
        let header = ledger.to_header().unwrap().header;
        assert_eq!(header.scp_value.ext.discriminant() as i32, discriminant);
        assert_eq!(header.scp_value.close_time.0, 1_700_000_000);
        let xdr::LedgerCloseMeta::V2(meta) = ledger.to_metadata().unwrap() else {
            panic!("expected LedgerCloseMetaV2");
        };
        assert_eq!(meta.ledger_header.header, header);
    }
}

#[tokio::test]
async fn protocol_30_muxed_contracts_are_rejected_in_storage_lookups() {
    let rpc = Server::new(
        "http://127.0.0.1:1",
        Options {
            allow_http: true,
            ..Default::default()
        },
    )
    .unwrap();
    let encoded = "WA7QYNF7SOWQ3GLR2BGMZEHXAVIRZA4KVWLTJJFC7MGXUA74P7UJUAAAAAAAAAAAAAWWC";
    let address = Address::new(encoded).unwrap().to_sc_val().unwrap();
    let nested = xdr::ScVal::Vec(Some(vec![address.clone()].try_into().unwrap()));
    let map_key = xdr::ScVal::Map(Some(
        vec![xdr::ScMapEntry {
            key: address.clone(),
            val: xdr::ScVal::Void,
        }]
        .try_into()
        .unwrap(),
    ));
    let map_value = xdr::ScVal::Map(Some(
        vec![xdr::ScMapEntry {
            key: xdr::ScVal::Void,
            val: address.clone(),
        }]
        .try_into()
        .unwrap(),
    ));
    for key in [address, nested, map_key, map_value] {
        assert!(matches!(
            rpc.get_contract_data(
                "CA7QYNF7SOWQ3GLR2BGMZEHXAVIRZA4KVWLTJJFC7MGXUA74P7UJUWDA",
                key,
                Durability::Persistent,
            )
            .await,
            Err(soroban_client::error::Error::ContractDataNotFound)
        ));
    }
    assert!(matches!(
        rpc.get_contract_data(encoded, xdr::ScVal::Void, Durability::Persistent)
            .await,
        Err(soroban_client::error::Error::ContractDataNotFound)
    ));
}
