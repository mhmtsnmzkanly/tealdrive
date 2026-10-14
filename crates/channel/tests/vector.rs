//! Cacophony interoperability vector distributed with Snow0.10.0 (MIT/Apache2).
//! Source: https://github.com/mcginty/snow/blob/v0.10.0/tests/vectors/cacophony.txt
fn hex(value: &str) -> Vec<u8> {
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|c| u8::from_str_radix(std::str::from_utf8(c).unwrap(), 16).unwrap())
        .collect()
}
#[test]
fn official_cacophony_nk_handshake_and_directional_transport_vector() {
    let v: serde_json::Value = serde_json::from_str(include_str!("cacophony-nk.json")).unwrap();
    let bytes = |key: &str| hex(v[key].as_str().unwrap());
    let ie = bytes("init_ephemeral");
    let re = bytes("resp_ephemeral");
    let rs = bytes("resp_static");
    let remote = bytes("init_remote_static");
    let prologue = bytes("init_prologue");
    let mut init = snow::Builder::new(tealdrive_channel::PATTERN.parse().unwrap())
        .remote_public_key(&remote)
        .unwrap()
        .prologue(&prologue)
        .unwrap()
        .fixed_ephemeral_key_for_testing_only(&ie)
        .build_initiator()
        .unwrap();
    let mut resp = snow::Builder::new(tealdrive_channel::PATTERN.parse().unwrap())
        .local_private_key(&rs)
        .unwrap()
        .prologue(&prologue)
        .unwrap()
        .fixed_ephemeral_key_for_testing_only(&re)
        .build_responder()
        .unwrap();
    let messages = v["messages"].as_array().unwrap();
    let mut cipher = [0; 65535];
    let mut plain = [0; 65535];
    for (i, m) in messages.iter().take(2).enumerate() {
        let payload = hex(m["payload"].as_str().unwrap());
        let expected = hex(m["ciphertext"].as_str().unwrap());
        let (sender, receiver) = if i == 0 {
            (&mut init, &mut resp)
        } else {
            (&mut resp, &mut init)
        };
        let n = sender.write_message(&payload, &mut cipher).unwrap();
        assert_eq!(&cipher[..n], expected);
        let n = receiver.read_message(&cipher[..n], &mut plain).unwrap();
        assert_eq!(&plain[..n], payload);
    }
    let expected_hash = bytes("handshake_hash");
    assert_eq!(init.get_handshake_hash(), expected_hash);
    assert_eq!(resp.get_handshake_hash(), expected_hash);
    let mut init = init.into_transport_mode().unwrap();
    let mut resp = resp.into_transport_mode().unwrap();
    for (i, m) in messages.iter().skip(2).enumerate() {
        let payload = hex(m["payload"].as_str().unwrap());
        let expected = hex(m["ciphertext"].as_str().unwrap());
        let (sender, receiver) = if i % 2 == 0 {
            (&mut init, &mut resp)
        } else {
            (&mut resp, &mut init)
        };
        let n = sender.write_message(&payload, &mut cipher).unwrap();
        assert_eq!(&cipher[..n], expected);
        let n = receiver.read_message(&cipher[..n], &mut plain).unwrap();
        assert_eq!(&plain[..n], payload);
    }
}
