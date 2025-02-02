// TODO: These are equivalent and bs58 seems better. migrate.
// Test that our base58 encoder can encode/decode arbitrary bytes.
#[test]
fn base58_bytes() {
    use rust_base58::*;

    // Public keys and the private half of private keys are 32 bytes:
    let original_bytes = vec![
        0x00, 0x00, 0xFF, 0x00,
        0xFF, 0x00, 0xFF, 0x00,
        0xFF, 0x00, 0xFF, 0x00,
        0xFF, 0x00, 0xFF, 0x00,
        0xFF, 0x00, 0xFF, 0x00,
        0xFF, 0x00, 0xFF, 0x00,
        0xFF, 0x00, 0xFF, 0x00,
        0xFF, 0x00, 0xFF, 0x00,
    ];
    
    let encoded = original_bytes.to_base58();

    assert_eq!(encoded.as_str(), "11t6ZcNTcApL3s8ScmkCwNcFxJ47FfATY7cdg632k4X");
    let decoded = encoded.from_base58().unwrap();

    assert_eq!(original_bytes, decoded);

    // // Typos get decoded too?  Nope.
    // let decoded = "llLQX".from_base58().unwrap();
    // assert_eq!(original_bytes, decoded);
}

// The new one is better?
#[test]
fn bs58_bytes() {
    use bs58;

    // Public keys and the private half of private keys are 32 bytes:
    let original_bytes = vec![
        0x00, 0x00, 0xFF, 0x00,
        0xFF, 0x00, 0xFF, 0x00,
        0xFF, 0x00, 0xFF, 0x00,
        0xFF, 0x00, 0xFF, 0x00,
        0xFF, 0x00, 0xFF, 0x00,
        0xFF, 0x00, 0xFF, 0x00,
        0xFF, 0x00, 0xFF, 0x00,
        0xFF, 0x00, 0xFF, 0x00,
    ];
    
    let encoded = bs58::encode(&original_bytes)
        .with_alphabet(bs58::alphabet::Alphabet::BITCOIN)
        .into_string()
    ;

    assert_eq!(encoded.as_str(), "11t6ZcNTcApL3s8ScmkCwNcFxJ47FfATY7cdg632k4X");


    let decoded = bs58::decode(encoded)
        .into_vec()
        .unwrap()
    ;

    assert_eq!(original_bytes, decoded);


    let encoded = bs58::encode(&original_bytes)
        .with_check()
        .into_string()
    ;

    assert_eq!(encoded.as_str(), "116mMWNtNGD53Go3cNrwfPGW8dDEgLCGPdB7HAND77SUGmaWE");

    // // Should be able to fix up incorrect letters: But does not.
    // let decoded = bs58::decode("1l6mMWNtNGD53Go3cNrwfPGW8dDEgLCGPdB7HAND77SUGmaWE")
    //     .with_check(None)
    //     .into_vec()
    //     .unwrap();
    // assert_eq!(original_bytes, decoded);

}

// The time crate doesn't really specify what the max duration is. 
#[test]
fn time_duration() {
    use time::Duration;

    let max = Duration::max_value();
    assert_eq!(9223372036854775807999, max.whole_milliseconds());

    // Seems like enough years. 😆
    assert_eq!(292471208677, max.whole_days() / 365);

    // Diskuto uses an i64 # ms since epoch, so its max is:
    let max_feo = Duration::milliseconds(i64::MAX);
    assert_eq!(292471208, max_feo.whole_days() / 365);
}

